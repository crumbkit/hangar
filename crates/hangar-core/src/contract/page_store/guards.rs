//! The [`PageStore`] clauses about version names, the pin a move expects, the
//! provenance of a move and the fit check of a stored binding.

use super::{
    PageFixture, TASK_A, TASKS, bind, binding_name, move_to, page_name, pin, pin_of, publish,
};
use crate::contract::{Clause, Violation, ensure, fixture, unexpected};
use crate::{Misfit, MoveError, PageStore, PageStoreError, RecordPath, RecordStore, SlotName};

/// A page never gets the same version name twice, and an older version keeps
/// its content after later publishes.
///
/// # Errors
///
/// Returns [`Clause::VersionNamesAreNeverReused`] otherwise.
pub async fn version_names_are_never_reused<S: PageStore, R: RecordStore>(
    store: &S,
    _records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let first = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let second = publish(store, contract_fixture, "home", "two", TASKS).await?;
    let third = publish(store, contract_fixture, "home", "three", TASKS).await?;
    let listed = unexpected(store.versions(&page_name("home")?).await)?;
    let oldest = unexpected(store.at(&pin("home", first.as_str())?).await)?;
    ensure(
        first != second
            && first != third
            && second != third
            && listed == [third.clone(), second.clone(), first.clone()]
            && oldest.page().content().text() == "one",
        Clause::VersionNamesAreNeverReused,
        || format!("published {first:?}, {second:?} and {third:?}, the page lists {listed:?}"),
    )
}

/// A move from a pin that the binding does not have answers
/// [`MoveError::Moved`] with the pin that the binding has, and changes
/// nothing.
///
/// # Errors
///
/// Returns [`Clause::MoveBindingRefusesAStalePin`] otherwise.
pub async fn move_binding_refuses_a_stale_pin<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let v2 = publish(store, contract_fixture, "home", "two", TASKS).await?;
    let v3 = publish(store, contract_fixture, "home", "three", TASKS).await?;
    bind(
        store,
        records,
        contract_fixture,
        "my-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    unexpected(move_to(store, records, contract_fixture, "my-view", &v1, &v2).await?)?;
    let before = unexpected(store.binding(&binding_name("my-view")?).await)?;
    let answer = move_to(store, records, contract_fixture, "my-view", &v1, &v3).await?;
    let after = unexpected(store.binding(&binding_name("my-view")?).await)?;
    ensure(
        answer
            == Err(MoveError::Moved {
                current: v2.clone(),
            })
            && after == before,
        Clause::MoveBindingRefusesAStalePin,
        || format!("a move from a stale pin answered {answer:?}, the binding is now {after:?}"),
    )
}

/// A move keeps the provenance of its mover as the last move of the binding,
/// beside the provenance of the save, which it leaves as it was. A binding
/// that no move has changed has no last move.
///
/// # Errors
///
/// Returns [`Clause::AMoveRecordsItsMover`] otherwise.
pub async fn a_move_records_its_mover<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let v2 = publish(store, contract_fixture, "home", "two", TASKS).await?;
    let saved = bind(
        store,
        records,
        contract_fixture,
        "my-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    let name = binding_name("my-view")?;
    let unmoved = unexpected(store.binding(&name).await)?;
    unexpected(move_to(store, records, contract_fixture, "my-view", &v1, &v2).await?)?;
    let moved = unexpected(store.binding(&name).await)?;
    let mover = contract_fixture.mover("my-view")?;
    ensure(
        unmoved.last_move().is_none()
            && moved.last_move() == Some(&mover)
            && moved.provenance() == saved.provenance(),
        Clause::AMoveRecordsItsMover,
        || format!("before the move {unmoved:?}, after it {moved:?}"),
    )
}

/// Checking a stored binding against the records as they are now refuses a
/// record that no longer exists with `Misfit`, accepts the records that still
/// fit, refuses an unknown binding with `NotFound` and changes nothing.
/// `without_records` is a record store that holds none of the fixture's
/// records.
///
/// # Errors
///
/// Returns [`Clause::CheckFitSeesAVanishedRecord`] otherwise.
pub async fn check_fit_sees_a_vanished_record<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    without_records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let clause = Clause::CheckFitSeesAVanishedRecord;
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let saved = bind(
        store,
        records,
        contract_fixture,
        "my-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    let name = binding_name("my-view")?;
    let still_fits = store.check_fit(&name, records).await;
    let vanished = store.check_fit(&name, without_records).await;
    let unknown = store.check_fit(&binding_name("ghost-view")?, records).await;
    let after = unexpected(store.binding(&name).await)?;
    let expected = PageStoreError::Misfit(Misfit::MissingRecord {
        slot: fixture(SlotName::try_from("tasks"))?,
        path: fixture(RecordPath::try_from(TASK_A))?,
    });
    ensure(
        still_fits == Ok(())
            && vanished == Err(expected)
            && unknown == Err(PageStoreError::NotFound)
            && after == saved
            && pin_of(store, "my-view").await? == v1,
        clause,
        || {
            format!(
                "the records that still fit answered {still_fits:?}, the vanished record {vanished:?}, an unknown binding {unknown:?}"
            )
        },
    )
}
