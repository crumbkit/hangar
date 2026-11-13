//! Unit tests of the record types.

use super::*;

#[test]
fn a_record_path_accepts_absolute_paths_of_plain_segments() {
    for text in ["/a", "/tasks/x.md", "/a/b/c", "/with space/ok", "/a%2Fb"] {
        assert!(RecordPath::try_from(text).is_ok(), "{text}");
    }
}

#[test]
fn a_record_path_refuses_relative_empty_and_traversing_spellings() {
    for text in [
        "",
        "a/b",
        "/",
        "/a//b",
        "/a/",
        "//a",
        "/a/./b",
        "/./a",
        "/a/..",
        "/../etc/passwd",
        "/a/../b",
        "/..",
        "/.",
        "/a\\b",
        "\\a",
        "/a\0b",
        "/a/b\0",
    ] {
        assert!(RecordPath::try_from(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_record_path_refuses_control_characters() {
    for text in [
        "/a\nb",
        "/a\tb",
        "/a\r",
        "/a\u{7f}",
        "/a\u{1b}[0m",
        "/a\u{85}",
    ] {
        assert!(RecordPath::try_from(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_record_path_with_printable_characters_is_accepted() {
    for text in ["/a b", "/tasks/a-1", "/é/x", "/a.b/c_d"] {
        assert!(RecordPath::try_from(text).is_ok(), "{text:?}");
    }
}

#[test]
fn a_record_path_is_at_most_1024_bytes() {
    let at_limit = format!("/{}", "a".repeat(1023));
    let over = format!("/{}", "a".repeat(1024));
    assert!(RecordPath::try_from(at_limit.as_str()).is_ok());
    assert_eq!(
        RecordPath::try_from(over.as_str()),
        Err(ParseError::TooLong {
            subject: Subject::RecordPath,
            max: 1024
        })
    );
}

#[test]
fn a_record_path_displays_as_its_text() {
    let path = RecordPath::try_from("/tasks/a").unwrap();
    assert_eq!(path.to_string(), "/tasks/a");
    assert_eq!(path.as_str(), "/tasks/a");
}

#[test]
fn a_record_type_is_slash_joined_segments_of_lower_case_letters_digits_and_hyphens() {
    for text in ["tasks/task/v2", "a", "a-b/c-d", "x/1"] {
        assert!(RecordType::try_from(text).is_ok(), "{text}");
    }
    for text in ["", "/a", "a/", "a//b", "A/b", "a_b/c", "a b", "a.b", "é/x"] {
        assert!(RecordType::try_from(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_record_type_is_at_most_128_bytes() {
    assert!(RecordType::try_from("a".repeat(128).as_str()).is_ok());
    assert!(RecordType::try_from("a".repeat(129).as_str()).is_err());
}

#[test]
fn a_blob_is_40_or_64_lower_case_hex_digits() {
    for count in [40, 64] {
        assert!(Blob::try_from("a1".repeat(count / 2).as_str()).is_ok());
    }
    for text in [
        "a".repeat(39),
        "a".repeat(41),
        "a".repeat(63),
        "a".repeat(65),
        String::new(),
        "A".repeat(40),
        "g".repeat(40),
        " ".repeat(40),
    ] {
        assert!(Blob::try_from(text.as_str()).is_err(), "{text:?}");
    }
}

#[test]
fn a_record_body_holds_at_most_one_mebibyte() {
    assert!(RecordBody::try_from("x".repeat(1024 * 1024).as_str()).is_ok());
    assert_eq!(
        RecordBody::try_from("x".repeat(1024 * 1024 + 1).as_str()),
        Err(ParseError::TooLong {
            subject: Subject::RecordBody,
            max: 1024 * 1024
        })
    );
    assert!(RecordBody::try_from("").is_ok());
}

#[test]
fn a_record_keeps_the_parts_it_was_built_from() {
    let record = Record::new(
        RecordPath::try_from("/tasks/a").unwrap(),
        RecordType::try_from("tasks/task/v2").unwrap(),
        Blob::try_from("0".repeat(40).as_str()).unwrap(),
        RecordBody::try_from("title: a").unwrap(),
    );
    assert_eq!(record.path().as_str(), "/tasks/a");
    assert_eq!(record.record_type().as_str(), "tasks/task/v2");
    assert_eq!(record.blob().as_str(), "0".repeat(40));
    assert_eq!(record.body().as_str(), "title: a");
}
