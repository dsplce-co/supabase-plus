use std::collections::BTreeMap;

use crate::abstraction::{TableRef, sql};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskKind {
    Email,
    Generic,
}

impl MaskKind {
    pub fn for_column(column: &str) -> Self {
        if column.to_lowercase().contains("email") {
            Self::Email
        } else {
            Self::Generic
        }
    }

    pub fn looks_personal(column: &str) -> bool {
        let column = column.to_lowercase();

        [
            "email",
            "phone",
            "mobile",
            "address",
            "street",
            "postal",
            "zip_code",
            "city",
            "first_name",
            "last_name",
            "full_name",
            "surname",
            "username",
        ]
        .iter()
        .any(|needle| column.contains(needle))
            || ["name", "ip"].contains(&column.as_str())
    }

    pub fn fits_type(column_type: &str) -> bool {
        ["text", "character varying", "character(", "citext", "name"]
            .iter()
            .any(|prefix| column_type.starts_with(prefix))
    }

    fn fake_sql(self, value_sql: &str, salt: &str) -> String {
        let hash = format!("md5({} || {value_sql})", sql::literal(salt));

        match self {
            Self::Email => format!("'user_' || substr({hash}, 1, 16) || '@example.com'"),
            Self::Generic => format!("'masked_' || substr({hash}, 1, 12)"),
        }
    }

    pub fn select_expr(
        self,
        column: &str,
        column_type: &str,
        condition: &str,
        salt: &str,
    ) -> String {
        let column = sql::ident(column);
        let fake = self.fake_sql(&format!("{column}::text"), salt);

        format!(
            "(CASE WHEN {column} IS NULL OR {column}::text = '' OR NOT ({condition}) THEN {column}::text ELSE {fake} END)::{column_type} AS {column}"
        )
    }

    pub fn json_key_select_expr(self, column: &str, key: &str, salt: &str) -> String {
        let column = sql::ident(column);
        let key = sql::literal(key);
        let value = format!("({column}->>{key})");
        let fake = self.fake_sql(&value, salt);

        format!(
            "(CASE WHEN jsonb_typeof({column}->{key}) = 'string' AND {value} <> '' THEN jsonb_set({column}, ARRAY[{key}], to_jsonb({fake})) ELSE {column} END) AS {column}"
        )
    }
}

#[derive(Debug, Default)]
pub struct MaskRules(BTreeMap<TableRef, BTreeMap<String, MaskKind>>);

impl MaskRules {
    pub fn parse(values: &[String]) -> anyhow::Result<Self> {
        let mut rules = Self::default();

        for value in values {
            let Some((table, column)) = value.trim().rsplit_once('.') else {
                crate::styled_bail!(
                    "`{}` should look like `{}`",
                    (value, "id"),
                    ("table.column", "property")
                );
            };

            rules.insert(TableRef::parse(table)?, column);
        }

        Ok(rules)
    }

    pub fn insert(&mut self, table: TableRef, column: &str) {
        self.0
            .entry(table)
            .or_default()
            .insert(column.into(), MaskKind::for_column(column));
    }

    pub fn covers(&self, table: &TableRef, column: &str) -> bool {
        self.0
            .get(table)
            .is_some_and(|columns| columns.contains_key(column))
    }

    pub fn columns(&self, table: &TableRef) -> BTreeMap<String, MaskKind> {
        self.0.get(table).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_expression_is_salted_null_safe_and_typed() {
        let expr =
            MaskKind::Email.select_expr("email", "character varying(255)", "TRUE", "it's salty");

        assert!(expr.contains("IS NULL OR \"email\"::text = ''"));
        assert!(expr.contains("md5('it''s salty' || \"email\"::text)"));
        assert!(expr.ends_with(")::character varying(255) AS \"email\""));
    }
}
