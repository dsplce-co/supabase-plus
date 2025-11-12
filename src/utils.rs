#[macro_export]
macro_rules! use_promptuity {
    ($promptuity:ident => $block:block) => {{
        let mut term = promptuity::Term::<std::io::Stderr>::default();
        let mut theme = promptuity::themes::FancyTheme::default();
        let mut $promptuity = promptuity::Promptuity::new(&mut term, &mut theme);
        let _ = $promptuity.term().clear();

        $block
    }};
}

pub fn escape_for_sh_double_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
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
