//! The records that fill a binding's slots.

use crate::error::{ParseError, Subject};
use crate::name::SlotName;
use crate::record::RecordPath;
use std::collections::BTreeMap;

/// The records that fill a binding's slots. A binding fills at least one
/// slot and may leave others unfilled.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Fills(BTreeMap<SlotName, RecordPath>);

impl Fills {
    /// Builds the fills of a binding.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::Empty`] for no entry and
    /// [`ParseError::Duplicate`] when a slot is named twice.
    pub fn new(
        entries: impl IntoIterator<Item = (SlotName, RecordPath)>,
    ) -> Result<Self, ParseError> {
        let mut map = BTreeMap::new();
        for (slot, path) in entries {
            if map.insert(slot, path).is_some() {
                return Err(ParseError::Duplicate {
                    subject: Subject::Fills,
                });
            }
        }
        if map.is_empty() {
            return Err(ParseError::Empty {
                subject: Subject::Fills,
            });
        }
        Ok(Self(map))
    }

    /// The record that fills `slot`.
    #[must_use]
    pub fn get(&self, slot: &SlotName) -> Option<&RecordPath> {
        self.0.get(slot)
    }

    /// The entries in slot name order.
    pub fn iter(&self) -> impl Iterator<Item = (&SlotName, &RecordPath)> {
        self.0.iter()
    }
}
