use super::*;

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
