use super::*;

#[test]
fn a_deleted_expense_waits_in_the_trash() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    add_expense(
        &doc,
        "Dinner",
        3000,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
    )
    .unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);

    delete_expense(&doc, &e.id, Some(bob.as_str())).unwrap();
    let group = read_group(&doc).unwrap();
    assert!(group.expenses.is_empty());
    assert_eq!(net(&doc, bob), 0);
    assert_eq!(group.trash.len(), 1);
    assert_eq!(group.trash[0].expense, e);
    assert_eq!(group.trash[0].deleted_by.as_deref(), Some(bob.as_str()));
    // For an app version from before, it is deleted.
    assert!(child_map(&doc.get_map(EXPENSES), &e.id).is_none());

    restore_expense(&doc, &e.id, Some(alice.as_str())).unwrap();
    let group = read_group(&doc).unwrap();
    assert!(group.trash.is_empty());
    let back = &group.expenses[0];
    assert_eq!((back.id.as_str(), back.amount_cents), (e.id.as_str(), 3000));
    let noted = back.history.last().unwrap();
    assert_eq!(noted.summary, "Restored from the trash");
    assert_eq!(noted.edited_by.as_deref(), Some(alice.as_str()));
    assert_eq!(net(&doc, bob), -1500);

    delete_expense(&doc, &e.id, None).unwrap();
    purge_expense(&doc, &e.id).unwrap();
    assert!(read_group(&doc).unwrap().trash.is_empty());
    assert!(restore_expense(&doc, &e.id, None).is_err());
    assert!(purge_expense(&doc, &e.id).is_err());
}

#[test]
fn the_trash_empties_itself_after_a_month() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    for title in ["Dinner", "Taxi"] {
        let splits = vec![split(alice, 1), split(bob, 1)];
        add_expense(&doc, title, 3000, alice.clone(), splits, None, None).unwrap();
    }
    let group = read_group(&doc).unwrap();
    let (dinner, taxi) = (&group.expenses[0].id, &group.expenses[1].id);
    add_comment(&doc, dinner, "Was it this much?", Some(bob.as_str())).unwrap();
    delete_expense(&doc, dinner, None).unwrap();
    delete_expense(&doc, taxi, None).unwrap();
    // The dinner was deleted 31 days ago.
    let long_ago = Utc::now() - chrono::Duration::days(31);
    child_map(&doc.get_map(TRASH), dinner)
        .unwrap()
        .insert("deleted_at", timestamp(long_ago))
        .unwrap();

    let now = Utc::now();
    assert!(has_old_trash(&doc, now));
    empty_old_trash(&doc, now).unwrap();
    assert!(!has_old_trash(&doc, now));
    let trash = read_group(&doc).unwrap().trash;
    assert_eq!(trash.len(), 1);
    assert_eq!(&trash[0].expense.id, taxi);
    // Its comments went with it.
    assert_eq!(doc.get_map(COMMENTS).len(), 0);
}
