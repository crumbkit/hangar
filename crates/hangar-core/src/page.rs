//! Pages, their slots and the rule that decides whether records fit them.

use crate::error::{ParseError, Subject};
use crate::name::{FieldName, PageName, SlotName};
use crate::record::{RecordBody, RecordPath, RecordType};

/// The largest page content in bytes.
const MAX_CONTENT_BYTES: usize = 1024 * 1024;
/// The largest number of slots in one page.
const MAX_SLOTS: usize = 32;

/// The markup language of a page's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContentFormat {
    /// The content is HTML.
    Html,
    /// The content is Markdown.
    Markdown,
}

/// The text of a page, in HTML or Markdown, at most 1 MiB.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PageContent {
    format: ContentFormat,
    text: String,
}

impl PageContent {
    /// Builds page content.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::TooLong`] when the text is over 1 MiB.
    pub fn new(format: ContentFormat, text: impl Into<String>) -> Result<Self, ParseError> {
        let text = text.into();
        if text.len() > MAX_CONTENT_BYTES {
            return Err(ParseError::TooLong {
                subject: Subject::PageContent,
                max: MAX_CONTENT_BYTES,
            });
        }
        Ok(Self { format, text })
    }

    /// The markup language of the text.
    #[must_use]
    pub fn format(&self) -> ContentFormat {
        self.format
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// An example record that shows what a slot looks like when nothing fills it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Sample {
    record_type: RecordType,
    body: RecordBody,
}

impl Sample {
    /// Builds a sample from a type and a body.
    #[must_use]
    pub fn new(record_type: RecordType, body: RecordBody) -> Self {
        Self { record_type, body }
    }

    /// The sample's record type.
    #[must_use]
    pub fn record_type(&self) -> &RecordType {
        &self.record_type
    }

    /// The sample's body.
    #[must_use]
    pub fn body(&self) -> &RecordBody {
        &self.body
    }
}

/// A place in a page that records of one type fill.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Slot {
    name: SlotName,
    record_type: RecordType,
    editable_fields: Vec<FieldName>,
    sample: Option<Sample>,
}

impl Slot {
    /// Builds a slot that accepts `record_type` and lets a form edit
    /// `editable_fields`.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::Malformed`] when the sample's type differs from
    /// the slot's type.
    pub fn new(
        name: SlotName,
        record_type: RecordType,
        editable_fields: Vec<FieldName>,
        sample: Option<Sample>,
    ) -> Result<Self, ParseError> {
        if sample
            .as_ref()
            .is_some_and(|sample| sample.record_type() != &record_type)
        {
            return Err(ParseError::Malformed {
                subject: Subject::Slot,
                rule: "given a sample of the slot's record type",
            });
        }
        Ok(Self {
            name,
            record_type,
            editable_fields,
            sample,
        })
    }

    /// The slot's name.
    #[must_use]
    pub fn name(&self) -> &SlotName {
        &self.name
    }

    /// The record type the slot accepts.
    #[must_use]
    pub fn record_type(&self) -> &RecordType {
        &self.record_type
    }

    /// The fields that a form may edit.
    #[must_use]
    pub fn editable_fields(&self) -> &[FieldName] {
        &self.editable_fields
    }

    /// The sample record, when the slot has one.
    #[must_use]
    pub fn sample(&self) -> Option<&Sample> {
        self.sample.as_ref()
    }
}

/// The slots of one page: at most 32, with distinct names.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Slots(Vec<Slot>);

impl Slots {
    /// Builds the slot list of a page.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::TooMany`] for more than 32 slots and
    /// [`ParseError::Duplicate`] when two slots share a name.
    pub fn new(slots: Vec<Slot>) -> Result<Self, ParseError> {
        if slots.len() > MAX_SLOTS {
            return Err(ParseError::TooMany {
                subject: Subject::Slots,
                max: MAX_SLOTS,
            });
        }
        let mut names: Vec<&SlotName> = slots.iter().map(Slot::name).collect();
        names.sort();
        if names.windows(2).any(|pair| pair.first() == pair.get(1)) {
            return Err(ParseError::Duplicate {
                subject: Subject::Slots,
            });
        }
        Ok(Self(slots))
    }

    /// The slot with this name.
    #[must_use]
    pub fn get(&self, name: &SlotName) -> Option<&Slot> {
        self.0.iter().find(|slot| slot.name() == name)
    }

    /// The slots in the order they were given.
    pub fn iter(&self) -> impl Iterator<Item = &Slot> {
        self.0.iter()
    }

    /// The number of slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the page has no slot.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A page: a name, content and the slots that records fill.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Page {
    name: PageName,
    content: PageContent,
    slots: Slots,
}

impl Page {
    /// Builds a page.
    #[must_use]
    pub fn new(name: PageName, content: PageContent, slots: Slots) -> Self {
        Self {
            name,
            content,
            slots,
        }
    }

    /// The page's name.
    #[must_use]
    pub fn name(&self) -> &PageName {
        &self.name
    }

    /// The page's content.
    #[must_use]
    pub fn content(&self) -> &PageContent {
        &self.content
    }

    /// The page's slots.
    #[must_use]
    pub fn slots(&self) -> &Slots {
        &self.slots
    }

    /// Checks that each filled slot is declared and that the record read for
    /// it has the slot's type. A declared slot may stay unfilled.
    ///
    /// The function performs no read. The caller reads the type of each
    /// record that fills a slot and passes the pairs in.
    ///
    /// # Errors
    ///
    /// Returns the first [`Misfit`] found, in the order of `filled`.
    pub fn check_fit<'a>(
        &self,
        filled: impl IntoIterator<Item = (&'a SlotName, &'a RecordType)>,
    ) -> Result<(), Misfit> {
        for (slot, found) in filled {
            let Some(declared) = self.slots.get(slot) else {
                return Err(Misfit::UndeclaredSlot { slot: slot.clone() });
            };
            if declared.record_type() != found {
                return Err(Misfit::WrongType {
                    slot: slot.clone(),
                    expected: declared.record_type().clone(),
                    found: found.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Why a set of records does not fit a page.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Misfit {
    /// A fill names a slot that the page does not declare.
    #[error("the page declares no slot named {slot}")]
    UndeclaredSlot {
        /// The slot that the fill names.
        slot: SlotName,
    },
    /// A record's type differs from the type of the slot it fills.
    #[error("the record in slot {slot} has type {found} but the slot accepts {expected}")]
    WrongType {
        /// The slot that the record fills.
        slot: SlotName,
        /// The type that the slot accepts.
        expected: RecordType,
        /// The type of the record.
        found: RecordType,
    },
    /// A record that fills a slot does not exist.
    #[error("the record {path} in slot {slot} does not exist")]
    MissingRecord {
        /// The slot that the record fills.
        slot: SlotName,
        /// The path that the fill names.
        path: RecordPath,
    },
}

mod fills;
mod published;

pub use fills::Fills;
pub use published::{PagePin, PublishedPage};

#[cfg(test)]
mod tests;
