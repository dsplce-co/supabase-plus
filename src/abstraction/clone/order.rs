use std::collections::{BTreeMap, BTreeSet};

use crate::abstraction::TableRef;

pub fn load_order(tables: &[TableRef], references: &[(TableRef, TableRef)]) -> Vec<TableRef> {
    let mut parents: BTreeMap<&TableRef, BTreeSet<&TableRef>> = tables
        .iter()
        .map(|table| (table, BTreeSet::new()))
        .collect();

    for (child, parent) in references {
        if child != parent
            && parents.contains_key(parent)
            && let Some(of_child) = parents.get_mut(child)
        {
            of_child.insert(parent);
        }
    }

    let mut sorted = Vec::new();

    loop {
        let ready: Vec<&TableRef> = parents
            .iter()
            .filter(|(_, of_table)| of_table.is_empty())
            .map(|(table, _)| *table)
            .collect();

        if ready.is_empty() {
            break;
        }

        for table in ready {
            parents.remove(table);

            for of_other in parents.values_mut() {
                of_other.remove(table);
            }

            sorted.push(table.clone());
        }
    }

    sorted.extend(parents.into_keys().cloned());
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parents_come_first() {
        let t = |name: &str| TableRef::parse(name).unwrap();

        let order = load_order(
            &[t("order_items"), t("orders"), t("users")],
            &[
                (t("order_items"), t("orders")),
                (t("orders"), t("users")),
                (t("users"), t("users")),
            ],
        );

        assert_eq!(order, [t("users"), t("orders"), t("order_items")]);
    }
}
