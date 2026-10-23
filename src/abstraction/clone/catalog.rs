use std::collections::BTreeMap;

use anyhow::Context;
use regex::Regex;
use tokio_postgres::Client;

use crate::abstraction::{RESERVED_SCHEMAS, TableRef};
use crate::errors::NoWay;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub type_sql: String,
    pub generated: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct QuerySize {
    pub rows: u64,
    pub bytes: u64,
}

pub struct Catalog<'a> {
    client: &'a Client,
}

impl<'a> Catalog<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn columns(
        &self,
        tables: &[TableRef],
    ) -> anyhow::Result<BTreeMap<TableRef, Vec<Column>>> {
        let mut columns: BTreeMap<TableRef, Vec<Column>> = BTreeMap::new();

        if tables.is_empty() {
            return Ok(columns);
        }

        let schemas: Vec<String> = tables.iter().map(|table| table.schema.clone()).collect();
        let names: Vec<String> = tables.iter().map(|table| table.name.clone()).collect();

        let rows = self
            .client
            .query(
                "select t.nsp, t.rel, a.attname::text, format_type(a.atttypid, a.atttypmod), a.attgenerated::text <> ''
                 from unnest($1::text[], $2::text[]) as t(nsp, rel)
                 join pg_attribute a on a.attrelid = to_regclass(format('%I.%I', t.nsp, t.rel))
                 where a.attnum > 0 and not a.attisdropped
                 order by 1, 2, a.attnum",
                &[&schemas, &names],
            )
            .await
            .context("Couldn't read columns of the tables")?;

        for row in rows {
            columns
                .entry(TableRef::new(
                    &row.get::<_, String>(0),
                    &row.get::<_, String>(1),
                ))
                .or_default()
                .push(Column {
                    name: row.get(2),
                    type_sql: row.get(3),
                    generated: row.get(4),
                });
        }

        Ok(columns)
    }

    pub async fn user_tables(&self) -> anyhow::Result<Vec<TableRef>> {
        let reserved: Vec<String> = RESERVED_SCHEMAS
            .iter()
            .map(|schema| schema.to_string())
            .collect();

        let rows = self
            .client
            .query(
                "select n.nspname::text, c.relname::text
                 from pg_class c join pg_namespace n on n.oid = c.relnamespace
                 where c.relkind in ('r', 'p') and not c.relispartition
                   and n.nspname !~ '^pg_' and n.nspname <> all($1)
                 order by 1, 2",
                &[&reserved],
            )
            .await
            .context("Couldn't list tables")?;

        Ok(rows
            .into_iter()
            .map(|row| TableRef::new(&row.get::<_, String>(0), &row.get::<_, String>(1)))
            .collect())
    }

    pub async fn missing_buckets(&self, wanted: &[String]) -> anyhow::Result<Vec<String>> {
        let existing = self.buckets().await?;

        Ok(wanted
            .iter()
            .filter(|bucket| !existing.contains(bucket))
            .cloned()
            .collect())
    }

    pub async fn buckets(&self) -> anyhow::Result<Vec<String>> {
        let rows = self
            .client
            .query("select id::text from storage.buckets order by 1", &[])
            .await
            .context("Couldn't list storage buckets")?;

        Ok(rows.into_iter().map(|row| row.get(0)).collect())
    }

    pub async fn references(
        &self,
        tables: &[TableRef],
    ) -> anyhow::Result<Vec<(TableRef, TableRef)>> {
        let schemas: Vec<String> = tables.iter().map(|table| table.schema.clone()).collect();
        let names: Vec<String> = tables.iter().map(|table| table.name.clone()).collect();

        let rows = self
            .client
            .query(
                "select cn.nspname::text, cc.relname::text, pn.nspname::text, pc.relname::text
                 from pg_constraint con
                 join pg_class cc on cc.oid = con.conrelid
                 join pg_namespace cn on cn.oid = cc.relnamespace
                 join pg_class pc on pc.oid = con.confrelid
                 join pg_namespace pn on pn.oid = pc.relnamespace
                 where con.contype = 'f'
                   and (cn.nspname::text, cc.relname::text) in (select * from unnest($1::text[], $2::text[]))",
                &[&schemas, &names],
            )
            .await
            .context("Couldn't read foreign keys")?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    TableRef::new(&row.get::<_, String>(0), &row.get::<_, String>(1)),
                    TableRef::new(&row.get::<_, String>(2), &row.get::<_, String>(3)),
                )
            })
            .collect())
    }

    pub async fn auth_dependents(&self) -> anyhow::Result<Vec<TableRef>> {
        let rows = self
            .client
            .query(
                "with recursive deps(oid, depth) as (
                    select 'auth.users'::regclass::oid, 0
                    union
                    select c.conrelid, d.depth + 1
                    from pg_constraint c join deps d on c.confrelid = d.oid
                    where c.contype = 'f' and c.conrelid <> c.confrelid
                      and c.connamespace = 'auth'::regnamespace and d.depth < 6
                 )
                 select n.nspname::text, r.relname::text
                 from deps d join pg_class r on r.oid = d.oid join pg_namespace n on n.oid = r.relnamespace
                 where d.depth > 0
                 group by 1, 2 order by max(d.depth) desc, 2",
                &[],
            )
            .await
            .with_context(|| {
                styled_error!(
                    "Couldn't read tables depending on `{}`",
                    ("auth.users", "id")
                )
            })?;

        Ok(rows
            .into_iter()
            .map(|row| TableRef::new(&row.get::<_, String>(0), &row.get::<_, String>(1)))
            .collect())
    }

    pub async fn estimate(&self, select_sql: &str) -> anyhow::Result<QuerySize> {
        let rows = self
            .client
            .query(&format!("EXPLAIN {select_sql}"), &[])
            .await
            .context("Couldn't estimate the size of a query")?;

        let pattern = Regex::new(r"rows=(\d+) width=(\d+)").no_way_because("the regex is 'static");

        for row in rows {
            let line: String = row.get(0);

            if let Some(captures) = pattern.captures(&line) {
                let rows: u64 = captures[1].parse()?;
                let width: u64 = captures[2].parse()?;

                return Ok(QuerySize {
                    rows,
                    bytes: rows.saturating_mul(width),
                });
            }
        }

        Ok(QuerySize::default())
    }
}
