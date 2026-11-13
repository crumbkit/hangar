//! The [`PageStore`] clauses about pins, moving a binding and listing the
//! bindings of a page.

use super::{
    NOTE, NOTE_TYPE, PageFixture, TASK_A, TASK_TYPE, TASKS, TASKS_AND_NOTES, bind, binding,
    binding_name, move_to, page_name, pin, pin_of, publish, version,
};
use crate::contract::{Clause, Violation, ensure, fixture, unexpected};
use crate::{
    Misfit, MoveError, PageStore, PageStoreError, RecordPath, RecordStore, RecordType, SlotName,
};

/// The answer of a move that a page store refuses for a reason that a save
/// shares.
fn refused(error: PageStoreError) -> MoveError {
    MoveError::Refused(error)
}

/// A republish leaves a binding's pin unchanged.
///
/// # Errors
///
/// Returns [`Clause::ARepublishLeavesAPinUnchanged`] otherwise.
pub async fn a_republish_leaves_a_pin_unchanged<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    bind(
        store,
        records,
        contract_fixture,
        "my-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    publish(store, contract_fixture, "home", "two", TASKS).await?;
    let pinned = pin_of(store, "my-view").await?;
    ensure(pinned == v1, Clause::ARepublishLeavesAPinUnchanged, || {
        format!("the pin moved from {v1:?} to {pinned:?}")
    })
}

/// `move_binding` changes only the pin, toward an older and toward a newer
/// version.
///
/// # Errors
///
/// Returns [`Clause::MoveBindingChangesOnlyThePin`] otherwise.
pub async fn move_binding_changes_only_the_pin<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let clause = Clause::MoveBindingChangesOnlyThePin;
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let v2 = publish(store, contract_fixture, "home", "two", TASKS_AND_NOTES).await?;
    let original = bind(
        store,
        records,
        contract_fixture,
        "my-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    let name = binding_name("my-view")?;

    unexpected(move_to(store, records, contract_fixture, "my-view", &v1, &v2).await?)?;
    let newer = unexpected(store.binding(&name).await)?;
    ensure(
        newer.pin().version() == &v2
            && newer.pin().name() == original.pin().name()
            && newer.fills() == original.fills()
            && newer.provenance() == original.provenance(),
        clause,
        || format!("after moving to the newer version: {newer:?}"),
    )?;
    unexpected(move_to(store, records, contract_fixture, "my-view", &v2, &v1).await?)?;
    let older = unexpected(store.binding(&name).await)?;
    ensure(
        older.pin() == original.pin()
            && older.fills() == original.fills()
            && older.provenance() == original.provenance(),
        clause,
        || format!("after moving back to the older version: {older:?}"),
    )
}

/// `move_binding` to a version that dropped a filled slot, or whose slot
/// accepts another type, is refused with `Misfit` and leaves the pin
/// unchanged. An unknown version and an unknown binding are refused too.
///
/// # Errors
///
/// Returns [`Clause::MoveBindingRefusesAMisfit`] otherwise.
pub async fn move_binding_refuses_a_misfit<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let clause = Clause::MoveBindingRefusesAMisfit;
    let v1 = publish(store, contract_fixture, "home", "one", TASKS_AND_NOTES).await?;
    let v2 = publish(store, contract_fixture, "home", "two", TASKS).await?;
    let v3 = publish(
        store,
        contract_fixture,
        "home",
        "three",
        &[("tasks", NOTE_TYPE), ("notes", NOTE_TYPE)],
    )
    .await?;
    bind(
        store,
        records,
        contract_fixture,
        "notes-view",
        v1.as_str(),
        &[("notes", NOTE)],
    )
    .await?;
    bind(
        store,
        records,
        contract_fixture,
        "tasks-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;

    let dropped = move_to(store, records, contract_fixture, "notes-view", &v1, &v2).await?;
    let expected_dropped = refused(PageStoreError::Misfit(Misfit::UndeclaredSlot {
        slot: fixture(SlotName::try_from("notes"))?,
    }));
    ensure(dropped == Err(expected_dropped), clause, || {
        format!("a move to a version without the slot answered {dropped:?}")
    })?;
    let retyped = move_to(store, records, contract_fixture, "tasks-view", &v1, &v3).await?;
    let expected_retyped = refused(PageStoreError::Misfit(Misfit::WrongType {
        slot: fixture(SlotName::try_from("tasks"))?,
        expected: fixture(RecordType::try_from(NOTE_TYPE))?,
        found: fixture(RecordType::try_from(TASK_TYPE))?,
    }));
    ensure(retyped == Err(expected_retyped), clause, || {
        format!("a move to a version whose slot has another type answered {retyped:?}")
    })?;
    let nope = version("nope")?;
    let unknown_version =
        move_to(store, records, contract_fixture, "tasks-view", &v1, &nope).await?;
    let unknown_binding = move_to(store, records, contract_fixture, "ghost-view", &v1, &v2).await?;
    let notes_pin = pin_of(store, "notes-view").await?;
    let tasks_pin = pin_of(store, "tasks-view").await?;
    ensure(
        unknown_version == Err(refused(PageStoreError::UnknownVersion))
            && unknown_binding == Err(refused(PageStoreError::NotFound))
            && notes_pin == v1
            && tasks_pin == v1,
        clause,
        || {
            format!(
                "unknown version {unknown_version:?}, unknown binding {unknown_binding:?}, pins {notes_pin:?} and {tasks_pin:?}"
            )
        },
    )
}

/// `move_binding` with a bound record that no longer exists is refused with
/// `Misfit` and leaves the pin unchanged. `without_records` is a record
/// store that holds none of the fixture's records.
///
/// # Errors
///
/// Returns [`Clause::MoveBindingRefusesAVanishedRecord`] otherwise.
pub async fn move_binding_refuses_a_vanished_record<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    without_records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let v2 = publish(store, contract_fixture, "home", "two", TASKS).await?;
    bind(
        store,
        records,
        contract_fixture,
        "my-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    let answer = move_to(
        store,
        without_records,
        contract_fixture,
        "my-view",
        &v1,
        &v2,
    )
    .await?;
    let expected = refused(PageStoreError::Misfit(Misfit::MissingRecord {
        slot: fixture(SlotName::try_from("tasks"))?,
        path: fixture(RecordPath::try_from(TASK_A))?,
    }));
    let pinned = pin_of(store, "my-view").await?;
    ensure(
        answer == Err(expected) && pinned == v1,
        Clause::MoveBindingRefusesAVanishedRecord,
        || format!("the move answered {answer:?}, the pin is {pinned:?}"),
    )
}

/// `bindings_of` lists the bindings pinned to a page and no others, whether
/// a binding is pinned to the newest version of the page or to an older one.
///
/// # Errors
///
/// Returns [`Clause::BindingsOfListsAPagesBindings`] otherwise.
pub async fn bindings_of_lists_a_pages_bindings<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let other_v1 = publish(store, contract_fixture, "other", "one", TASKS).await?;
    let first = bind(
        store,
        records,
        contract_fixture,
        "first-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    let v2 = publish(store, contract_fixture, "home", "two", TASKS).await?;
    let second = bind(
        store,
        records,
        contract_fixture,
        "second-view",
        v2.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    let elsewhere = binding(
        "other-view",
        pin("other", other_v1.as_str())?,
        &[("tasks", TASK_A)],
        contract_fixture.original()?,
    )?;
    unexpected(store.save_binding(elsewhere.clone(), records).await)?;

    let mut home = unexpected(store.bindings_of(&page_name("home")?).await)?;
    let other = unexpected(store.bindings_of(&page_name("other")?).await)?;
    home.sort_by(|a, b| a.name().cmp(b.name()));
    ensure(
        home == [first, second] && other == [elsewhere],
        Clause::BindingsOfListsAPagesBindings,
        || format!("home {home:?}, other {other:?}"),
    )
}
