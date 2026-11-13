//! The [`PageStore`] contract suite.
//!
//! The suite calls `save_binding` and `move_binding` with a [`RecordStore`]
//! that the caller seeds with the fixture's records. This module holds the
//! fixture, the helpers and the clauses about pages. The submodules hold the
//! clauses about bindings, about moving them, about concurrent calls and about
//! the rules that guard a move, and this module re-exports their functions.

mod bindings;
mod guards;
mod moves;
mod races;

pub use bindings::{
    a_binding_round_trips, a_misfit_is_refused_on_save,
    a_taken_name_and_an_unknown_version_are_refused, an_unresolved_origin_is_refused,
};
pub use guards::{
    a_move_records_its_mover, check_fit_sees_a_vanished_record, move_binding_refuses_a_stale_pin,
    version_names_are_never_reused,
};
pub use moves::{
    a_republish_leaves_a_pin_unchanged, bindings_of_lists_a_pages_bindings,
    move_binding_changes_only_the_pin, move_binding_refuses_a_misfit,
    move_binding_refuses_a_vanished_record,
};
pub use races::{
    concurrent_moves_from_one_pin_give_one_moved, concurrent_publishes_get_two_versions,
    concurrent_saves_of_one_name_give_one_name_taken,
};

use super::{Clause, SeedRecord, Violation, ensure, fixture, seed, unexpected};
use crate::{
    Binding, BindingName, Blob, ContentFormat, Fills, MadeFrom, MoveError, Page, PageContent,
    PageName, PagePin, PageStore, PageStoreError, PageVersion, Principal, PrincipalId, Provenance,
    RecordRef, RecordStore, Slot, SlotName, Slots, WriteProvenance,
};
use crate::{RecordPath, RecordType};

const TASK_TYPE: &str = "tasks/task/v2";
const NOTE_TYPE: &str = "notes/note/v1";
const TASK_A: &str = "/tasks/a";
const NOTE: &str = "/notes/n";
const MISSING: &str = "/tasks/missing";

/// The records and values that the suite uses.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PageFixture {
    seeds: Vec<SeedRecord>,
    principal: Principal,
}

impl PageFixture {
    /// The standard fixture.
    ///
    /// # Errors
    ///
    /// Returns a [`Violation`] if a constant of the fixture is refused by its
    /// type, which means this crate is broken.
    pub fn standard() -> Result<Self, Violation> {
        Ok(Self {
            seeds: vec![
                seed(TASK_A, TASK_TYPE, "title: first\n")?,
                seed("/tasks/b", TASK_TYPE, "title: second\n")?,
                seed(NOTE, NOTE_TYPE, "text: a note\n")?,
            ],
            principal: Principal::new(fixture(PrincipalId::try_from("contract-suite"))?),
        })
    }

    /// The records to seed into the [`RecordStore`] before running a
    /// function: two of type `tasks/task/v2` and one of `notes/note/v1`.
    #[must_use]
    pub fn seeds(&self) -> &[SeedRecord] {
        &self.seeds
    }

    fn provenance(&self, made_from: MadeFrom, sources: &[&str]) -> Result<Provenance, Violation> {
        let mut refs = Vec::new();
        for source in sources {
            refs.push(RecordRef::new(fixture(RecordPath::try_from(*source))?));
        }
        fixture(Provenance::new(self.principal.clone(), made_from, refs))
    }

    fn original(&self) -> Result<Provenance, Violation> {
        self.provenance(MadeFrom::Original, &[])
    }

    /// The provenance of a move of the binding `through`, made by the
    /// fixture's principal.
    fn mover(&self, through: &str) -> Result<WriteProvenance, Violation> {
        Ok(WriteProvenance::new(
            fixture(Blob::try_from("a".repeat(40).as_str()))?,
            self.principal.clone(),
            binding_name(through)?,
        ))
    }
}

/// Moves the binding `name` from the version `expected` to `target`, as the
/// fixture's mover. The outer result is a fixture failure and the inner
/// result is the store's answer.
async fn move_to<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
    name: &str,
    expected: &PageVersion,
    target: &PageVersion,
) -> Result<Result<(), MoveError>, Violation> {
    Ok(store
        .move_binding(
            &binding_name(name)?,
            expected,
            target,
            records,
            &contract_fixture.mover(name)?,
        )
        .await)
}

fn page_name(text: &str) -> Result<PageName, Violation> {
    fixture(PageName::try_from(text))
}

fn binding_name(text: &str) -> Result<BindingName, Violation> {
    fixture(BindingName::try_from(text))
}

fn version(text: &str) -> Result<PageVersion, Violation> {
    fixture(PageVersion::try_from(text))
}

fn pin(name: &str, version_text: &str) -> Result<PagePin, Violation> {
    Ok(PagePin::new(page_name(name)?, version(version_text)?))
}

/// A page with `text` as HTML content and one slot per `(name, type)` pair.
fn page(name: &str, text: &str, slots: &[(&str, &str)]) -> Result<Page, Violation> {
    let mut built = Vec::new();
    for (slot, record_type) in slots {
        built.push(fixture(Slot::new(
            fixture(SlotName::try_from(*slot))?,
            fixture(RecordType::try_from(*record_type))?,
            Vec::new(),
            None,
        ))?);
    }
    Ok(Page::new(
        page_name(name)?,
        fixture(PageContent::new(ContentFormat::Html, text))?,
        fixture(Slots::new(built))?,
    ))
}

/// A binding named `name` of `pinned`, filling each `(slot, path)` pair.
fn binding(
    name: &str,
    pinned: PagePin,
    fills: &[(&str, &str)],
    provenance: Provenance,
) -> Result<Binding, Violation> {
    let mut entries = Vec::new();
    for (slot, path) in fills {
        entries.push((
            fixture(SlotName::try_from(*slot))?,
            fixture(RecordPath::try_from(*path))?,
        ));
    }
    Ok(Binding::new(
        binding_name(name)?,
        pinned,
        fixture(Fills::new(entries))?,
        provenance,
    ))
}

/// Publishes a page with `Original` provenance.
async fn publish<S: PageStore>(
    store: &S,
    contract_fixture: &PageFixture,
    name: &str,
    text: &str,
    slots: &[(&str, &str)],
) -> Result<PageVersion, Violation> {
    unexpected(
        store
            .publish(page(name, text, slots)?, contract_fixture.original()?)
            .await,
    )
}

/// Saves a binding of `home` at `version_text` with `Original` provenance.
async fn bind<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
    name: &str,
    version_text: &str,
    fills: &[(&str, &str)],
) -> Result<Binding, Violation> {
    let saved = binding(
        name,
        pin("home", version_text)?,
        fills,
        contract_fixture.original()?,
    )?;
    unexpected(store.save_binding(saved.clone(), records).await)?;
    Ok(saved)
}

fn is_not_found<T>(result: &Result<T, PageStoreError>) -> bool {
    matches!(result, Err(PageStoreError::NotFound))
}

async fn pin_of<S: PageStore>(store: &S, name: &str) -> Result<PageVersion, Violation> {
    let found = unexpected(store.binding(&binding_name(name)?).await)?;
    Ok(found.pin().version().clone())
}

const TASKS: &[(&str, &str)] = &[("tasks", TASK_TYPE)];
const TASKS_AND_NOTES: &[(&str, &str)] = &[("tasks", TASK_TYPE), ("notes", NOTE_TYPE)];

/// Publishing a page then asking for its latest version returns the page
/// with the version and the provenance that were given.
///
/// # Errors
///
/// Returns [`Clause::PublishThenLatestReturnsThePage`] otherwise.
pub async fn publish_then_latest_returns_the_page<S: PageStore, R: RecordStore>(
    store: &S,
    _records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let page = page("home", "<p>hi</p>", TASKS)?;
    let provenance = contract_fixture.provenance(MadeFrom::Original, &[TASK_A])?;
    let version = unexpected(store.publish(page.clone(), provenance.clone()).await)?;
    let latest = unexpected(store.latest(page.name()).await)?;
    ensure(
        latest.page() == &page
            && latest.version() == &version
            && latest.provenance() == &provenance,
        Clause::PublishThenLatestReturnsThePage,
        || format!("{latest:?}"),
    )
}

/// A republish adds a version, `versions` lists newest first, `latest`
/// returns the newest and `at` returns the older content.
///
/// # Errors
///
/// Returns [`Clause::RepublishAddsAVersionNewestFirst`] otherwise.
pub async fn republish_adds_a_version_newest_first<S: PageStore, R: RecordStore>(
    store: &S,
    _records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let first = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let second = publish(store, contract_fixture, "home", "two", TASKS).await?;
    let versions = unexpected(store.versions(&page_name("home")?).await)?;
    let latest = unexpected(store.latest(&page_name("home")?).await)?;
    let older = unexpected(store.at(&pin("home", first.as_str())?).await)?;
    ensure(
        first != second
            && versions == [second.clone(), first.clone()]
            && latest.page().content().text() == "two"
            && latest.version() == &second
            && older.page().content().text() == "one",
        Clause::RepublishAddsAVersionNewestFirst,
        || format!("versions {versions:?}, latest {latest:?}, older {older:?}"),
    )
}

/// The page list names each published page.
///
/// # Errors
///
/// Returns [`Clause::PagesListsPublishedNames`] otherwise.
pub async fn pages_lists_published_names<S: PageStore, R: RecordStore>(
    store: &S,
    _records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    publish(store, contract_fixture, "alpha", "a", TASKS).await?;
    publish(store, contract_fixture, "beta", "b", TASKS).await?;
    publish(store, contract_fixture, "alpha", "a2", TASKS).await?;
    let names = unexpected(store.pages().await)?;
    ensure(
        names.len() == 2
            && names.contains(&page_name("alpha")?)
            && names.contains(&page_name("beta")?),
        Clause::PagesListsPublishedNames,
        || format!("{names:?}"),
    )
}

/// An unknown page, version or binding is `NotFound`.
///
/// # Errors
///
/// Returns [`Clause::UnknownNamesAreNotFound`] otherwise.
pub async fn unknown_names_are_not_found<S: PageStore, R: RecordStore>(
    store: &S,
    _records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let ghost = page_name("ghost")?;
    let before_publish = [
        is_not_found(&store.latest(&ghost).await),
        is_not_found(&store.versions(&ghost).await),
        is_not_found(&store.at(&pin("ghost", "v1")?).await),
        is_not_found(&store.binding(&binding_name("ghost-view")?).await),
        is_not_found(&store.bindings_of(&ghost).await),
    ];
    publish(store, contract_fixture, "home", "one", TASKS).await?;
    let unknown_version = is_not_found(&store.at(&pin("home", "nope")?).await);
    ensure(
        before_publish.iter().all(|found| *found) && unknown_version,
        Clause::UnknownNamesAreNotFound,
        || format!("{before_publish:?}, unknown version {unknown_version}"),
    )
}

/// `latest`, `at` and `binding` return the provenance that `publish` and
/// `save_binding` were given, unchanged, including `Original` with no
/// sources, and sources that name no existing record.
///
/// # Errors
///
/// Returns [`Clause::ProvenanceIsReturnedUnchanged`] otherwise.
pub async fn provenance_is_returned_unchanged<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
) -> Result<(), Violation> {
    let first = contract_fixture.provenance(MadeFrom::Original, &[TASK_A, "/elsewhere/gone"])?;
    let v1 = unexpected(
        store
            .publish(page("home", "one", TASKS)?, first.clone())
            .await,
    )?;
    let second = contract_fixture.original()?;
    unexpected(
        store
            .publish(page("home", "two", TASKS)?, second.clone())
            .await,
    )?;
    let from_page =
        contract_fixture.provenance(MadeFrom::Page(pin("home", v1.as_str())?), &[TASK_A])?;
    let saved = binding(
        "first-view",
        pin("home", v1.as_str())?,
        &[("tasks", TASK_A)],
        from_page.clone(),
    )?;
    unexpected(store.save_binding(saved, records).await)?;
    let from_binding =
        contract_fixture.provenance(MadeFrom::Binding(binding_name("first-view")?), &[])?;
    let derived = binding(
        "second-view",
        pin("home", v1.as_str())?,
        &[("tasks", TASK_A)],
        from_binding.clone(),
    )?;
    unexpected(store.save_binding(derived, records).await)?;

    let latest = unexpected(store.latest(&page_name("home")?).await)?;
    let older = unexpected(store.at(&pin("home", v1.as_str())?).await)?;
    let one = unexpected(store.binding(&binding_name("first-view")?).await)?;
    let two = unexpected(store.binding(&binding_name("second-view")?).await)?;
    ensure(
        latest.provenance() == &second
            && older.provenance() == &first
            && one.provenance() == &from_page
            && two.provenance() == &from_binding,
        Clause::ProvenanceIsReturnedUnchanged,
        || format!("latest {latest:?}, older {older:?}, bindings {one:?} and {two:?}"),
    )
}
