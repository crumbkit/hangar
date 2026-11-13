//! The [`RecordStore`] contract suite.

use super::{Clause, SeedRecord, Violation, ensure, fixture, seed, unexpected};
use crate::{
    BindingName, Blob, Principal, PrincipalId, ReadError, RecordBody, RecordPath, RecordStore,
    WriteError, WriteProvenance,
};

/// The records and values that the suite uses.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RecordFixture {
    seeds: Vec<SeedRecord>,
    other_body: RecordBody,
    missing: RecordPath,
    principal: Principal,
    binding: BindingName,
}

impl RecordFixture {
    /// The standard fixture.
    ///
    /// # Errors
    ///
    /// Returns a [`Violation`] if a constant of the fixture is refused by its
    /// type, which means this crate is broken.
    pub fn standard() -> Result<Self, Violation> {
        Ok(Self {
            seeds: vec![seed("/tasks/a", "tasks/task/v2", "title: first\n")?],
            other_body: fixture(RecordBody::try_from("title: second\n"))?,
            missing: fixture(RecordPath::try_from("/tasks/missing"))?,
            principal: Principal::new(fixture(PrincipalId::try_from("contract-suite"))?),
            binding: fixture(BindingName::try_from("contract-view"))?,
        })
    }

    /// The records to seed into a new store before running a function. The
    /// first one is the record that the functions read and write.
    #[must_use]
    pub fn seeds(&self) -> &[SeedRecord] {
        &self.seeds
    }

    fn target(&self) -> Result<&SeedRecord, Violation> {
        fixture(self.seeds.first().ok_or("the fixture has no seed"))
    }

    fn write_based_on(&self, blob: Blob) -> WriteProvenance {
        WriteProvenance::new(blob, self.principal.clone(), self.binding.clone())
    }
}

/// A blob that differs from `current` and that no store has handed out.
fn unrelated_blob(current: &Blob) -> Result<Blob, Violation> {
    for digit in ["f", "e"] {
        let candidate = fixture(Blob::try_from(digit.repeat(40).as_str()))?;
        if candidate != *current {
            return Ok(candidate);
        }
    }
    fixture::<Blob, _>(Err("every candidate blob equals the current blob"))
}

/// A read returns the record that was seeded.
///
/// # Errors
///
/// Returns [`Clause::ReadReturnsWhatWasSeeded`] when the record differs.
pub async fn read_returns_what_was_seeded<S: RecordStore>(
    store: &S,
    contract_fixture: &RecordFixture,
) -> Result<(), Violation> {
    let seeded = contract_fixture.target()?;
    let record = unexpected(store.read(seeded.path()).await)?;
    ensure(
        record.path() == seeded.path()
            && record.record_type() == seeded.record_type()
            && record.body() == seeded.body(),
        Clause::ReadReturnsWhatWasSeeded,
        || format!("{record:?}"),
    )
}

/// A write with the current blob returns a new blob, and a read then returns
/// the new body under the new blob.
///
/// # Errors
///
/// Returns [`Clause::WriteReturnsANewBlobAndReplacesTheBody`] otherwise.
pub async fn write_returns_a_new_blob_and_replaces_the_body<S: RecordStore>(
    store: &S,
    contract_fixture: &RecordFixture,
) -> Result<(), Violation> {
    let path = contract_fixture.target()?.path();
    let before = unexpected(store.read(path).await)?;
    let new_blob = unexpected(
        store
            .write(
                path,
                &contract_fixture.other_body,
                &contract_fixture.write_based_on(before.blob().clone()),
            )
            .await,
    )?;
    let after = unexpected(store.read(path).await)?;
    ensure(
        new_blob != *before.blob()
            && after.blob() == &new_blob
            && after.body() == &contract_fixture.other_body
            && after.record_type() == before.record_type(),
        Clause::WriteReturnsANewBlobAndReplacesTheBody,
        || format!("returned {new_blob:?}, read {after:?}"),
    )
}

/// A write with a stale blob returns `Conflict` and leaves the body and the
/// blob unchanged.
///
/// # Errors
///
/// Returns [`Clause::AStaleBlobConflictsAndChangesNothing`] otherwise.
pub async fn a_stale_blob_conflicts_and_changes_nothing<S: RecordStore>(
    store: &S,
    contract_fixture: &RecordFixture,
) -> Result<(), Violation> {
    let path = contract_fixture.target()?.path();
    let before = unexpected(store.read(path).await)?;
    let stale = unrelated_blob(before.blob())?;
    let answer = store
        .write(
            path,
            &contract_fixture.other_body,
            &contract_fixture.write_based_on(stale),
        )
        .await;
    let after = unexpected(store.read(path).await)?;
    ensure(
        answer == Err(WriteError::Conflict) && after == before,
        Clause::AStaleBlobConflictsAndChangesNothing,
        || format!("write answered {answer:?}, record is now {after:?}"),
    )
}

/// A second write that names the blob from before the first write returns
/// `Conflict`.
///
/// # Errors
///
/// Returns [`Clause::AnOldBlobConflictsAfterAWrite`] otherwise.
pub async fn an_old_blob_conflicts_after_a_write<S: RecordStore>(
    store: &S,
    contract_fixture: &RecordFixture,
) -> Result<(), Violation> {
    let seeded = contract_fixture.target()?;
    let path = seeded.path();
    let before = unexpected(store.read(path).await)?;
    let first = unexpected(
        store
            .write(
                path,
                &contract_fixture.other_body,
                &contract_fixture.write_based_on(before.blob().clone()),
            )
            .await,
    )?;
    let answer = store
        .write(
            path,
            seeded.body(),
            &contract_fixture.write_based_on(before.blob().clone()),
        )
        .await;
    let after = unexpected(store.read(path).await)?;
    ensure(
        answer == Err(WriteError::Conflict)
            && after.blob() == &first
            && after.body() == &contract_fixture.other_body,
        Clause::AnOldBlobConflictsAfterAWrite,
        || format!("second write answered {answer:?}, record is now {after:?}"),
    )
}

/// A read or a write of an unknown path returns `NotFound`.
///
/// # Errors
///
/// Returns [`Clause::AnUnknownPathIsNotFound`] otherwise.
pub async fn an_unknown_path_is_not_found<S: RecordStore>(
    store: &S,
    contract_fixture: &RecordFixture,
) -> Result<(), Violation> {
    let read = store.read(&contract_fixture.missing).await;
    let current = unexpected(store.read(contract_fixture.target()?.path()).await)?;
    let write = store
        .write(
            &contract_fixture.missing,
            &contract_fixture.other_body,
            &contract_fixture.write_based_on(current.blob().clone()),
        )
        .await;
    ensure(
        read == Err(ReadError::NotFound) && write == Err(WriteError::NotFound),
        Clause::AnUnknownPathIsNotFound,
        || format!("read answered {read:?}, write answered {write:?}"),
    )
}
