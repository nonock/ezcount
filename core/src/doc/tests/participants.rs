use super::*;

#[test]
fn members_remember_who_added_and_removed_them() {
    let (doc, group) = sample();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
    // The members the group started with weren't added by anyone.
    assert!(group
        .participants
        .iter()
        .all(|p| p.added_at.is_none() && p.added_by.is_none()));

    let carol = add_participant(&doc, "Carol", AddedBy::Member(Some(alice))).unwrap();
    let dave = add_participant(&doc, "Dave", AddedBy::Themselves).unwrap();
    let eve = add_participant(&doc, "Eve", AddedBy::Member(None)).unwrap();
    remove_participant(&doc, &carol, Some(bob)).unwrap();
    remove_participant(&doc, &eve, None).unwrap();

    let read = read_group(&doc).unwrap();
    let of = |id: &str| {
        read.participants
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .clone()
    };
    let (carol, dave, eve) = (of(&carol), of(&dave), of(&eve));
    assert!(carol.added_at.is_some() && carol.removed_at.is_some());
    assert_eq!(carol.added_by.as_ref(), Some(alice));
    assert_eq!(carol.removed_by.as_ref(), Some(bob));
    assert_eq!(dave.added_by.as_ref(), Some(&dave.id), "joined");
    assert_eq!((dave.removed_at, dave.removed_by), (None, None));
    assert!(eve.added_at.is_some() && eve.removed_at.is_some());
    assert_eq!((eve.added_by, eve.removed_by), (None, None));

    // A date that isn't one doesn't take the member with it.
    let stored = child_map(&doc.get_map(PARTICIPANTS), &dave.id).unwrap();
    stored.insert("added_at", "yesterday").unwrap();
    let read = read_group(&doc).unwrap();
    let dave = read.participants.iter().find(|p| p.id == dave.id).unwrap();
    assert_eq!(dave.added_at, None);
}

#[test]
fn renaming_updates_payment_titles_only() {
    let (doc, g) = sample();
    let (alice, bob) = (g.participants[0].id.clone(), g.participants[1].id.clone());
    record_reimbursement(&doc, alice.clone(), bob.clone(), 500, None, None).unwrap();
    record_reimbursement(
        &doc,
        bob.clone(),
        alice.clone(),
        300,
        Some("cash".into()),
        None,
    )
    .unwrap();
    add_expense(
        &doc,
        "Alice's birthday",
        2000,
        alice.clone(),
        vec![split(&bob, 1)],
        None,
        None,
    )
    .unwrap();
    // A payment whose title someone rewrote keeps it.
    record_reimbursement(&doc, alice.clone(), bob.clone(), 100, None, None).unwrap();
    let custom = read_group(&doc)
        .unwrap()
        .expenses
        .into_iter()
        .find(|e| e.amount_cents == 100)
        .unwrap();
    update_expense(
        &doc,
        &custom.id,
        "Beers",
        100,
        alice.clone(),
        custom.splits.clone(),
        None,
        None,
        None,
    )
    .unwrap();

    rename_participant(&doc, &alice, " Alicia ").unwrap();

    let g = read_group(&doc).unwrap();
    assert_eq!(g.participants[0].name, "Alicia");
    let mut titles: Vec<_> = g.expenses.iter().map(|e| e.title.as_str()).collect();
    titles.sort();
    assert_eq!(
        titles,
        [
            "Alice's birthday",
            "Beers",
            "Payment: Alicia → Bob",
            "Payment: Bob → Alicia (cash)",
        ]
    );

    assert!(rename_participant(&doc, &alice, " ").is_err());
    assert!(rename_participant(&doc, "ghost", "Zoe").is_err());
}
