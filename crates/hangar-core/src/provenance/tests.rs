//! Unit tests of provenance.

use super::*;
use crate::name::{PageName, PageVersion, PrincipalId};

fn principal(text: &str) -> Principal {
    Principal::new(PrincipalId::try_from(text).unwrap())
}

fn source(index: usize) -> RecordRef {
    RecordRef::new(RecordPath::try_from(format!("/tasks/{index}").as_str()).unwrap())
}

#[test]
fn provenance_allows_an_empty_list_of_sources_when_it_is_stated() {
    let provenance = Provenance::new(principal("ada"), MadeFrom::Original, Vec::new()).unwrap();
    assert!(provenance.sources().is_empty());
    assert_eq!(provenance.made_by(), &principal("ada"));
    assert_eq!(provenance.made_from(), &MadeFrom::Original);
}

#[test]
fn provenance_allows_64_sources_and_refuses_a_65th() {
    let make = |count: usize| (0..count).map(source).collect::<Vec<_>>();
    assert!(Provenance::new(principal("ada"), MadeFrom::Original, make(64)).is_ok());
    assert_eq!(
        Provenance::new(principal("ada"), MadeFrom::Original, make(65)),
        Err(ParseError::TooMany {
            subject: Subject::Sources,
            max: 64
        })
    );
}

#[test]
fn provenance_keeps_the_origin_and_the_sources_it_was_given() {
    let pin = PagePin::new(
        PageName::try_from("home").unwrap(),
        PageVersion::try_from("v2").unwrap(),
    );
    let provenance = Provenance::new(
        principal("agent"),
        MadeFrom::Page(pin.clone()),
        vec![source(1), source(2)],
    )
    .unwrap();
    assert_eq!(provenance.made_from(), &MadeFrom::Page(pin));
    assert_eq!(provenance.sources(), [source(1), source(2)]);
}

#[test]
fn write_provenance_keeps_the_blob_the_maker_and_the_binding() {
    let based_on = Blob::try_from("1".repeat(40).as_str()).unwrap();
    let through = BindingName::try_from("my-view").unwrap();
    let write = WriteProvenance::new(based_on.clone(), principal("ada"), through.clone());
    assert_eq!(write.based_on(), &based_on);
    assert_eq!(write.made_by(), &principal("ada"));
    assert_eq!(write.through(), &through);
}
