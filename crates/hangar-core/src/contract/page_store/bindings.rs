//! The [`PageStore`] clauses about saving bindings.

use super::{
    MISSING, NOTE, NOTE_TYPE, PageFixture, TASK_A, TASK_TYPE, TASKS, TASKS_AND_NOTES, bind,
    binding, binding_name, is_not_found, page, page_name, pin, publish,
};
use crate::contract::{Clause, Violation, ensure, fixture, unexpected};
use crate::{
    MadeFrom, Misfit, PageStore, PageStoreError, RecordPath, RecordStore, RecordType, SlotName,
};

/// `publish` and `save_binding` refuse a `made_from` that names no existing
/// page version or binding with `UnresolvedOrigin` and store nothing, and
/// accept one that names an existing page version or binding.
///
/// # Errors
///
/// Returns [`Clause::AnUnresolvedOriginIsRefused`] otherwise.
pub async fn an_unresolved_origin_is_refused<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let clause = Clause::AnUnresolvedOriginIsRefused;
    let unresolved_page = contract_fixture.provenance(MadeFrom::Page(pin("home", "v1")?), &[])?;
    let published = store
        .publish(page("home", "one", TASKS)?, unresolved_page)
        .await;
    ensure(
        published == Err(PageStoreError::UnresolvedOrigin),
        clause,
        || format!("publish from a missing page answered {published:?}"),
    )?;
    let unresolved_binding =
        contract_fixture.provenance(MadeFrom::Binding(binding_name("ghost-view")?), &[])?;
    let published = store
        .publish(page("home", "one", TASKS)?, unresolved_binding)
        .await;
    let pages = unexpected(store.pages().await)?;
    ensure(
        published == Err(PageStoreError::UnresolvedOrigin) && pages.is_empty(),
        clause,
        || format!("publish from a missing binding answered {published:?}, pages {pages:?}"),
    )?;

    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let from_page = contract_fixture.provenance(MadeFrom::Page(pin("home", v1.as_str())?), &[])?;
    unexpected(
        store
            .publish(page("home", "two", TASKS)?, from_page.clone())
            .await,
    )?;
    let missing_version = contract_fixture.provenance(MadeFrom::Page(pin("home", "nope")?), &[])?;
    let published = store
        .publish(page("home", "three", TASKS)?, missing_version)
        .await;
    let versions = unexpected(store.versions(&page_name("home")?).await)?;
    ensure(
        published == Err(PageStoreError::UnresolvedOrigin) && versions.len() == 2,
        clause,
        || format!("publish from a missing version answered {published:?}, versions {versions:?}"),
    )?;

    let unresolved_version = binding(
        "my-view",
        pin("home", v1.as_str())?,
        &[("tasks", TASK_A)],
        contract_fixture.provenance(MadeFrom::Page(pin("home", "nope")?), &[])?,
    )?;
    let saved = store.save_binding(unresolved_version, records).await;
    let stored = store.binding(&binding_name("my-view")?).await;
    ensure(
        saved == Err(PageStoreError::UnresolvedOrigin) && is_not_found(&stored),
        clause,
        || format!("save from a missing version answered {saved:?}, stored {stored:?}"),
    )?;
    let unresolved = binding(
        "my-view",
        pin("home", v1.as_str())?,
        &[("tasks", TASK_A)],
        contract_fixture.provenance(MadeFrom::Binding(binding_name("ghost-view")?), &[])?,
    )?;
    let saved = store.save_binding(unresolved, records).await;
    let stored = store.binding(&binding_name("my-view")?).await;
    ensure(
        saved == Err(PageStoreError::UnresolvedOrigin) && is_not_found(&stored),
        clause,
        || format!("save from a missing binding answered {saved:?}, stored {stored:?}"),
    )?;
    let resolved = binding(
        "my-view",
        pin("home", v1.as_str())?,
        &[("tasks", TASK_A)],
        from_page,
    )?;
    unexpected(store.save_binding(resolved, records).await)
        .map_err(|_| Violation::new(clause, "a binding made from an existing page was refused"))
}

/// A saved binding reads back with its pin, fills and provenance, and its
/// page lists it.
///
/// # Errors
///
/// Returns [`Clause::ABindingRoundTrips`] otherwise.
pub async fn a_binding_round_trips<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS_AND_NOTES).await?;
    let provenance =
        contract_fixture.provenance(MadeFrom::Page(pin("home", v1.as_str())?), &[TASK_A, NOTE])?;
    let saved = binding(
        "my-view",
        pin("home", v1.as_str())?,
        &[("tasks", TASK_A), ("notes", NOTE)],
        provenance,
    )?;
    unexpected(store.save_binding(saved.clone(), records).await)?;
    let read = unexpected(store.binding(&binding_name("my-view")?).await)?;
    let listed = unexpected(store.bindings_of(&page_name("home")?).await)?;
    ensure(
        read == saved && listed == [saved.clone()],
        Clause::ABindingRoundTrips,
        || format!("saved {saved:?}, read {read:?}, listed {listed:?}"),
    )
}

/// A taken binding name and an unknown pinned version are refused with
/// `NameTaken` and `UnknownVersion`, and change nothing.
///
/// # Errors
///
/// Returns [`Clause::ATakenNameAndAnUnknownVersionAreRefused`] otherwise.
pub async fn a_taken_name_and_an_unknown_version_are_refused<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS_AND_NOTES).await?;
    let first = bind(
        store,
        records,
        contract_fixture,
        "my-view",
        v1.as_str(),
        &[("tasks", TASK_A)],
    )
    .await?;
    let again = binding(
        "my-view",
        pin("home", v1.as_str())?,
        &[("notes", NOTE)],
        contract_fixture.original()?,
    )?;
    let taken = store.save_binding(again, records).await;
    let unknown_version = store
        .save_binding(
            binding(
                "other-view",
                pin("home", "nope")?,
                &[("tasks", TASK_A)],
                contract_fixture.original()?,
            )?,
            records,
        )
        .await;
    let unknown_page = store
        .save_binding(
            binding(
                "third-view",
                pin("ghost", "v1")?,
                &[("tasks", TASK_A)],
                contract_fixture.original()?,
            )?,
            records,
        )
        .await;
    let kept = unexpected(store.binding(&binding_name("my-view")?).await)?;
    let other = store.binding(&binding_name("other-view")?).await;
    let third = store.binding(&binding_name("third-view")?).await;
    ensure(
        taken == Err(PageStoreError::NameTaken)
            && unknown_version == Err(PageStoreError::UnknownVersion)
            && unknown_page == Err(PageStoreError::UnknownVersion)
            && kept == first
            && is_not_found(&other)
            && is_not_found(&third),
        Clause::ATakenNameAndAnUnknownVersionAreRefused,
        || {
            format!(
                "taken {taken:?}, unknown version {unknown_version:?}, unknown page {unknown_page:?}"
            )
        },
    )
}

/// A binding that fills an undeclared slot, fills a slot with a record of
/// another type, or names a record that does not exist is refused with
/// `Misfit` and not saved.
///
/// # Errors
///
/// Returns [`Clause::AMisfitIsRefusedOnSave`] otherwise.
pub async fn a_misfit_is_refused_on_save<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let cases = [
        (
            "gone-view",
            ("gone", TASK_A),
            Misfit::UndeclaredSlot {
                slot: fixture(SlotName::try_from("gone"))?,
            },
        ),
        (
            "wrong-view",
            ("tasks", NOTE),
            Misfit::WrongType {
                slot: fixture(SlotName::try_from("tasks"))?,
                expected: fixture(RecordType::try_from(TASK_TYPE))?,
                found: fixture(RecordType::try_from(NOTE_TYPE))?,
            },
        ),
        (
            "missing-view",
            ("tasks", MISSING),
            Misfit::MissingRecord {
                slot: fixture(SlotName::try_from("tasks"))?,
                path: fixture(RecordPath::try_from(MISSING))?,
            },
        ),
    ];
    for (name, fill, expected) in cases {
        let refused = binding(
            name,
            pin("home", v1.as_str())?,
            &[fill],
            contract_fixture.original()?,
        )?;
        let answer = store.save_binding(refused, records).await;
        let stored = store.binding(&binding_name(name)?).await;
        ensure(
            answer == Err(PageStoreError::Misfit(expected.clone())) && is_not_found(&stored),
            Clause::AMisfitIsRefusedOnSave,
            || {
                format!(
                    "{name}: expected {expected:?}, save answered {answer:?}, stored {stored:?}"
                )
            },
        )?;
    }
    Ok(())
}
