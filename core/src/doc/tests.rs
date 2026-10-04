use super::*;
use loro::ExportMode;

fn split(id: &str, shares: u32) -> ExpenseSplit {
    ExpenseSplit {
        participant_id: id.to_string(),
        shares,
        fixed_cents: None,
    }
}

fn sample() -> (LoroDoc, Group) {
    let doc = new_group_doc(" Trip ", "eur", &["Alice".into(), " ".into(), "Bob".into()]).unwrap();
    let group = read_group(&doc).unwrap();
    (doc, group)
}

/// Makes an independent replica, as another device would have after syncing.
fn fork(doc: &LoroDoc) -> LoroDoc {
    let other = LoroDoc::new();
    other
        .import(&doc.export(ExportMode::Snapshot).unwrap())
        .unwrap();
    other
}

fn merge(a: &LoroDoc, b: &LoroDoc) {
    a.import(&b.export(ExportMode::Snapshot).unwrap()).unwrap();
    b.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
}

#[test]
fn new_group_round_trips() {
    let (_, group) = sample();
    assert_eq!(group.name, "Trip");
    assert_eq!(group.currency, "EUR");
    let names: Vec<_> = group.participants.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Alice", "Bob"], "blank names dropped, order kept");
}

fn fixed(id: &str, amount: i64) -> ExpenseSplit {
    ExpenseSplit {
        participant_id: id.to_string(),
        shares: 0,
        fixed_cents: Some(amount),
    }
}

fn net(doc: &LoroDoc, id: &str) -> i64 {
    engine::calculate_balances(&read_group(doc).unwrap())
        .into_iter()
        .find(|b| b.participant_id == id)
        .unwrap()
        .net_cents
}

/// The splits as an app version from before fixed amounts reads them.
fn legacy_splits(doc: &LoroDoc) -> Vec<ExpenseSplit> {
    entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES)
        .into_iter()
        .flat_map(|(_, e)| read_splits(&e.splits, true))
        .collect()
}

#[test]
fn fixed_amounts_and_another_currency() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    let usd = |amount_cents: i64, rate: &str| {
        Some(OriginalAmount {
            currency: "usd".to_string(),
            amount_cents,
            rate: rate.to_string(),
        })
    };
    let add = |amount: i64, splits: Vec<ExpenseSplit>, original| {
        add_expense(&doc, "Taxi", amount, alice.clone(), splits, None, original)
    };

    // Bob owes 12.50, Alice the rest.
    add(3000, vec![split(alice, 1), fixed(bob, 1250)], None).unwrap();
    assert_eq!(net(&doc, bob), -1250);
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(e.splits, [split(alice, 1), fixed(bob, 1250)]);
    // An older app reads plain shares that give the same amounts.
    assert_eq!(legacy_splits(&doc), [split(alice, 7), split(bob, 5)]);

    // Changing the amount keeps the fixed part, and what the older app reads follows.
    update_expense(
        &doc,
        &e.id,
        "Taxi",
        2500,
        alice.clone(),
        e.splits.clone(),
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(net(&doc, bob), -1250);
    assert_eq!(legacy_splits(&doc), [split(alice, 1), split(bob, 1)]);

    // An older app changes the amount alone. The fixed part no longer fits in 10.00, so
    // the shares it reads are used here too.
    child_map(&doc.get_map(EXPENSES), &e.id)
        .unwrap()
        .insert("amount_cents", 1000)
        .unwrap();
    assert_eq!(
        read_group(&doc).unwrap().expenses[0].splits,
        [split(alice, 1), split(bob, 1)]
    );
    delete_expense(&doc, &e.id, None).unwrap();

    // 50.00 USD at 0.9234: Bob owes 20.00 USD of it, which is 18.47 of the 46.17.
    add(
        4617,
        vec![split(alice, 1), fixed(bob, 2000)],
        usd(5000, " 0,9234 "),
    )
    .unwrap();
    assert_eq!(net(&doc, bob), -1847);
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(
        e.original,
        usd(5000, "0.9234").map(|o| OriginalAmount {
            currency: "USD".to_string(),
            ..o
        })
    );
    // Back to the group's currency: recorded in the history.
    update_expense(
        &doc,
        &e.id,
        "Taxi",
        4617,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
        None,
    )
    .unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(e.original, None);
    assert_eq!(
        e.history[0].previous_original,
        usd(5000, "0.9234").map(|o| OriginalAmount {
            currency: "USD".to_string(),
            ..o
        })
    );
    assert_eq!(
        e.history[0].previous_splits,
        [split(alice, 1), fixed(bob, 2000)]
    );
    assert!(e.history[0]
        .summary
        .contains("Paid in EUR instead of 50.00 USD at 0.9234"));

    let both = || vec![fixed(alice, 1000), fixed(bob, 1250)];
    assert_eq!(
        add(3000, both(), None).unwrap_err(),
        "The amounts add up to 22.50, not the expense's 30.00"
    );
    assert_eq!(
        add(2000, vec![split(alice, 1), fixed(bob, 2500)], None).unwrap_err(),
        "The fixed amounts add up to 25.00, more than the expense's 20.00"
    );
    assert_eq!(
        add(2000, vec![split(alice, 1), fixed(bob, 0)], None).unwrap_err(),
        "A fixed amount must be above zero"
    );
    assert_eq!(
        add(2000, vec![split(alice, 1)], usd(2200, "0")).unwrap_err(),
        "The exchange rate must be a number above zero, such as 0.92"
    );
    assert_eq!(
        add(
            2000,
            vec![split(alice, 1)],
            Some(OriginalAmount {
                currency: "EUR".to_string(),
                amount_cents: 2000,
                rate: "1".to_string(),
            })
        )
        .unwrap_err(),
        "The group is in EUR already: leave out the exchange rate"
    );
}

#[test]
fn expense_edit_records_history() {
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
    let expense_id = read_group(&doc).unwrap().expenses[0].id.clone();

    update_expense(
        &doc,
        &expense_id,
        "Dinner",
        4500,
        bob.clone(),
        vec![split(bob, 1)],
        None,
        None,
        None,
    )
    .unwrap();

    let e = &read_group(&doc).unwrap().expenses[0];
    assert_eq!(e.amount_cents, 4500);
    assert_eq!(e.paid_by, *bob);
    assert_eq!(e.history.len(), 1);
    assert_eq!(e.history[0].previous_amount_cents, 3000);
    assert!(e.history[0]
        .summary
        .contains("Payer changed from Alice to Bob"));
}

#[test]
fn rejects_unknown_and_removed_participants() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    let err = add_expense(
        &doc,
        "X",
        100,
        "ghost".to_string(),
        vec![split(alice, 1)],
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("payer"));

    add_expense(
        &doc,
        "Old",
        100,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
    )
    .unwrap();
    remove_participant(&doc, bob, None).unwrap();
    let err = add_expense(
        &doc,
        "New",
        100,
        alice.clone(),
        vec![split(bob, 1)],
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("not an active member"));

    // Editing an expense Bob was already on is still allowed.
    let old = read_group(&doc).unwrap().expenses[0].id.clone();
    update_expense(
        &doc,
        &old,
        "Old (edited)",
        200,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
        None,
    )
    .unwrap();

    // And a removed participant can still settle up.
    record_reimbursement(&doc, bob.clone(), alice.clone(), 100, None, None).unwrap();
}

#[test]
fn concurrent_field_edits_both_survive() {
    let (a, g) = sample();
    let alice = &g.participants[0].id;
    add_expense(
        &a,
        "Taxi",
        1000,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
    )
    .unwrap();
    a.commit();
    let b = fork(&a);
    let id = read_group(&a).unwrap().expenses[0].id.clone();

    update_expense(
        &a,
        &id,
        "Taxi",
        1200,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
        None,
    )
    .unwrap();
    update_expense(
        &b,
        &id,
        "Airport taxi",
        1000,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
        None,
    )
    .unwrap();
    a.commit();
    b.commit();
    merge(&a, &b);

    let (ga, gb) = (read_group(&a).unwrap(), read_group(&b).unwrap());
    assert_eq!(ga, gb, "replicas converge");
    let e = &ga.expenses[0];
    assert_eq!(e.amount_cents, 1200);
    assert_eq!(e.title, "Airport taxi");
    assert_eq!(e.history.len(), 2, "both edits are in the history");
}

#[test]
fn concurrent_additions_merge() {
    let (a, g) = sample();
    let alice = &g.participants[0].id;
    let b = fork(&a);
    add_expense(
        &a,
        "Coffee",
        400,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
    )
    .unwrap();
    add_participant(&b, "Charlie", AddedBy::Member(None)).unwrap();
    a.commit();
    b.commit();
    merge(&a, &b);

    let g = read_group(&a).unwrap();
    assert_eq!(g.expenses.len(), 1);
    assert_eq!(g.participants.len(), 3);
    assert_eq!(g, read_group(&b).unwrap());
}

#[test]
fn group_name_and_currency_can_change() {
    let (doc, g) = sample();
    let alice = &g.participants[0].id;
    add_expense(
        &doc,
        "Taxi",
        1000,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
    )
    .unwrap();

    update_group(&doc, " Lisbon ", "usd", "", None).unwrap();
    let g = read_group(&doc).unwrap();
    assert_eq!((g.name.as_str(), g.currency.as_str()), ("Lisbon", "USD"));
    assert_eq!(
        g.expenses[0].amount_cents, 1000,
        "amounts are not converted"
    );

    assert!(update_group(&doc, "  ", "EUR", "", None).is_err());
    for bad in ["", "EURO", "€", "E1R"] {
        assert!(
            update_group(&doc, "Lisbon", bad, "", None).is_err(),
            "{bad:?} accepted"
        );
    }
    assert!(new_group_doc("Trip", "euro", &[]).is_err());
}

#[test]
fn deleting_takes_settled_balances_or_everyone() {
    // Nobody owes anything: anyone deletes, without saying who they are.
    let (doc, _) = sample();
    assert!(delete_or_vote(&doc, None).unwrap());
    assert!(read_group(&doc).unwrap().deleted);

    let (doc, group) = sample();
    let ids: Vec<String> = group.participants.iter().map(|p| p.id.clone()).collect();
    let (alice, bob) = (&ids[0], &ids[1]);
    add_expense(
        &doc,
        "Dinner",
        9000,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
    )
    .unwrap();
    assert!(delete_or_vote(&doc, None).is_err(), "who asks?");
    assert!(delete_or_vote(&doc, Some("ghost")).is_err());

    // Everyone agrees one after the other; the last one deletes.
    for (i, id) in ids.iter().enumerate() {
        let last = i == ids.len() - 1;
        assert_eq!(delete_or_vote(&doc, Some(id)).unwrap(), last, "{i}");
        let read = read_group(&doc).unwrap();
        assert_eq!(read.deleted, last);
        assert_eq!(read.deletion_votes, ids[..=i]);
    }

    // A refusal starts over, and a removed member has no say.
    let (doc, group) = sample();
    let ids: Vec<String> = group.participants.iter().map(|p| p.id.clone()).collect();
    add_expense(
        &doc,
        "Dinner",
        9000,
        ids[0].clone(),
        vec![split(&ids[0], 1), split(&ids[1], 1)],
        None,
        None,
    )
    .unwrap();
    assert!(!delete_or_vote(&doc, Some(&ids[0])).unwrap());
    refuse_deletion(&doc).unwrap();
    assert!(read_group(&doc).unwrap().deletion_votes.is_empty());
    for id in &ids[2..] {
        remove_participant(&doc, id, None).unwrap();
    }
    assert!(!delete_or_vote(&doc, Some(&ids[0])).unwrap());
    assert!(delete_or_vote(&doc, Some(&ids[1])).unwrap());
}

#[test]
fn expenses_have_a_category() {
    let (doc, group) = sample();
    let alice = &group.participants[0].id;
    let add = |category: Option<&str>| {
        add_expense(
            &doc,
            Label::new("Dinner", category),
            9000,
            alice.clone(),
            vec![split(alice, 1)],
            None,
            None,
        )
    };
    add(Some("food")).unwrap();
    // No category, and an empty one, are the same.
    add(Some("  ")).unwrap();
    for bad in ["Food", "a b", "<b>", &"a".repeat(31)] {
        assert!(add(Some(bad)).is_err(), "{bad}");
    }
    let mut expenses = read_group(&doc).unwrap().expenses;
    expenses.sort_by_key(|e| e.category.is_none());
    assert_eq!(expenses[0].category.as_deref(), Some("food"));
    assert_eq!(expenses[1].category, None);

    let update = |category: Option<&str>| {
        update_expense(
            &doc,
            &expenses[0].id,
            Label::new("Dinner", category),
            9000,
            alice.clone(),
            vec![split(alice, 1)],
            None,
            None,
            None,
        )
        .unwrap();
        let group = read_group(&doc).unwrap();
        group
            .expenses
            .into_iter()
            .find(|e| e.id == expenses[0].id)
            .unwrap()
    };
    let moved = update(Some("transport"));
    assert_eq!(moved.category.as_deref(), Some("transport"));
    assert_eq!(
        moved.history[0].summary,
        "Category changed from food to transport"
    );
    assert_eq!(moved.history[0].previous_category.as_deref(), Some("food"));
    let cleared = update(None);
    assert_eq!(cleared.category, None);
    assert_eq!(
        cleared.history[1].summary,
        "Category changed from transport to none"
    );

    // A category written by something else than the app is left out.
    let stored = child_map(&doc.get_map(EXPENSES), &expenses[0].id).unwrap();
    stored.insert("category", "Not A Key").unwrap();
    assert_eq!(read_group(&doc).unwrap().expenses[0].category, None);
}

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
fn several_people_pay_one_expense() {
    let (doc, group) = sample();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
    let payer = |id: &str, amount_cents: i64| ExpensePayer {
        participant_id: id.to_string(),
        amount_cents,
    };
    let everyone = || vec![split(alice, 1), split(bob, 1)];
    let add = |payers: Vec<ExpensePayer>| {
        add_expense(
            &doc,
            "Dinner",
            9000,
            PaidBy::new(alice.clone(), payers),
            everyone(),
            None,
            None,
        )
    };

    // Who paid the most is `paid_by`, whatever the order given.
    add(vec![payer(alice, 3000), payer(bob, 6000)]).unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(e.paid_by, *bob);
    assert_eq!(e.payers, vec![payer(bob, 6000), payer(alice, 3000)]);
    assert_eq!(net(&doc, bob), 1500);
    assert_eq!(net(&doc, alice), -1500);

    // A lone payer paid it all.
    add(vec![payer(bob, 1)]).unwrap();
    let lone = read_group(&doc).unwrap().expenses.remove(1);
    assert_eq!(
        (lone.paid_by.as_str(), lone.payers.len()),
        (bob.as_str(), 0)
    );
    delete_expense(&doc, &lone.id, None).unwrap();

    for (bad, why) in [
        (vec![payer(alice, 3000), payer(bob, 5000)], "not the total"),
        (vec![payer(alice, 9000), payer(bob, 0)], "nothing paid"),
        (vec![payer(alice, 4500), payer(alice, 4500)], "twice"),
        (
            vec![payer(alice, 4500), payer("ghost", 4500)],
            "not a member",
        ),
    ] {
        assert!(add(bad).is_err(), "{why}");
    }

    // Editing the payers is recorded with what each paid, and going back to one payer
    // removes them.
    let update = |paid_by: PaidBy, amount_cents: i64| {
        update_expense(
            &doc,
            &e.id,
            "Dinner",
            amount_cents,
            paid_by,
            everyone(),
            None,
            None,
            None,
        )
    };
    update(
        PaidBy::new(alice.clone(), vec![payer(alice, 7000), payer(bob, 2000)]),
        9000,
    )
    .unwrap();
    let edited = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(edited.paid_by, *alice);
    assert_eq!(
        edited.history[0].summary,
        "Payer changed from Bob (60.00), Alice (30.00) to Alice (70.00), Bob (20.00)"
    );
    assert_eq!(edited.history[0].previous_payers, e.payers);
    // The amount can't change without the payers' amounts.
    assert!(update(PaidBy::new(alice.clone(), edited.payers.clone()), 8000).is_err());
    update(bob.clone().into(), 8000).unwrap();
    let single = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(
        (single.paid_by.as_str(), single.payers.len()),
        (bob.as_str(), 0)
    );
    assert_eq!(net(&doc, bob), 4000);

    // An older app version changes the amount, or who paid, and knows nothing of the
    // payers: they no longer fit, so `paid_by` paid it all.
    add(vec![payer(alice, 3000), payer(bob, 6000)]).unwrap();
    let shared = read_group(&doc).unwrap().expenses.remove(1);
    let stored = child_map(&doc.get_map(EXPENSES), &shared.id).unwrap();
    stored.insert("amount_cents", 5000).unwrap();
    assert!(read_group(&doc).unwrap().expenses[1].payers.is_empty());
    stored.insert("amount_cents", 9000).unwrap();
    assert_eq!(read_group(&doc).unwrap().expenses[1].payers.len(), 2);
    stored.insert("paid_by", alice.as_str()).unwrap();
    let read = read_group(&doc).unwrap().expenses.remove(1);
    assert_eq!(
        (read.paid_by.as_str(), read.payers.len()),
        (alice.as_str(), 0)
    );
}

#[test]
fn a_group_and_its_people_have_pictures() {
    let (doc, group) = sample();
    let picture = "data:image/webp;base64,UklGRg==";
    assert_eq!((group.description.as_str(), &group.image), ("", &None));

    update_group(&doc, "Trip", "EUR", " A week away ", Some(picture)).unwrap();
    let alice = group.participants[0].id.clone();
    set_participant_avatar(&doc, &alice, Some(picture)).unwrap();
    let read = read_group(&doc).unwrap();
    assert_eq!(read.description, "A week away");
    assert_eq!(read.image.as_deref(), Some(picture));
    assert_eq!(read.participants[0].avatar.as_deref(), Some(picture));
    assert_eq!(read.participants[1].avatar, None);

    // Not pictures, or too big.
    for bad in [
        "https://example.com/a.png",
        "data:image/svg+xml;base64,AAAA",
        "data:image/png;base64,<script>",
        "data:image/png;base64,",
    ] {
        assert!(
            update_group(&doc, "Trip", "EUR", "", Some(bad)).is_err(),
            "{bad}"
        );
    }
    let big = format!("data:image/png;base64,{}", "A".repeat(MAX_IMAGE_LEN));
    assert!(set_participant_avatar(&doc, &alice, Some(&big)).is_err());
    let long = "a".repeat(MAX_DESCRIPTION_CHARS + 1);
    assert!(update_group(&doc, "Trip", "EUR", &long, None).is_err());

    update_group(&doc, "Trip", "EUR", "", None).unwrap();
    set_participant_avatar(&doc, &alice, None).unwrap();
    let read = read_group(&doc).unwrap();
    assert_eq!((read.description.as_str(), &read.image), ("", &None));
    assert_eq!(read.participants[0].avatar, None);

    // A picture written by something else than the app is left out, not shown.
    doc.get_map(META)
        .insert("image", "javascript:alert(1)")
        .unwrap();
    assert_eq!(read_group(&doc).unwrap().image, None);
}

#[test]
fn concurrent_rename_and_currency_change_both_survive() {
    let (a, _) = sample();
    a.commit();
    let b = fork(&a);
    update_group(&a, "Lisbon", "EUR", "", None).unwrap();
    update_group(&b, "Trip", "CHF", "", None).unwrap();
    a.commit();
    b.commit();
    merge(&a, &b);

    let g = read_group(&a).unwrap();
    assert_eq!((g.name.as_str(), g.currency.as_str()), ("Lisbon", "CHF"));
    assert_eq!(g, read_group(&b).unwrap());
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

/// What a group member could sync in: writes that never went through the checks above.
#[test]
fn crafted_synced_data_is_skipped_not_fatal() {
    let (doc, g) = sample();
    let alice = &g.participants[0].id;
    add_expense(
        &doc,
        "Real",
        1000,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
    )
    .unwrap();

    let now = Utc::now();
    let unchecked = |title: &str, amount_cents: i64, splits: Vec<ExpenseSplit>| {
        let expense = Expense {
            id: Uuid::new_v4().to_string(),
            group_id: g.id.clone(),
            title: title.to_string(),
            category: None,
            amount_cents,
            paid_by: alice.clone(),
            payers: Vec::new(),
            splits,
            created_at: now,
            updated_at: now,
            history: Vec::new(),
            is_reimbursement: false,
            original: None,
            income: false,
            added_at: None,
            added_by: None,
            recurring: None,
            items: Vec::new(),
            comments: Vec::new(),
        };
        insert_expense(&doc, &expense).unwrap();
    };
    unchecked("Too big", i64::MAX, vec![split(alice, 1)]);
    unchecked("Nothing", 0, vec![split(alice, 1)]);
    unchecked("Too big the other way", i64::MIN, vec![split(alice, 1)]);
    unchecked("Nobody", 500, vec![]);
    unchecked("Zero shares", 500, vec![split(alice, 0)]);

    // Containers nested far deeper than any schema, and a deeply nested plain value.
    let participants = doc.get_map(PARTICIPANTS);
    let mut deep = participants
        .insert_container("deep", LoroMap::new())
        .unwrap();
    for _ in 0..5_000 {
        deep = deep.insert_container("name", LoroMap::new()).unwrap();
    }
    let mut nested = LoroValue::from("x");
    for _ in 0..100 {
        nested = LoroValue::List(vec![nested].into());
    }
    participants.insert("nested", nested).unwrap();

    let group = read_group(&doc).unwrap();
    assert_eq!(group.participants.len(), 2, "only Alice and Bob");
    let titles: Vec<_> = group.expenses.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["Real"]);

    let err = add_expense(
        &doc,
        "Huge",
        MAX_AMOUNT_CENTS + 1,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("too large"), "{err}");
}

#[test]
fn money_that_came_in_counts_the_other_way() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    add_expense_as(
        &doc,
        "Deposit back",
        1000,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
        Adding {
            income: true,
            by: Some(bob.as_str()),
            ..Adding::default()
        },
    )
    .unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert!(e.income);
    assert_eq!(e.amount_cents, 1000);
    assert_eq!(e.added_by.as_deref(), Some(bob.as_str()));
    assert!(e.added_at.is_some());
    // Alice holds 10.00, of which 5.00 are Bob's.
    assert_eq!((net(&doc, alice), net(&doc, bob)), (-500, 500));
    // An app version from before finds an amount below zero, and leaves the entry out.
    let stored = entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES);
    assert_eq!(stored[0].1.amount_cents, -1000);

    // Editing keeps it money that came in, and says who did.
    update_expense(
        &doc,
        &e.id,
        "Deposit back",
        800,
        alice.clone(),
        e.splits.clone(),
        None,
        None,
        Some(alice.as_str()),
    )
    .unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert!(e.income);
    assert_eq!(e.amount_cents, 800);
    assert_eq!(e.history[0].previous_amount_cents, 1000);
    assert_eq!(e.history[0].edited_by.as_deref(), Some(alice.as_str()));
    assert_eq!(net(&doc, bob), 400);
}

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

#[test]
fn an_expense_entered_item_by_item() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    let item = |name: &str, amount_cents: i64, people: &[&String]| ExpenseItem {
        name: name.to_string(),
        amount_cents,
        participants: people.iter().map(|p| p.to_string()).collect(),
    };
    let add = |amount: i64, items: Vec<ExpenseItem>| {
        add_expense_as(
            &doc,
            "Groceries",
            amount,
            alice.clone(),
            Vec::new(),
            None,
            None,
            Adding {
                items,
                ..Adding::default()
            },
        )
    };
    // The lines must add up to the expense, and each be someone's.
    assert_eq!(
        add(3000, vec![item("Wine", 1201, &[bob])]).unwrap_err(),
        "The items add up to 12.01, not the expense's 30.00"
    );
    assert_eq!(
        add(1201, vec![item("Wine", 1201, &[])]).unwrap_err(),
        "Each item needs at least one person"
    );

    // Bob's wine, and bread for both: the odd cent goes to the first on the line.
    let items = vec![
        item(" Wine ", 1200, &[bob]),
        item("Bread", 301, &[alice, bob]),
    ];
    add(1501, items).unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(e.items[0].name, "Wine");
    assert_eq!(e.splits, [fixed(bob, 1350), fixed(alice, 151)]);
    assert_eq!(net(&doc, bob), -1350);

    // Editing the lines replaces the splits, and says so.
    update_expense_as(
        &doc,
        &e.id,
        "Groceries",
        1501,
        alice.clone(),
        Vec::new(),
        None,
        None,
        Editing {
            by: None,
            items: vec![item("Wine", 1501, &[alice, bob])],
        },
    )
    .unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert_eq!(e.splits, [fixed(alice, 751), fixed(bob, 750)]);
    assert_eq!(e.history[0].summary, "Items updated");

    // An app version from before changes the split without the lines: they no longer
    // describe the expense, and are left out.
    child_map(&doc.get_map(EXPENSES), &e.id)
        .unwrap()
        .insert(
            "splits",
            splits_value(1501, None, &[split(alice, 1), split(bob, 2)]),
        )
        .unwrap();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert!(e.items.is_empty());
    assert_eq!(e.splits, [split(alice, 1), split(bob, 2)]);
}

#[test]
fn comments_under_an_expense() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    let splits = vec![split(alice, 1), split(bob, 1)];
    add_expense(&doc, "Dinner", 3000, alice.clone(), splits, None, None).unwrap();
    doc.commit();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert!(add_comment(&doc, &e.id, "  ", None).is_err());
    assert!(add_comment(&doc, &e.id, &"a".repeat(MAX_COMMENT_CHARS + 1), None).is_err());
    assert!(add_comment(&doc, "nope", "Hello", None).is_err());

    // Two people comment at once, each on their device: both stay.
    let other = fork(&doc);
    add_comment(&doc, &e.id, " Was it this much? ", Some(bob.as_str())).unwrap();
    add_comment(&other, &e.id, "With the tip", Some(alice.as_str())).unwrap();
    doc.commit();
    other.commit();
    merge(&doc, &other);
    let comments = read_group(&doc).unwrap().expenses.remove(0).comments;
    let mut texts: Vec<&str> = comments.iter().map(|c| c.text.as_str()).collect();
    texts.sort_unstable();
    assert_eq!(texts, ["Was it this much?", "With the tip"]);
    let bobs = comments
        .iter()
        .find(|c| c.by.as_ref() == Some(bob))
        .unwrap();

    // They follow the expense to the trash and back.
    delete_expense(&doc, &e.id, None).unwrap();
    assert_eq!(read_group(&doc).unwrap().trash[0].expense.comments.len(), 2);
    restore_expense(&doc, &e.id, None).unwrap();

    delete_comment(&doc, &bobs.id).unwrap();
    assert!(delete_comment(&doc, &bobs.id).is_err());
    let comments = read_group(&doc).unwrap().expenses.remove(0).comments;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "With the tip");
}

#[test]
fn ibans_are_checked() {
    assert_eq!(
        check_iban(" fr76 3000 6000 0112 3456 7890 189 ").unwrap(),
        "FR7630006000011234567890189"
    );
    assert!(check_iban("GB82WEST12345698765432").is_ok());
    for wrong in [
        "",
        "FR76",
        "GB82WEST12345698765433",
        "1234567890123456",
        "FR76 30é0",
    ] {
        assert!(check_iban(wrong).is_err(), "{wrong}");
    }
    let (doc, g) = sample();
    let alice = &g.participants[0].id;
    set_participant_iban(&doc, alice, Some("de89 3704 0044 0532 0130 00")).unwrap();
    let iban = read_group(&doc).unwrap().participants.remove(0).iban;
    assert_eq!(iban.as_deref(), Some("DE89370400440532013000"));
    set_participant_iban(&doc, alice, None).unwrap();
    assert_eq!(read_group(&doc).unwrap().participants[0].iban, None);
}

#[test]
fn a_repeated_expense_comes_back_when_due() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    let at = |text: &str| text.parse::<DateTime<Utc>>().unwrap();
    let add = |repeat: &'static str, original| {
        add_expense_as(
            &doc,
            Label::new("Rent", Some("housing")),
            90000,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            Some(at("2100-01-31T10:00:00Z")),
            original,
            Adding {
                repeat: Some(repeat),
                ..Adding::default()
            },
        )
    };
    assert!(add("day", None).unwrap_err().contains("every week"));
    let usd = OriginalAmount {
        currency: "USD".to_string(),
        amount_cents: 100000,
        rate: "0.9".to_string(),
    };
    assert!(add("month", Some(usd)).unwrap_err().contains("currency"));
    assert!(read_group(&doc).unwrap().expenses.is_empty());

    add("month", None).unwrap();
    let group = read_group(&doc).unwrap();
    let model = group.recurring[0].clone();
    assert_eq!(group.expenses[0].recurring.as_ref(), Some(&model.id));
    assert_eq!((model.every.as_str(), model.amount_cents), ("month", 90000));
    // The 31st, or the month's last day.
    assert_eq!(model.next, at("2100-02-28T10:00:00Z"));
    assert!(!has_due_expenses(&doc, at("2100-02-27T10:00:00Z")));
    assert!(has_due_expenses(&doc, at("2100-02-28T10:00:00Z")));

    // Two devices add the ones due: the same expenses, not twice each.
    doc.commit();
    let other = fork(&doc);
    assert!(add_due_expenses(&doc, at("2100-03-31T12:00:00Z")).unwrap());
    assert!(add_due_expenses(&other, at("2100-03-31T12:00:00Z")).unwrap());
    doc.commit();
    other.commit();
    merge(&doc, &other);
    let group = read_group(&doc).unwrap();
    let days: Vec<String> = group
        .expenses
        .iter()
        .map(|e| e.created_at.format("%Y-%m-%d").to_string())
        .collect();
    assert_eq!(days, ["2100-01-31", "2100-02-28", "2100-03-31"]);
    assert!(group.expenses.iter().all(|e| e.title == "Rent"
        && e.category.as_deref() == Some("housing")
        && e.recurring.as_ref() == Some(&model.id)));
    assert_eq!(group.recurring[0].next, at("2100-04-30T10:00:00Z"));
    assert_eq!(net(&doc, bob), -135000);

    // One deleted since isn't added again.
    delete_expense(&doc, &group.expenses[2].id, None).unwrap();
    assert!(!add_due_expenses(&doc, at("2100-03-31T12:00:00Z")).unwrap());
    assert_eq!(read_group(&doc).unwrap().expenses.len(), 2);

    // Bob leaves: nothing more is added for him.
    remove_participant(&doc, bob, None).unwrap();
    assert!(read_group(&doc).unwrap().recurring[0].paused);
    assert!(!has_due_expenses(&doc, at("2101-01-01T00:00:00Z")));
    assert!(!add_due_expenses(&doc, at("2101-01-01T00:00:00Z")).unwrap());

    stop_recurring(&doc, &model.id).unwrap();
    let group = read_group(&doc).unwrap();
    assert!(group.recurring.is_empty());
    assert_eq!(group.expenses.len(), 2);
    assert!(stop_recurring(&doc, &model.id).is_err());
}
