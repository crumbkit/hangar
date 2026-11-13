//! Unit tests of the validated name types.

use super::*;

fn long(byte: char, count: usize) -> String {
    byte.to_string().repeat(count)
}

#[test]
fn a_page_name_accepts_lower_case_letters_digits_and_inner_hyphens() {
    for text in ["a", "0", "tasks-board", "a-b-c", "x9", "9x"] {
        assert!(PageName::try_from(text).is_ok(), "{text}");
    }
}

#[test]
fn a_page_name_refuses_every_other_spelling() {
    for text in [
        "", "-a", "A", "a_b", "a b", "a/b", "../x", "a.b", "é", "a\0b", "a%2e",
    ] {
        assert!(PageName::try_from(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_page_name_is_at_most_64_bytes() {
    assert!(PageName::try_from(long('a', 64).as_str()).is_ok());
    assert_eq!(
        PageName::try_from(long('a', 65).as_str()),
        Err(ParseError::TooLong {
            subject: Subject::PageName,
            max: 64
        })
    );
}

#[test]
fn an_empty_page_name_reports_that_it_is_empty() {
    assert_eq!(
        PageName::try_from(""),
        Err(ParseError::Empty {
            subject: Subject::PageName
        })
    );
}

#[test]
fn a_binding_name_follows_the_page_name_rule() {
    assert!(BindingName::try_from("my-view-2").is_ok());
    assert!(BindingName::try_from("My-View").is_err());
    assert!(BindingName::try_from(long('b', 64).as_str()).is_ok());
    assert!(BindingName::try_from(long('b', 65).as_str()).is_err());
}

#[test]
fn a_name_displays_as_its_text() {
    let name = PageName::try_from("board-home").unwrap();
    assert_eq!(name.to_string(), "board-home");
    assert_eq!(name.as_str(), "board-home");
}

#[test]
fn a_slot_name_starts_with_a_letter_and_allows_digits_and_underscores() {
    for text in ["a", "task_list", "t9", "a_"] {
        assert!(SlotName::try_from(text).is_ok(), "{text}");
    }
    for text in ["", "9a", "_a", "A", "a-b", "a b", "a.b"] {
        assert!(SlotName::try_from(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_slot_name_is_at_most_32_bytes() {
    assert!(SlotName::try_from(long('a', 32).as_str()).is_ok());
    assert!(SlotName::try_from(long('a', 33).as_str()).is_err());
}

#[test]
fn a_field_name_is_1_to_64_bytes_and_never_starts_with_a_dollar_sign() {
    for text in ["title", "a", "with space", "é", "a$b"] {
        assert!(FieldName::try_from(text).is_ok(), "{text:?}");
    }
    for text in ["", "$type", "$"] {
        assert!(FieldName::try_from(text).is_err(), "{text:?}");
    }
    assert!(FieldName::try_from(long('f', 64).as_str()).is_ok());
    assert!(FieldName::try_from(long('f', 65).as_str()).is_err());
}

#[test]
fn a_field_name_refuses_control_characters() {
    for text in ["a\0b", "a\nb", "a\tb", "\u{7f}", "a\u{1b}[0m", "a\u{85}b"] {
        assert!(FieldName::try_from(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_field_name_with_printable_characters_is_accepted() {
    for text in ["title", "with space", "é", "a-b.c"] {
        assert!(FieldName::try_from(text).is_ok(), "{text:?}");
    }
}

#[test]
fn a_page_version_is_1_to_128_ascii_letters_and_digits() {
    for text in ["v1", "0f3a9c", "ABCdef012"] {
        assert!(PageVersion::try_from(text).is_ok(), "{text}");
    }
    for text in ["", "v-1", "v 1", "v.1", "é", "v\n"] {
        assert!(PageVersion::try_from(text).is_err(), "{text:?}");
    }
    assert!(PageVersion::try_from(long('a', 128).as_str()).is_ok());
    assert!(PageVersion::try_from(long('a', 129).as_str()).is_err());
}

#[test]
fn a_principal_id_is_1_to_256_printable_ascii_bytes_without_spaces() {
    for text in ["ada", "github:ada", "u@example.com", "a"] {
        assert!(PrincipalId::try_from(text).is_ok(), "{text}");
    }
    for text in ["", "a b", "a\tb", "a\0b", "é", "a\n"] {
        assert!(PrincipalId::try_from(text).is_err(), "{text:?}");
    }
    assert!(PrincipalId::try_from(long('p', 256).as_str()).is_ok());
    assert!(PrincipalId::try_from(long('p', 257).as_str()).is_err());
}
