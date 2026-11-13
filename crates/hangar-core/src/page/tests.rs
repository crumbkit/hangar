//! Unit tests of pages, slots, fills and the fit rule.

use super::*;
use crate::name::PageVersion;

fn slot_name(text: &str) -> SlotName {
    SlotName::try_from(text).unwrap()
}

fn record_type(text: &str) -> RecordType {
    RecordType::try_from(text).unwrap()
}

fn path(text: &str) -> RecordPath {
    RecordPath::try_from(text).unwrap()
}

fn slot(name: &str, kind: &str) -> Slot {
    Slot::new(slot_name(name), record_type(kind), Vec::new(), None).unwrap()
}

fn page_with(slots: Vec<Slot>) -> Page {
    Page::new(
        PageName::try_from("home").unwrap(),
        PageContent::new(ContentFormat::Html, "<p>hi</p>").unwrap(),
        Slots::new(slots).unwrap(),
    )
}

#[test]
fn page_content_holds_at_most_one_mebibyte_of_text() {
    assert!(PageContent::new(ContentFormat::Markdown, "x".repeat(1024 * 1024)).is_ok());
    assert!(PageContent::new(ContentFormat::Html, "x".repeat(1024 * 1024 + 1)).is_err());
}

#[test]
fn page_content_keeps_its_format_and_text() {
    let content = PageContent::new(ContentFormat::Markdown, "# hi").unwrap();
    assert_eq!(content.format(), ContentFormat::Markdown);
    assert_eq!(content.text(), "# hi");
}

#[test]
fn a_slot_sample_must_have_the_slots_record_type() {
    let sample = Sample::new(
        record_type("notes/note/v1"),
        RecordBody::try_from("x").unwrap(),
    );
    let refused = Slot::new(
        slot_name("tasks"),
        record_type("tasks/task/v2"),
        Vec::new(),
        Some(sample),
    );
    assert!(refused.is_err());

    let sample = Sample::new(
        record_type("tasks/task/v2"),
        RecordBody::try_from("x").unwrap(),
    );
    let accepted = Slot::new(
        slot_name("tasks"),
        record_type("tasks/task/v2"),
        vec![FieldName::try_from("title").unwrap()],
        Some(sample),
    )
    .unwrap();
    assert_eq!(accepted.editable_fields().len(), 1);
    assert!(accepted.sample().is_some());
    assert_eq!(accepted.name().as_str(), "tasks");
    assert_eq!(accepted.record_type().as_str(), "tasks/task/v2");
}

#[test]
fn slots_allow_32_slots_and_refuse_a_33rd() {
    let make = |count: usize| {
        (0..count)
            .map(|index| slot(&format!("s{index}"), "tasks/task/v2"))
            .collect::<Vec<_>>()
    };
    assert!(Slots::new(make(32)).is_ok());
    assert_eq!(
        Slots::new(make(33)),
        Err(ParseError::TooMany {
            subject: Subject::Slots,
            max: 32
        })
    );
}

#[test]
fn slots_refuse_two_slots_with_one_name() {
    let refused = Slots::new(vec![
        slot("tasks", "tasks/task/v2"),
        slot("tasks", "notes/note/v1"),
    ]);
    assert_eq!(
        refused,
        Err(ParseError::Duplicate {
            subject: Subject::Slots
        })
    );
}

#[test]
fn slots_find_a_slot_by_name() {
    let slots = Slots::new(vec![slot("tasks", "tasks/task/v2")]).unwrap();
    assert!(slots.get(&slot_name("tasks")).is_some());
    assert!(slots.get(&slot_name("other")).is_none());
    assert_eq!(slots.len(), 1);
    assert!(!slots.is_empty());
}

#[test]
fn fills_need_at_least_one_entry() {
    let empty: Vec<(SlotName, RecordPath)> = Vec::new();
    assert_eq!(
        Fills::new(empty),
        Err(ParseError::Empty {
            subject: Subject::Fills
        })
    );
}

#[test]
fn fills_refuse_a_slot_named_twice() {
    let refused = Fills::new([
        (slot_name("tasks"), path("/tasks/a")),
        (slot_name("tasks"), path("/tasks/b")),
    ]);
    assert_eq!(
        refused,
        Err(ParseError::Duplicate {
            subject: Subject::Fills
        })
    );
}

#[test]
fn fills_list_their_entries_in_slot_name_order() {
    let fills = Fills::new([
        (slot_name("b"), path("/tasks/b")),
        (slot_name("a"), path("/tasks/a")),
    ])
    .unwrap();
    let names: Vec<&str> = fills.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(fills.get(&slot_name("a")), Some(&path("/tasks/a")));
}

#[test]
fn check_fit_passes_when_each_fill_names_a_slot_of_the_records_type() {
    let page = page_with(vec![slot("tasks", "tasks/task/v2")]);
    let name = slot_name("tasks");
    let kind = record_type("tasks/task/v2");
    assert_eq!(page.check_fit([(&name, &kind)]), Ok(()));
}

#[test]
fn check_fit_allows_a_slot_to_stay_unfilled() {
    let page = page_with(vec![
        slot("tasks", "tasks/task/v2"),
        slot("notes", "notes/note/v1"),
    ]);
    let name = slot_name("tasks");
    let kind = record_type("tasks/task/v2");
    assert_eq!(page.check_fit([(&name, &kind)]), Ok(()));
}

#[test]
fn check_fit_names_a_slot_the_page_does_not_declare() {
    let page = page_with(vec![slot("tasks", "tasks/task/v2")]);
    let name = slot_name("gone");
    let kind = record_type("tasks/task/v2");
    assert_eq!(
        page.check_fit([(&name, &kind)]),
        Err(Misfit::UndeclaredSlot { slot: name.clone() })
    );
}

#[test]
fn check_fit_names_a_record_of_the_wrong_type() {
    let page = page_with(vec![slot("tasks", "tasks/task/v2")]);
    let name = slot_name("tasks");
    let found = record_type("notes/note/v1");
    assert_eq!(
        page.check_fit([(&name, &found)]),
        Err(Misfit::WrongType {
            slot: name.clone(),
            expected: record_type("tasks/task/v2"),
            found: found.clone(),
        })
    );
}

#[test]
fn a_page_pin_keeps_its_name_and_version() {
    let pin = PagePin::new(
        PageName::try_from("home").unwrap(),
        PageVersion::try_from("v1").unwrap(),
    );
    assert_eq!(pin.name().as_str(), "home");
    assert_eq!(pin.version().as_str(), "v1");
}
