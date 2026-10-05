use super::*;

#[test]
fn a_group_nobody_marked_is_in_the_first_format() {
    let (doc, group) = sample();
    assert_eq!(format_needed(&doc), 1);
    assert!(!needs_newer_app(&doc));
    assert!(!group.needs_update);
}

#[test]
fn a_format_this_version_knows_changes_nothing() {
    let (doc, _) = sample();
    require_format(&doc, FORMAT).unwrap();
    assert_eq!(format_needed(&doc), FORMAT);
    let group = read_group(&doc).unwrap();
    assert!(!group.needs_update);
    assert_eq!(group.participants.len(), 2);
}

#[test]
fn a_group_in_a_newer_format_shows_its_name_and_nothing_else() {
    let (doc, group) = sample();
    let alice = group.participants[0].id.clone();
    add_expense(
        &doc,
        "Taxi",
        3000,
        alice.clone(),
        vec![split(&alice, 1)],
        None,
        None,
    )
    .unwrap();
    update_group(&doc, "Trip", "EUR", "Two weeks", None).unwrap();

    require_format(&doc, FORMAT + 1).unwrap();
    assert!(needs_newer_app(&doc));
    let newer = read_group(&doc).unwrap();
    assert!(newer.needs_update);
    assert_eq!(
        (&newer.id, &newer.name, &newer.currency, newer.created_at),
        (&group.id, &group.name, &group.currency, group.created_at)
    );
    assert!(newer.participants.is_empty() && newer.expenses.is_empty());
    assert_eq!(newer.description, "");
    // Nothing is owed by anyone in what isn't read.
    assert!(engine::calculate_balances(&newer).is_empty());
}

#[test]
fn a_deleted_group_is_still_seen_as_deleted_in_a_newer_format() {
    let (doc, _) = sample();
    assert!(delete_or_vote(&doc, None).unwrap());
    require_format(&doc, FORMAT + 1).unwrap();
    assert!(read_group(&doc).unwrap().deleted);
}

#[test]
fn the_format_only_goes_up() {
    let (doc, _) = sample();
    require_format(&doc, 3).unwrap();
    require_format(&doc, 2).unwrap();
    assert_eq!(format_needed(&doc), 3);
}

#[test]
fn two_devices_raising_the_format_at_once_keep_the_higher_one() {
    let (phone, _) = sample();
    let laptop = fork(&phone);
    require_format(&phone, 2).unwrap();
    require_format(&laptop, 3).unwrap();
    phone.commit();
    laptop.commit();
    merge(&phone, &laptop);
    assert_eq!(format_needed(&phone), 3);
    assert_eq!(format_needed(&laptop), 3);
}

#[test]
fn marks_that_are_no_format_are_ignored() {
    let (doc, _) = sample();
    let marks = doc.get_map("format");
    marks.insert("soon", true).unwrap();
    marks.insert("-4", true).unwrap();
    marks.insert("", true).unwrap();
    assert_eq!(format_needed(&doc), 1);
}
