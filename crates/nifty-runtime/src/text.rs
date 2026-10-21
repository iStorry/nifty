//! The string methods and `json_text`, shared by the interpreter and compiled apps so text
//! is cut and searched the same way however an app runs. Positions count characters, as
//! `len` does.

use serde_json::Value;

use crate::failure::Failure;

coded_enum! {
    /// A built-in method on strings beyond `len` and `is_empty`, which lists have too.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TextMethod {
        Contains,
        StartsWith,
        EndsWith,
        Find,
        Slice,
        Split,
        Replace,
        Trim,
    }
}

impl TextMethod {
    /// The method's name, such as `starts_with`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::StartsWith => "starts_with",
            Self::EndsWith => "ends_with",
            Self::Find => "find",
            Self::Slice => "slice",
            Self::Split => "split",
            Self::Replace => "replace",
            Self::Trim => "trim",
        }
    }
}

/// The position of the first character of the first `needle` in `text`, or `-1` when
/// `text` doesn't hold it. An empty `needle` is found at `0`.
///
/// # Errors
/// When the position doesn't fit in an `int`.
pub fn find(text: &str, needle: &str) -> Result<i64, Failure> {
    text.find(needle).map_or(Ok(-1), |byte| {
        i64::try_from(text[..byte].chars().count()).map_err(|_| Failure::TooLong)
    })
}

/// The characters of `text` from `start` up to, but not including, `end`.
///
/// # Errors
/// Unless `0 <= start <= end <= text.len()`.
pub fn slice(text: &str, start: i64, end: i64) -> Result<String, Failure> {
    let len = text.chars().count();
    let out_of_range = || Failure::NoSlice { start, end, len };
    let from = usize::try_from(start).map_err(|_| out_of_range())?;
    let to = usize::try_from(end).map_err(|_| out_of_range())?;
    if from > to || to > len {
        return Err(out_of_range());
    }
    Ok(text.chars().skip(from).take(to - from).collect())
}

/// The pieces of `text` between each `separator`, or its characters when `separator` is
/// empty, which has nothing between its characters to split at.
#[must_use]
pub fn split(text: &str, separator: &str) -> Vec<String> {
    if separator.is_empty() {
        return text.chars().map(String::from).collect();
    }
    text.split(separator).map(str::to_owned).collect()
}

/// `text` with each `from` replaced by `to`. An empty `from` matches nothing, so `text`
/// comes back as it is.
#[must_use]
pub fn replace(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_owned();
    }
    text.replace(from, to)
}

/// The value at `path` in the JSON `json`, as text: a string's own text, and any other
/// value written as JSON, such as `42`, `true` or `{"a":1}`. Each step names an object's
/// key, or an array's position written as a number, such as `"0"`. `""` when `json` isn't
/// JSON, or nothing is at `path`, or the value there is `null`.
#[must_use]
pub fn json_text(json: &str, path: &[&str]) -> String {
    let Ok(document) = serde_json::from_str::<Value>(json) else {
        return String::new();
    };
    let found = path.iter().try_fold(&document, |value, step| match value {
        Value::Object(fields) => fields.get(*step),
        Value::Array(items) => step
            .parse::<usize>()
            .ok()
            .and_then(|index| items.get(index)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    });
    match found {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_by_character_position() {
        assert_eq!(find("héllo wörld", "wö"), Ok(6));
        assert_eq!(find("hello", "x"), Ok(-1));
        assert_eq!(find("hello", ""), Ok(0));
    }

    #[test]
    fn slices_by_character_position() {
        assert_eq!(slice("héllo", 1, 4), Ok("éll".to_owned()));
        assert_eq!(slice("héllo", 5, 5), Ok(String::new()));
        assert_eq!(slice("", 0, 0), Ok(String::new()));
    }

    #[test]
    fn refuses_a_slice_outside_the_text() {
        let out = |start, end| Err(Failure::NoSlice { start, end, len: 5 });
        assert_eq!(slice("héllo", 2, 6), out(2, 6));
        assert_eq!(slice("héllo", -1, 2), out(-1, 2));
        assert_eq!(slice("héllo", 3, 2), out(3, 2));
    }

    #[test]
    fn splits_at_each_separator_or_into_characters() {
        assert_eq!(split("a; b;;c", ";"), ["a", " b", "", "c"]);
        assert_eq!(split("", ","), [""]);
        assert_eq!(split("hé", ""), ["h", "é"]);
    }

    #[test]
    fn replaces_every_match_and_nothing_for_an_empty_one() {
        assert_eq!(replace("a\\u002Fb\\u002F", "\\u002F", "/"), "a/b/");
        assert_eq!(replace("abc", "", "-"), "abc");
    }

    #[test]
    fn reads_json_values_as_text() {
        let json = r#"{"a": {"b": ["x", {"c": "https://e.com"}]}, "n": 42, "t": true, "z": null}"#;
        assert_eq!(json_text(json, &["a", "b", "1", "c"]), "https://e.com");
        assert_eq!(json_text(json, &["a", "b", "0"]), "x");
        assert_eq!(json_text(json, &["n"]), "42");
        assert_eq!(json_text(json, &["t"]), "true");
        assert_eq!(
            json_text(json, &["a", "b", "1"]),
            r#"{"c":"https://e.com"}"#
        );
        assert_eq!(json_text(json, &["z"]), "");
        assert_eq!(json_text(json, &["a", "b", "2"]), "");
        assert_eq!(json_text(json, &["a", "b", "x"]), "");
        assert_eq!(json_text(json, &["n", "deeper"]), "");
        assert_eq!(json_text("not json", &[]), "");
        assert_eq!(json_text(r#"[1, "two"]"#, &[]), r#"[1,"two"]"#);
    }
}
