//! The error that every parsing constructor returns.

use std::fmt;

/// The value that a constructor refused to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Subject {
    /// A [`PageName`](crate::PageName).
    PageName,
    /// A [`BindingName`](crate::BindingName).
    BindingName,
    /// A [`SlotName`](crate::SlotName).
    SlotName,
    /// A [`FieldName`](crate::FieldName).
    FieldName,
    /// A [`PageVersion`](crate::PageVersion).
    PageVersion,
    /// A [`PrincipalId`](crate::PrincipalId).
    PrincipalId,
    /// A [`RecordPath`](crate::RecordPath).
    RecordPath,
    /// A [`RecordType`](crate::RecordType).
    RecordType,
    /// A [`Blob`](crate::Blob).
    Blob,
    /// A [`RecordBody`](crate::RecordBody).
    RecordBody,
    /// A [`PageContent`](crate::PageContent).
    PageContent,
    /// A [`Slot`](crate::Slot).
    Slot,
    /// A [`Slots`](crate::Slots).
    Slots,
    /// A [`Fills`](crate::Fills).
    Fills,
    /// The sources of a [`Provenance`](crate::Provenance).
    Sources,
}

impl fmt::Display for Subject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::PageName => "page name",
            Self::BindingName => "binding name",
            Self::SlotName => "slot name",
            Self::FieldName => "field name",
            Self::PageVersion => "page version",
            Self::PrincipalId => "principal id",
            Self::RecordPath => "record path",
            Self::RecordType => "record type",
            Self::Blob => "blob",
            Self::RecordBody => "record body",
            Self::PageContent => "page content",
            Self::Slot => "slot",
            Self::Slots => "slots",
            Self::Fills => "fills",
            Self::Sources => "sources",
        };
        formatter.write_str(text)
    }
}

/// Why a parsing constructor refused its input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// The value has no content and the type needs some.
    #[error("{subject} is empty")]
    Empty {
        /// The value that was refused.
        subject: Subject,
    },
    /// The value is longer than the type allows.
    #[error("{subject} is longer than {max} bytes")]
    TooLong {
        /// The value that was refused.
        subject: Subject,
        /// The longest accepted length in bytes.
        max: usize,
    },
    /// The value has more entries than the type allows.
    #[error("{subject} has more than {max} entries")]
    TooMany {
        /// The value that was refused.
        subject: Subject,
        /// The largest accepted number of entries.
        max: usize,
    },
    /// The value names one entry twice.
    #[error("{subject} names one entry twice")]
    Duplicate {
        /// The value that was refused.
        subject: Subject,
    },
    /// The value breaks the spelling rule of the type.
    #[error("{subject} must be {rule}")]
    Malformed {
        /// The value that was refused.
        subject: Subject,
        /// The rule that the value breaks, as a phrase that completes "must be".
        rule: &'static str,
    },
}
