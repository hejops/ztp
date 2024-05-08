use std::collections::HashSet;

use unicode_segmentation::UnicodeSegmentation;
// basic tuple struct (single unnamed private field)
/// A struct that parses user-submitted `name` and enforces constraints, namely:
/// reject empty/whitespace, enforce maximum length, reject some problematic
/// characters.
///
/// Must be instantiated with `SubscriberName::parse`.
///
/// The field is left private, to prevent bypassing of `parse`, and mutation of
/// the value.
#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
    pub fn parse(name: String) -> Result<Self, String> {
        let empty = name.trim().is_empty();
        let too_long = name.graphemes(true).count() > 256;
        let bad_chars: HashSet<char> = r#"/()"<>\{}"#.chars().collect();
        let bad = name.chars().any(|c| bad_chars.contains(&c));
        match !empty && !too_long && !bad {
            true => Ok(Self(name)), //.to_string()),
            // // panics are reserved for unrecoverable errors; in most cases, `Result` is
            // // preferable
            // false => panic!("Invalid name: {name:?}"),
            false => Err(format!("Invalid name: {name:?}")),
        }
    }

    // // better written as a trait method (see below)
    // pub fn as_ref(&self) -> &str { &self.0 }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str { &self.0 }
}

#[cfg(test)]
mod tests {
    use claims::assert_err;
    use claims::assert_ok;

    // by default, calling assert!(foo.is_ok()) does not reveal the `Err` in cargo
    // test:
    //
    // ---- dummy_fail stdout ----
    // thread 'dummy_fail' panicked at tests/health_check.rs:236:5:
    // assertion failed: result.is_ok()
    //
    // with claims::assert_ok(result):
    //
    // ---- dummy_fail stdout ----
    // thread 'dummy_fail' panicked at tests/health_check.rs:244:5:
    // assertion failed, expected Ok(..), got Err("The app crashed due to an IO
    // error")
    use crate::domain::SubscriberName;

    #[test]
    fn name_ok() {
        assert_ok!(SubscriberName::parse("a".repeat(256)));
        assert_ok!(SubscriberName::parse("john".to_string()));
    }

    #[test]
    fn too_long() {
        assert_err!(SubscriberName::parse("a".repeat(257)));
    }

    #[test]
    fn whitespace() {
        assert_err!(SubscriberName::parse(" ".to_string()));
    }

    #[test]
    fn empty() {
        assert_err!(SubscriberName::parse("".to_string()));
    }

    #[test]
    fn bad_chars() {
        for c in r#"/()"<>\{}"#.chars() {
            assert_err!(SubscriberName::parse(c.to_string()));
        }
    }
}
