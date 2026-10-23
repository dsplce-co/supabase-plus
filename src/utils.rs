#[macro_export]
macro_rules! use_promptuity {
    ($promptuity:ident => $block:block) => {{
        let mut term = $crate::patched::promptuity::Term::<std::io::Stderr>::default();
        let mut theme = $crate::patched::promptuity::themes::FancyTheme::default();
        let mut $promptuity = $crate::patched::promptuity::Promptuity::new(&mut term, &mut theme);
        let _ = $promptuity.term().clear();

        $block
    }};
}

pub fn escape_for_sh_double_quotes(value: &str) -> String {
    let mut out = String::with_capacity(value.len());

    for char in value.chars() {
        match char {
            '\\' => out.push_str(
                r#"\\#),
            '"'  => out.push_str(r#"\"#,
            ),
            '$' => out.push_str(r#"\$"#),
            '`' => out.push_str(r#"\`"#),
            other => out.push(other),
        }
    }

    out
}

pub fn human_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < 4 {
        value /= 1024.0;
        unit += 1;
    }

    format!("{value:.1} {}", ["B", "KB", "MB", "GB", "TB"][unit])
}
