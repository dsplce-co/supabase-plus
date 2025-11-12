#[macro_export]
macro_rules! styled_bail {
    ($text:expr) => {{ anyhow::bail!($crate::styled_error!($text)) }};

    ($format:expr, $(($text:expr, $style:expr)),+ $(,)?) => {{
        anyhow::bail!($crate::styled_error!($format, $(($text, $style)),+))
    }};
}

#[macro_export]
macro_rules! styled_error {
    ($text:expr) => {
        {
            supercli::output::styling::replace_symbols($text)
        }
    };

    ($format:expr, $(($text:expr, $style:expr)),+ $(,)?) => {
        {
            let mut result = supercli::output::styling::replace_symbols($format);

            $(
                let styled_text = supercli::output::styling::apply_style($text, $style);

                if let Some(pos) = result.find("{}") {
                    let before = &result[..pos];
                    let after = &result[pos + 2..];
                    result = format!("{}{}{}", before, styled_text, supercli::output::styling::apply_style(after, "error"));
                }
            )+

            result
        }
    };
}
