use super::*;

#[test]
fn a_contact_is_trimmed_and_nothing_when_empty() {
    assert_eq!(note(None).unwrap(), None);
    assert_eq!(note(Some("   ".to_string())).unwrap(), None);
    assert_eq!(
        note(Some("  alice@example.com ".to_string())).unwrap(),
        Some("alice@example.com".to_string())
    );
}

#[test]
fn a_contact_has_a_longest_length_in_characters() {
    let longest = "é".repeat(MAX_NOTE_CHARS);
    assert_eq!(note(Some(longest.clone())).unwrap(), Some(longest.clone()));
    assert!(matches!(
        note(Some(format!("{longest}!"))),
        Err(ApiError::BadRequest(_))
    ));
}
