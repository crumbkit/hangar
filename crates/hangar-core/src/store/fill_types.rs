//! The fit rule that every page store applies to a binding.

use super::{PageStoreError, ReadError, RecordStore};
use crate::name::SlotName;
use crate::page::{Fills, Misfit, Page};
use crate::record::{RecordPath, RecordType};

/// What a record store answered for each record that a binding fills.
///
/// Every [`PageStore`](super::PageStore) applies the same rule to a binding:
/// it reads the type of each filled record, a record that does not exist is
/// a [`Misfit::MissingRecord`], and [`Page::check_fit`] compares the rest.
/// This type holds that rule once. [`FillTypes::read`] does the asynchronous
/// reads, and [`FillTypes::check`] applies the rule without waiting, so a
/// store can read first and then check and insert in one step under its own
/// lock.
#[derive(Debug, Clone)]
pub struct FillTypes {
    entries: Vec<(SlotName, Fetched)>,
}

#[derive(Debug, Clone)]
enum Fetched {
    Found(RecordType),
    Missing(RecordPath),
    Unavailable,
}

impl FillTypes {
    /// Reads the record at each path of `fills`, in the order of the fills.
    pub async fn read(fills: &Fills, records: &impl RecordStore) -> Self {
        let mut entries = Vec::new();
        for (slot, path) in fills.iter() {
            let fetched = match records.read(path).await {
                Ok(record) => Fetched::Found(record.record_type().clone()),
                Err(ReadError::NotFound) => Fetched::Missing(path.clone()),
                Err(_) => Fetched::Unavailable,
            };
            entries.push((slot.clone(), fetched));
        }
        Self { entries }
    }

    /// Checks the records that were read against `page`. Every fill was
    /// read before this check, and the check walks the fills in the order of
    /// the binding and stops at the first one that fails.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::Misfit`] for the first fill whose slot the
    /// page does not declare, whose record is missing or whose record has
    /// another type than its slot. A fill whose slot the page does not
    /// declare is a misfit even when its record could not be read. Returns
    /// [`PageStoreError::RecordsUnavailable`] for the first fill whose read
    /// failed for any other reason, when no earlier fill failed.
    pub fn check(&self, page: &Page) -> Result<(), PageStoreError> {
        for (slot, fetched) in &self.entries {
            if page.slots().get(slot).is_none() {
                return Err(PageStoreError::Misfit(Misfit::UndeclaredSlot {
                    slot: slot.clone(),
                }));
            }
            match fetched {
                Fetched::Found(record_type) => page
                    .check_fit([(slot, record_type)])
                    .map_err(PageStoreError::Misfit)?,
                Fetched::Missing(path) => {
                    return Err(PageStoreError::Misfit(Misfit::MissingRecord {
                        slot: slot.clone(),
                        path: path.clone(),
                    }));
                }
                Fetched::Unavailable => return Err(PageStoreError::RecordsUnavailable),
            }
        }
        Ok(())
    }
}
