//! The short validated names of pages, bindings, slots, fields, versions and
//! principals.

use crate::error::{ParseError, Subject};
use std::fmt;

/// Checks the length and the characters of a name.
fn check_name(
    subject: Subject,
    text: &str,
    max: usize,
    first_ok: fn(char) -> bool,
    rest_ok: fn(char) -> bool,
    rule: &'static str,
) -> Result<(), ParseError> {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return Err(ParseError::Empty { subject });
    };
    if text.len() > max {
        return Err(ParseError::TooLong { subject, max });
    }
    if first_ok(first) && chars.all(rest_ok) {
        Ok(())
    } else {
        Err(ParseError::Malformed { subject, rule })
    }
}

fn label_start(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit()
}

fn label_rest(character: char) -> bool {
    label_start(character) || character == '-'
}

fn check_label(subject: Subject, text: &str) -> Result<(), ParseError> {
    check_name(
        subject,
        text,
        64,
        label_start,
        label_rest,
        "lower-case letters, digits and hyphens, starting with a letter or digit",
    )
}

fn check_slot(text: &str) -> Result<(), ParseError> {
    check_name(
        Subject::SlotName,
        text,
        32,
        |character| character.is_ascii_lowercase(),
        |character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        },
        "lower-case letters, digits and underscores, starting with a letter",
    )
}

fn check_field(text: &str) -> Result<(), ParseError> {
    check_name(
        Subject::FieldName,
        text,
        64,
        |character| character != '$' && !character.is_control(),
        |character| !character.is_control(),
        "a name without control characters that does not start with a dollar sign",
    )
}

fn check_version(text: &str) -> Result<(), ParseError> {
    check_name(
        Subject::PageVersion,
        text,
        128,
        |character| character.is_ascii_alphanumeric(),
        |character| character.is_ascii_alphanumeric(),
        "ASCII letters and digits",
    )
}

fn check_principal(text: &str) -> Result<(), ParseError> {
    check_name(
        Subject::PrincipalId,
        text,
        256,
        |character| character.is_ascii_graphic(),
        |character| character.is_ascii_graphic(),
        "printable ASCII without spaces",
    )
}

/// Defines a name type that only a checking constructor builds.
macro_rules! name_type {
    ($(#[$meta:meta])* $name:ident, $check:expr) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// The name as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<&str> for $name {
            type Error = ParseError;

            fn try_from(text: &str) -> Result<Self, ParseError> {
                let check: fn(&str) -> Result<(), ParseError> = $check;
                check(text)?;
                Ok(Self(text.to_owned()))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

name_type!(
    /// The name of a page: `[a-z0-9][a-z0-9-]{0,63}`.
    PageName,
    |text| check_label(Subject::PageName, text)
);
name_type!(
    /// The name of a binding: `[a-z0-9][a-z0-9-]{0,63}`.
    BindingName,
    |text| check_label(Subject::BindingName, text)
);
name_type!(
    /// The name of a slot: `[a-z][a-z0-9_]{0,31}`.
    SlotName,
    check_slot
);
name_type!(
    /// The name of a top-level record field: 1 to 64 bytes with no control
    /// character that never start with `$`.
    FieldName,
    check_field
);
name_type!(
    /// An opaque version name: 1 to 128 ASCII letters and digits. In a board
    /// it is a commit id.
    PageVersion,
    check_version
);
name_type!(
    /// The id of a principal: 1 to 256 printable ASCII bytes without spaces.
    PrincipalId,
    check_principal
);

#[cfg(test)]
mod tests;
