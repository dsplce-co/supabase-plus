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
