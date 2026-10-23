use std::collections::BTreeMap;

use crate::abstraction::{Column, MaskKind, TableFilter, TableRef, sql};

#[derive(Debug, Clone)]
struct PlannedColumn {
    name: String,
    type_sql: String,
    replacement: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CopyItem {
    table: TableRef,
    columns: Vec<PlannedColumn>,
    filter: TableFilter,
}

impl CopyItem {
    pub fn new(table: &TableRef, production: &[Column]) -> anyhow::Result<Self> {
        if production.is_empty() {
            crate::styled_bail!(
                "`{}` doesn't exist in production",
                (table.to_string(), "id")
            );
        }

        Ok(Self {
            table: table.clone(),
            columns: production
                .iter()
                .filter(|column| !column.generated)
                .map(|column| PlannedColumn {
                    name: column.name.clone(),
                    type_sql: column.type_sql.clone(),
                    replacement: None,
                })
                .collect(),
            filter: TableFilter::default(),
        })
    }

    pub fn auth_users(production: &[Column], salt: &str) -> anyhow::Result<Self> {
        Ok(Self::new(&TableRef::auth_users(), production)?
            .mask_email("email", "TRUE", salt)
            .mask_email("email_change", "TRUE", salt)
            .mask_json_email("raw_user_meta_data", salt))
    }

    pub fn auth_identities(production: &[Column], salt: &str) -> anyhow::Result<Self> {
        Ok(Self::new(&TableRef::auth_identities(), production)?
            .mask_email(
                "provider_id",
                "\"provider_id\" = (\"identity_data\"->>'email')",
                salt,
            )
            .mask_json_email("identity_data", salt))
    }

    pub fn with_filter(mut self, filter: TableFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn with_masks(
        mut self,
        rules: &BTreeMap<String, MaskKind>,
        salt: &str,
    ) -> anyhow::Result<Self> {
        for (name, kind) in rules {
            let Some(column) = self.columns.iter_mut().find(|column| &column.name == name) else {
                crate::styled_bail!(
                    "Can't mask `{}`, there's no such column in production",
                    (format!("{}.{name}", self.table), "id")
                );
            };

            if !MaskKind::fits_type(&column.type_sql) {
                crate::styled_bail!(
                    "Can't mask `{}`, it's `{}` and only text columns can be masked",
                    (format!("{}.{name}", self.table), "id"),
                    (&column.type_sql, "property")
                );
            }

            column.replacement = Some(kind.select_expr(name, &column.type_sql, "TRUE", salt));
        }

        Ok(self)
    }

    fn mask_email(mut self, column: &str, condition: &str, salt: &str) -> Self {
        if let Some(column) = self.columns.iter_mut().find(|other| other.name == column) {
            column.replacement =
                Some(MaskKind::Email.select_expr(&column.name, &column.type_sql, condition, salt));
        }

        self
    }

    fn mask_json_email(mut self, column: &str, salt: &str) -> Self {
        if let Some(column) = self.columns.iter_mut().find(|other| other.name == column) {
            column.replacement =
                Some(MaskKind::Email.json_key_select_expr(&column.name, "email", salt));
        }

        self
    }

    pub fn retain_columns(mut self, local: &[Column]) -> Self {
        self.columns.retain(|column| {
            local
                .iter()
                .any(|other| other.name == column.name && !other.generated)
        });

        self
    }

    pub fn table(&self) -> &TableRef {
        &self.table
    }

    pub fn masked_columns(&self) -> Vec<String> {
        self.columns
            .iter()
            .filter(|column| column.replacement.is_some())
            .map(|column| column.name.clone())
            .collect()
    }

    pub fn select_sql(&self) -> String {
        let columns = self
            .columns
            .iter()
            .map(|column| {
                column
                    .replacement
                    .clone()
                    .unwrap_or_else(|| sql::ident(&column.name))
            })
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "SELECT {columns} FROM {}{}",
            self.table.quoted(),
            self.filter.sql_tail()
        )
    }

    pub fn copy_out_sql(&self) -> String {
        format!("COPY ({}) TO STDOUT", self.select_sql())
    }

    pub fn copy_in_sql(&self) -> String {
        format!(
            "COPY {} ({}) FROM STDIN",
            self.table.quoted(),
            self.column_list()
        )
    }

    pub fn insert_statements_sql(&self) -> String {
        let prefix = format!(
            "INSERT INTO {} ({}) OVERRIDING SYSTEM VALUE VALUES (",
            self.table.quoted(),
            self.column_list()
        );

        let values = self
            .columns
            .iter()
            .map(|column| format!("quote_nullable(s.{}::text)", sql::ident(&column.name)))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "SELECT {} || concat_ws(', ', {values}) || ') ON CONFLICT DO NOTHING;' FROM ({}) AS s",
            sql::literal(&prefix),
            self.select_sql()
        )
    }

    fn column_list(&self) -> String {
        self.columns
            .iter()
            .map(|column| sql::ident(&column.name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, type_sql: &str, generated: bool) -> Column {
        Column {
            name: name.into(),
            type_sql: type_sql.into(),
            generated,
        }
    }

    #[test]
    fn auth_keeps_only_columns_the_local_version_has_and_masks_emails() {
        let production = [
            col("id", "uuid", false),
            col("email", "text", false),
            col("new_column", "text", false),
        ];
        let local = [col("id", "uuid", false), col("email", "text", false)];

        let item = CopyItem::auth_users(&production, "s")
            .unwrap()
            .retain_columns(&local);

        assert_eq!(item.masked_columns(), ["email"]);
        assert!(!item.select_sql().contains("new_column"));
    }

    #[test]
    fn seeds_get_one_insert_per_row_of_the_select() {
        let columns = [col("id", "uuid", false), col("total", "numeric", true)];
        let item = CopyItem::new(&TableRef::parse("orders").unwrap(), &columns).unwrap();

        assert!(
            item.insert_statements_sql()
                .contains("(\"id\") OVERRIDING SYSTEM VALUE VALUES (")
        );
        assert!(
            item.insert_statements_sql()
                .ends_with(&format!("FROM ({}) AS s", item.select_sql()))
        );
    }
}
