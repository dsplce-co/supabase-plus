use std::collections::BTreeMap;
use std::fmt;

use crate::abstraction::sql;

pub const RESERVED_SCHEMAS: &[&str] = &[
    "auth",
    "storage",
    "extensions",
    "graphql",
    "graphql_public",
    "pg_catalog",
    "information_schema",
    "pgsodium",
    "pgsodium_masks",
    "realtime",
    "supabase_functions",
    "supabase_migrations",
    "vault",
    "net",
    "pgbouncer",
    "_realtime",
    "_analytics",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableRef {
    pub schema: String,
    pub name: String,
}

impl TableRef {
    pub fn new(schema: &str, name: &str) -> Self {
        Self {
            schema: schema.into(),
            name: name.into(),
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let (schema, name) = match value.trim().split('.').collect::<Vec<_>>().as_slice() {
            [name] => ("public", *name),
            [schema, name] => (*schema, *name),
            _ => crate::styled_bail!(
                "`{}` is not a `{}` or `{}`",
                (value, "id"),
                ("table", "property"),
                ("schema.table", "property")
            ),
        };

        for part in [schema, name] {
            let plain = !part.is_empty()
                && part
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '$');

            if !plain {
                crate::styled_bail!("`{}` is not a plain Postgres identifier", (part, "id"));
            }
        }

        if RESERVED_SCHEMAS.contains(&schema) {
            crate::styled_bail!(
                "Schema `{}` is managed by Supabase and can't be picked",
                (schema, "id")
            );
        }

        Ok(Self::new(schema, name))
    }

    pub fn auth_users() -> Self {
        Self::new("auth", "users")
    }

    pub fn auth_identities() -> Self {
        Self::new("auth", "identities")
    }

    pub fn is_auth(&self) -> bool {
        self.schema == "auth"
    }

    pub fn quoted(&self) -> String {
        format!("{}.{}", sql::ident(&self.schema), sql::ident(&self.name))
    }

    pub fn reset_sequences_sql(tables: &[TableRef]) -> String {
        if tables.is_empty() {
            return String::new();
        }

        let values = tables
            .iter()
            .map(|table| {
                format!(
                    "({}, {})",
                    sql::literal(&table.schema),
                    sql::literal(&table.name)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "DO $sbp$
DECLARE r record;
BEGIN
  FOR r IN
    SELECT format('%I.%I', n.nspname, c.relname) AS tbl, a.attname::text AS col,
           pg_get_serial_sequence(format('%I.%I', n.nspname, c.relname), a.attname::text) AS seq
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
    WHERE (n.nspname::text, c.relname::text) IN (VALUES {values})
  LOOP
    IF r.seq IS NOT NULL THEN
      EXECUTE format(
        'SELECT setval(%L, COALESCE((SELECT max(%I) FROM %s), 1), (SELECT max(%I) FROM %s) IS NOT NULL)',
        r.seq, r.col, r.tbl, r.col, r.tbl
      );
    END IF;
  END LOOP;
END
$sbp$;"
        )
    }
}

impl fmt::Display for TableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.schema, self.name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TableFilter {
    pub condition: Option<String>,
    pub limit: Option<u64>,
}

impl TableFilter {
    pub fn sql_tail(&self) -> String {
        let mut tail = String::new();

        if let Some(condition) = &self.condition {
            tail.push_str(&format!(" WHERE ({condition})"));
        }

        if let Some(limit) = self.limit {
            tail.push_str(&format!(" LIMIT {limit}"));
        }

        tail
    }
}

#[derive(Debug, Default)]
pub struct Conditions(BTreeMap<TableRef, String>);

impl Conditions {
    pub fn parse(values: &[String]) -> anyhow::Result<Self> {
        let mut conditions = BTreeMap::new();

        for value in values {
            let Some((table, condition)) = value.split_once(':') else {
                crate::styled_bail!(
                    "`{}` should look like `{}`",
                    (value, "id"),
                    ("table:condition", "property")
                );
            };

            if condition.contains(';') {
                crate::styled_bail!("`{}` contains `;`, pass a single condition", (value, "id"));
            }

            conditions.insert(TableRef::parse(table)?, condition.trim().to_string());
        }

        Ok(Self(conditions))
    }

    pub fn filter(&self, table: &TableRef, limit: Option<u64>) -> TableFilter {
        TableFilter {
            condition: self.0.get(table).cloned(),
            limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_default_to_public_and_reserved_schemas_are_refused() {
        assert_eq!(
            TableRef::parse("orders").unwrap().to_string(),
            "public.orders"
        );
        assert!(TableRef::parse("auth.users").is_err());
        assert!(TableRef::parse("orders; drop table x").is_err());
    }

    #[test]
    fn conditions_cant_smuggle_a_second_statement() {
        assert!(
            Conditions::parse(&["orders:true); commit; delete from orders --".into()]).is_err()
        );
        assert!(Conditions::parse(&["orders:total > 1".into()]).is_ok());
    }
}
