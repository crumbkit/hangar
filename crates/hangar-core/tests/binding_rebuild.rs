//! A store outside this crate rebuilds a binding from what it stored, last
//! move included, using only the public API.
#![cfg(test)]

use hangar_core::{
    Binding, BindingName, Blob, Fills, MadeFrom, PageName, PagePin, PageVersion, Principal,
    PrincipalId, Provenance, RecordPath, SlotName, WriteProvenance,
};

fn saved() -> Binding {
    let fills = Fills::new([(
        SlotName::try_from("tasks").unwrap(),
        RecordPath::try_from("/tasks/a").unwrap(),
    )])
    .unwrap();
    let principal = Principal::new(PrincipalId::try_from("saver").unwrap());
    Binding::new(
        BindingName::try_from("my-view").unwrap(),
        PagePin::new(
            PageName::try_from("home").unwrap(),
            PageVersion::try_from("v1").unwrap(),
        ),
        fills,
        Provenance::new(principal, MadeFrom::Original, Vec::new()).unwrap(),
    )
}

#[test]
fn a_binding_rebuilt_with_its_last_move_reads_back_that_move() {
    let mover = WriteProvenance::new(
        Blob::try_from("a".repeat(40).as_str()).unwrap(),
        Principal::new(PrincipalId::try_from("mover").unwrap()),
        BindingName::try_from("my-view").unwrap(),
    );
    let rebuilt = saved().with_last_move(mover.clone());
    assert_eq!(rebuilt.last_move(), Some(&mover));
    assert_eq!(rebuilt.provenance(), saved().provenance());
    assert_eq!(saved().last_move(), None);
}
