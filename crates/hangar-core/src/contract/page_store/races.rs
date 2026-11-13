//! The [`PageStore`] clauses about calls that run together. Each clause
//! starts two threads that leave a barrier at the same moment and each run
//! one call, as [`contract`](crate::contract) describes.

use super::{PageFixture, TASK_A, TASKS, bind, binding, binding_name, page, pin, pin_of, publish};
use crate::contract::{Clause, RunTogether, Violation, ensure, unexpected};
use crate::{MadeFrom, MoveError, PageStore, PageStoreError, RecordStore};

/// How many pairs of calls the clauses that can repeat run.
const ROUNDS: usize = 8;

/// Two publishes of one page that run together give two different versions,
/// and the page lists every version that a publish returned. The clause runs
/// several pairs, and `runner` drives each pair.
///
/// # Errors
///
/// Returns [`Clause::ConcurrentPublishesGetTwoVersions`] otherwise.
pub async fn concurrent_publishes_get_two_versions<S: PageStore, R: RecordStore>(
    store: &S,
    _records: &R,
    contract_fixture: &PageFixture,
    runner: &impl RunTogether,
) -> Result<(), Violation> {
    let clause = Clause::ConcurrentPublishesGetTwoVersions;
    let mut returned = Vec::new();
    for round in 0..ROUNDS {
        let left = (page("home", "left", TASKS)?, contract_fixture.original()?);
        let right = (page("home", "right", TASKS)?, contract_fixture.original()?);
        let answers = runner
            .run_together(
                move || store.publish(left.0, left.1),
                move || store.publish(right.0, right.1),
            )
            .await;
        let (first, second) = unexpected(answers.ok_or("a concurrent publish panicked"))?;
        let (first, second) = (unexpected(first)?, unexpected(second)?);
        ensure(first != second, clause, || {
            format!("round {round}: both publishes returned {first:?}")
        })?;
        returned.push(first);
        returned.push(second);
    }
    let listed = unexpected(store.versions(&super::page_name("home")?).await)?;
    ensure(
        listed.len() == returned.len() && returned.iter().all(|version| listed.contains(version)),
        clause,
        || format!("publishes returned {returned:?}, the page lists {listed:?}"),
    )
}

/// Two saves of one binding name that run together give one success and one
/// `NameTaken`, and the stored binding is the one whose save succeeded. The
/// clause runs several pairs, each with a new name, and `runner` drives each
/// pair.
///
/// # Errors
///
/// Returns [`Clause::ConcurrentSavesOfOneNameGiveOneNameTaken`] otherwise.
pub async fn concurrent_saves_of_one_name_give_one_name_taken<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
    runner: &impl RunTogether,
) -> Result<(), Violation> {
    let clause = Clause::ConcurrentSavesOfOneNameGiveOneNameTaken;
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    for round in 0..ROUNDS {
        let name = format!("race-{round}");
        let from_nothing = binding(
            &name,
            pin("home", v1.as_str())?,
            &[("tasks", TASK_A)],
            contract_fixture.original()?,
        )?;
        let from_the_page = binding(
            &name,
            pin("home", v1.as_str())?,
            &[("tasks", TASK_A)],
            contract_fixture.provenance(MadeFrom::Page(pin("home", v1.as_str())?), &[])?,
        )?;
        let (left, right) = (from_nothing.clone(), from_the_page.clone());
        let answers = runner
            .run_together(
                move || store.save_binding(left, records),
                move || store.save_binding(right, records),
            )
            .await;
        let (first, second) = unexpected(answers.ok_or("a concurrent save panicked"))?;
        let stored = unexpected(store.binding(&binding_name(&name)?).await)?;
        let winner = match (&first, &second) {
            (Ok(()), Err(PageStoreError::NameTaken)) => Some(&from_nothing),
            (Err(PageStoreError::NameTaken), Ok(())) => Some(&from_the_page),
            _ => None,
        };
        ensure(winner.is_some_and(|saved| *saved == stored), clause, || {
            format!("round {round}: answers {first:?} and {second:?}, stored {stored:?}")
        })?;
    }
    Ok(())
}

/// Two moves of one binding from one pin that run together give one success
/// and one [`MoveError::Moved`] that names the version the winner moved to,
/// and the binding is pinned there. The clause runs several pairs, each on a
/// new binding, and `runner` drives each pair.
///
/// # Errors
///
/// Returns [`Clause::ConcurrentMovesFromOnePinGiveOneMoved`] otherwise.
pub async fn concurrent_moves_from_one_pin_give_one_moved<S: PageStore, R: RecordStore>(
    store: &S,
    records: &R,
    contract_fixture: &PageFixture,
    runner: &impl RunTogether,
) -> Result<(), Violation> {
    let clause = Clause::ConcurrentMovesFromOnePinGiveOneMoved;
    let v1 = publish(store, contract_fixture, "home", "one", TASKS).await?;
    let v2 = publish(store, contract_fixture, "home", "two", TASKS).await?;
    let v3 = publish(store, contract_fixture, "home", "three", TASKS).await?;
    for round in 0..ROUNDS {
        let view = format!("race-{round}");
        bind(
            store,
            records,
            contract_fixture,
            &view,
            v1.as_str(),
            &[("tasks", TASK_A)],
        )
        .await?;
        let name = binding_name(&view)?;
        let mover = contract_fixture.mover(&view)?;
        let answers = runner
            .run_together(
                || store.move_binding(&name, &v1, &v2, records, &mover),
                || store.move_binding(&name, &v1, &v3, records, &mover),
            )
            .await;
        let (first, second) = unexpected(answers.ok_or("a concurrent move panicked"))?;
        let pinned = pin_of(store, &view).await?;
        let winner = match (&first, &second) {
            (Ok(()), Err(MoveError::Moved { current })) if *current == v2 => Some(&v2),
            (Err(MoveError::Moved { current }), Ok(())) if *current == v3 => Some(&v3),
            _ => None,
        };
        ensure(winner == Some(&pinned), clause, || {
            format!(
                "round {round}: answers {first:?} and {second:?}, the binding is pinned to {pinned:?}"
            )
        })?;
    }
    Ok(())
}
