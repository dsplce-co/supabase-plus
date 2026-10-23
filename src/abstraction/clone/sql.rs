pub fn ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
