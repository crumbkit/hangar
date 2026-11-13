//! Board records as hangar sees them: a path, a type, a blob and a body.

use crate::error::{ParseError, Subject};
use std::fmt;

/// The longest record path in bytes.
const MAX_PATH_BYTES: usize = 1024;
/// The longest record type in bytes.
const MAX_TYPE_BYTES: usize = 128;
/// The largest record body in bytes.
pub const MAX_RECORD_BYTES: usize = 1024 * 1024;

/// The absolute path of a record in a board, such as `/tasks/a`.
///
/// A path has no empty, `.` or `..` segment, no control character (NUL and
/// newline included) and no backslash, and at most 1,024 bytes. The caller percent-decodes before parsing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordPath(String);

impl RecordPath {
    /// The path as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for RecordPath {
    type Error = ParseError;

    fn try_from(text: &str) -> Result<Self, ParseError> {
        let subject = Subject::RecordPath;
        if text.is_empty() {
            return Err(ParseError::Empty { subject });
        }
        if text.len() > MAX_PATH_BYTES {
            return Err(ParseError::TooLong {
                subject,
                max: MAX_PATH_BYTES,
            });
        }
        if text.contains('\\') || text.chars().any(char::is_control) {
            return Err(ParseError::Malformed {
                subject,
                rule: "free of control characters and backslash",
            });
        }
        let Some(rest) = text.strip_prefix('/') else {
            return Err(ParseError::Malformed {
                subject,
                rule: "absolute",
            });
        };
        if rest
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."))
        {
            return Err(ParseError::Malformed {
                subject,
                rule: "made of segments that are not empty, `.` or `..`",
            });
        }
        Ok(Self(text.to_owned()))
    }
}

impl fmt::Display for RecordPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The type of a record, such as `tasks/task/v2`: segments of `[a-z0-9-]`
/// joined by `/`, at most 128 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordType(String);

impl RecordType {
    /// The type as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for RecordType {
    type Error = ParseError;

    fn try_from(text: &str) -> Result<Self, ParseError> {
        let subject = Subject::RecordType;
        if text.is_empty() {
            return Err(ParseError::Empty { subject });
        }
        if text.len() > MAX_TYPE_BYTES {
            return Err(ParseError::TooLong {
                subject,
                max: MAX_TYPE_BYTES,
            });
        }
        let segment_ok = |segment: &str| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        };
        if text.split('/').all(segment_ok) {
            Ok(Self(text.to_owned()))
        } else {
            Err(ParseError::Malformed {
                subject,
                rule: "segments of lower-case letters, digits and hyphens joined by `/`",
            })
        }
    }
}

impl fmt::Display for RecordType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The identity of one version of a record: 40 or 64 lower-case hex digits.
/// It is the value that `board put --expect` takes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Blob(String);

impl Blob {
    /// The blob as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for Blob {
    type Error = ParseError;

    fn try_from(text: &str) -> Result<Self, ParseError> {
        let subject = Subject::Blob;
        if text.is_empty() {
            return Err(ParseError::Empty { subject });
        }
        let hex = text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'));
        if hex && matches!(text.len(), 40 | 64) {
            Ok(Self(text.to_owned()))
        } else {
            Err(ParseError::Malformed {
                subject,
                rule: "40 or 64 lower-case hex digits",
            })
        }
    }
}

impl fmt::Display for Blob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The text of a record, at most 1 MiB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordBody(String);

impl RecordBody {
    /// The body as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for RecordBody {
    type Error = ParseError;

    fn try_from(text: &str) -> Result<Self, ParseError> {
        if text.len() > MAX_RECORD_BYTES {
            return Err(ParseError::TooLong {
                subject: Subject::RecordBody,
                max: MAX_RECORD_BYTES,
            });
        }
        Ok(Self(text.to_owned()))
    }
}

/// A board record as a store returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Record {
    path: RecordPath,
    record_type: RecordType,
    blob: Blob,
    body: RecordBody,
}

impl Record {
    /// Builds a record from its parts.
    #[must_use]
    pub fn new(path: RecordPath, record_type: RecordType, blob: Blob, body: RecordBody) -> Self {
        Self {
            path,
            record_type,
            blob,
            body,
        }
    }

    /// The record's path.
    #[must_use]
    pub fn path(&self) -> &RecordPath {
        &self.path
    }

    /// The record's type.
    #[must_use]
    pub fn record_type(&self) -> &RecordType {
        &self.record_type
    }

    /// The identity of this version of the record.
    #[must_use]
    pub fn blob(&self) -> &Blob {
        &self.blob
    }

    /// The record's text.
    #[must_use]
    pub fn body(&self) -> &RecordBody {
        &self.body
    }
}

#[cfg(test)]
mod tests;
