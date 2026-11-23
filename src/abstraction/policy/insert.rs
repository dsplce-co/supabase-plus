use anyhow::Context;
use supercli::output::styling::apply_style;
use tokio_postgres::Client;

use super::{Identity, PolicyReport, PolicyRow, Verdict};
use crate::utils;

const NEW_ROW: &str = r#"
with target as (
    select c.oid, c.oid::regclass::text as name, c.relrowsecurity as enabled
    from pg_class c
    where c.oid = to_regclass(format('%I.%I', $1::text, $2::text)) and c.relkind in ('r', 'p')
),
attrs as (
    select a.attnum, a.attname::text as name, a.attgenerated <> '' as generated, pg_get_expr(d.adbin, d.adrelid) as fallback
    from target t
    join pg_attribute a on a.attrelid = t.oid and a.attnum > 0 and not a.attisdropped
    left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
)
select
    t.name,
    t.enabled,
    array(select k from unnest($3::text[]) as k where k <> all (select name from attrs)) as unknown,
    -- sequences can't advance in a read-only transaction, so their columns stay null
    'row(' || coalesce((
        select string_agg(
            case when generated or fallback is null or fallback like 'nextval(%' then 'null' else '(' || fallback || ')' end,
            ', ' order by attnum
        ) from attrs
    ), '') || ')::' || t.name as base,
    array(
        select coalesce(
            (select bool_and(has_column_privilege(r.role, t.oid, a.attnum, 'insert')) from attrs a where a.name = any($3::text[])),
            has_any_column_privilege(r.role, t.oid, 'insert')
        )
        from unnest($4::text[]) with ordinality as r(role, ord) order by r.ord
    ) as insertable,
    array(
        select has_any_column_privilege(r.role, t.oid, 'select')
        from unnest($4::text[]) with ordinality as r(role, ord) order by r.ord
    ) as readable,
    array(
        select 'trigger ' || g.tgname from pg_trigger g
        where g.tgrelid = t.oid and not g.tgisinternal and g.tgenabled <> 'D' and g.tgtype & 7 = 7
        union all
        select 'generated column ' || a.name from attrs a where a.generated
    ) as computed
from target t
"#;

pub struct NewRow {
    table: String,
    json: String,
    base: String,
    enabled: bool,
    insertable: Vec<bool>,
    readable: Vec<bool>,
    computed: Vec<String>,
}

impl NewRow {
    pub async fn load(
        client: &mut Client,
        schema: &str,
        table: &str,
        row: &str,
        roles: &[&str],
    ) -> anyhow::Result<Self> {
        let object: serde_json::Map<String, serde_json::Value> = serde_json::from_str(row)
            .with_context(|| {
                styled_error!(
                    "`{}` takes a JSON object with the values of columns, like `{}`",
                    ("--insert", "command"),
                    (r#"{"total": 30}"#, "muted")
                )
            })?;

        let keys: Vec<&String> = object.keys().collect();

        let transaction = client.transaction().await?;

        transaction
            .batch_execute(
                "set local transaction_read_only = on; set local statement_timeout = '30s'",
            )
            .await?;

        let found = transaction
            .query_opt(NEW_ROW, &[&schema, &table, &keys, &roles])
            .await
            .context("Couldn't read the table")?
            .with_context(|| {
                styled_error!(
                    "Couldn't find table `{}` in `{}` schema",
                    (table, "id"),
                    (schema, "id")
                )
            })?;

        let name: String = found.get("name");
        let unknown: Vec<String> = found.get("unknown");

        if !unknown.is_empty() {
            crate::styled_bail!(
                "Table `{}` has no column `{}`",
                (&name, "id"),
                (unknown.join("`, `"), "id")
            );
        }

        if let Err(error) = transaction
            .execute(
                &format!("select json_populate_record(null::{name}, $1::text::json)"),
                &[&row],
            )
            .await
        {
            crate::styled_bail!(
                "The row doesn't fit `{}`: {}",
                (&name, "id"),
                (utils::database_message(&error), "warning")
            );
        }

        transaction.rollback().await?;

        Ok(Self {
            table: table.to_string(),
            json: row.to_string(),
            base: found.get("base"),
            enabled: found.get("enabled"),
            insertable: found.get("insertable"),
            readable: found.get("readable"),
            computed: found.get("computed"),
        })
    }

    pub(super) fn can(&self, command: &str, index: usize) -> bool {
        match command {
            "insert" => self.insertable[index],
            _ => self.readable[index],
        }
    }
}

impl PolicyRow {
    pub(super) fn checks(self) -> Vec<Self> {
        match self.command.as_str() {
            "all" => vec![
                Self {
                    command: "insert".to_string(),
                    expression: self.insert_check.clone(),
                    ..self.clone()
                },
                Self {
                    command: "select".to_string(),
                    ..self
                },
            ],
            "insert" | "select" => vec![self],
            _ => Vec::new(),
        }
    }

    pub(super) async fn accepts(
        &self,
        client: &mut Client,
        identity: &Identity,
        new_row: &NewRow,
    ) -> Result<Verdict, tokio_postgres::Error> {
        let transaction = client.transaction().await?;

        identity.assume(&transaction).await?;

        let row = transaction
            .query_one(
                &format!(
                    "select ({}) from json_populate_record({}, $1::text::json) as {}",
                    self.expression, new_row.base, self.alias
                ),
                &[&new_row.json],
            )
            .await?;

        Ok(Verdict::Allows(row.get::<_, Option<bool>>(0) == Some(true)))
    }
}

impl PolicyReport {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.new_row.is_none()
    }

    pub fn computed(&self) -> &[String] {
        self.new_row
            .as_ref()
            .map_or(&[], |new_row| &new_row.computed)
    }

    pub(super) fn standings(&self) -> String {
        let Some(new_row) = &self.new_row else {
            return String::new();
        };

        let width = self
            .identities
            .iter()
            .map(|identity| identity.label.chars().count())
            .max()
            .unwrap_or_default();

        (0..self.identities.len())
            .map(|index| self.standing(new_row, index, width))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn standing(&self, new_row: &NewRow, index: usize, width: usize) -> String {
        let failed = self
            .rows
            .iter()
            .any(|row| matches!(row.verdicts[index], Verdict::Failed(_)));

        let (text, style) = match (
            failed,
            self.rejection(new_row, index, "insert"),
            self.rejection(new_row, index, "select"),
        ) {
            (true, ..) => (
                "couldn't be checked, a policy failed to evaluate".to_string(),
                "warning",
            ),
            (_, Some(error), _) => (format!("blocked: {error}"), "warning"),
            (_, None, Some(error)) => (
                format!("passes, but `.select()` would be blocked: {error}"),
                "warning",
            ),
            _ if !new_row.enabled => (
                "passes, row level security is off for the table".to_string(),
                "success",
            ),
            _ => ("passes".to_string(), "success"),
        };

        format!(
            "{} {}",
            apply_style(format!("{:<width$}", self.identities[index].label), "id"),
            apply_style(text, style)
        )
    }

    /// Postgres checks privileges first, then the permissive policies as one and finally each
    /// restrictive one, and the error it raises names the policy only in the last case
    fn rejection(&self, new_row: &NewRow, index: usize, command: &str) -> Option<String> {
        let table = &new_row.table;

        if !new_row.can(command, index) {
            return Some(format!("permission denied for table {table}"));
        }

        if !new_row.enabled {
            return None;
        }

        let checks = || self.rows.iter().filter(|row| row.command == command);

        let permissive = || {
            checks().filter(|row| {
                !row.restrictive && !matches!(row.verdicts[index], Verdict::NotApplicable)
            })
        };

        if !permissive().any(|row| matches!(row.verdicts[index], Verdict::Allows(true))) {
            let hint = if permissive().next().is_none() {
                format!(" (no permissive {command} policy applies)")
            } else {
                String::new()
            };

            return Some(format!(
                r#"new row violates row-level security policy for table "{table}"{hint}"#
            ));
        }

        checks()
            .find(|row| row.restrictive && matches!(row.verdicts[index], Verdict::Allows(false)))
            .map(|row| {
                format!(
                    r#"new row violates row-level security policy "{}" for table "{table}""#,
                    row.name
                )
            })
    }
}
