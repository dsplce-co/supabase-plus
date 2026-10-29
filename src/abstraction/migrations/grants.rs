use crate::abstraction::{AccessMatrix, Migration};

pub struct GrantsFix {
    statements: Vec<String>,
}

impl GrantsFix {
    pub fn new(matrix: &AccessMatrix) -> Option<Self> {
        let mut statements = Vec::new();

        for row in matrix.fixable() {
            for (role, privileges) in row.missing() {
                statements.push(format!(
                    "grant {privileges} on {} {} to {role};",
                    row.kind.sql_object(),
                    row.name
                ));
            }
        }

        (!statements.is_empty()).then_some(Self { statements })
    }
}

impl Migration for GrantsFix {
    fn sql(&self) -> String {
        self.statements.join("\n")
    }

    fn migration_name(&self) -> String {
        "fix_grants".to_string()
    }
}
