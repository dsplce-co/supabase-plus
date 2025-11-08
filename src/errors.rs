use urlencoding::encode;

fn issue_url_with_preset(why: &'static str, error: &str) -> String {
    let title = format!("Unexpected state as `{why}`");

    let body = format!(
        "<Please replace this placeholder with a brief description what command you tried to run>

```
{error}
```"
    );

    format!(
        "https://github.com/dsplce-co/supabase-plus/issues/new?title={}&body={}",
        encode(&title),
        encode(&body)
    )
}

pub fn no_way_fmt<T, U: std::fmt::Debug>(why: &'static str, result: &Result<T, U>) -> String {
    let Some(error) = result.as_ref().err() else {
        return String::new();
    };

    format!(
        "\nDuring the development this potential error has been marked as not likely to occur because `{why}`\nPlease click the link below to automatically open a GitHub issue:\n\n{}\n\nPlease describe what you were doing when it occurred. Details below will be automatically included",
        issue_url_with_preset(why, &format!("{:#?}", error))
    )
}

pub trait NoWay<T> {
    /// To use with `Result` or `Option` when it's visible in the code that panic is not expected
    /// prints error about reporting GH issue
    fn no_way_because(self, why: &'static str) -> T;
}

impl<T, U: std::fmt::Debug> NoWay<T> for Result<T, U> {
    fn no_way_because(self, why: &'static str) -> T {
        let message = no_way_fmt(why, &self);
        self.expect(&message)
    }
}

impl<T> NoWay<T> for Option<T> {
    fn no_way_because(self, why: &'static str) -> T {
        let message = no_way_fmt(why, &self.as_ref().ok_or("None"));
        self.expect(&message)
    }
}
