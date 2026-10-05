use super::*;

/// A group with a dinner Alice paid for her and Bob, and the dinner's id.
fn dinner() -> (LoroDoc, Group, String) {
    let (doc, group) = sample();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
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
    let id = read_group(&doc).unwrap().expenses[0].id.clone();
    (doc, group, id)
}

/// What the history says of the last edit.
fn last_summary(doc: &LoroDoc) -> String {
    let group = read_group(doc).unwrap();
    group.expenses[0].history.last().unwrap().summary.clone()
}

#[test]
fn an_edit_that_changes_nothing_says_so() {
    let (doc, group, id) = dinner();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
    update_expense(
        &doc,
        &id,
        " Dinner ",
        3000,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(last_summary(&doc), "Updated without major changes");
    let expense = &read_group(&doc).unwrap().expenses[0];
    assert_eq!(expense.title, "Dinner");
    assert_eq!(expense.history.len(), 1);
    assert!(expense.updated_at >= expense.created_at);
}

#[test]
fn an_edit_lists_what_changed_in_order() {
    let (doc, group, id) = dinner();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
    let day: DateTime<Utc> = "2026-03-15T12:00:00Z".parse().unwrap();
    let before = read_group(&doc).unwrap().expenses[0]
        .created_at
        .format("%Y-%m-%d");
    update_expense(
        &doc,
        &id,
        Label::new("Lunch", Some("food")),
        4500,
        bob.clone(),
        vec![split(alice, 2), split(bob, 1)],
        Some(day),
        Some(OriginalAmount {
            currency: "USD".to_string(),
            amount_cents: 5000,
            rate: "0.9".to_string(),
        }),
        Some(alice),
    )
    .unwrap();
    assert_eq!(
        last_summary(&doc),
        format!(
            "Title changed from 'Dinner' to 'Lunch'; Category changed from none to food; \
             Amount changed from 30.00 to 45.00; Payer changed from Alice to Bob; \
             Paid in 50.00 USD at 0.9 instead of EUR; Participants / parts allocation updated; \
             Date changed from {before} to 2026-03-15"
        )
    );
    let expense = &read_group(&doc).unwrap().expenses[0];
    assert_eq!(expense.created_at, day);
    assert_eq!(
        expense.history[0].edited_by.as_deref(),
        Some(alice.as_str())
    );
    // What it was before is kept with the edit.
    assert_eq!(expense.history[0].previous_title, "Dinner");
    assert_eq!(expense.history[0].previous_paid_by, *alice);
    assert_eq!(expense.history[0].previous_category, None);
}

#[test]
fn several_payers_are_named_with_what_each_paid() {
    let (_, group) = sample();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
    assert_eq!(payers_named(&group, alice, &[]), "Alice");
    assert_eq!(payers_named(&group, "gone", &[]), "Unknown");
    let together = [
        ExpensePayer {
            participant_id: alice.clone(),
            amount_cents: 2000,
        },
        ExpensePayer {
            participant_id: bob.clone(),
            amount_cents: 1050,
        },
    ];
    assert_eq!(
        payers_named(&group, alice, &together),
        "Alice (20.00), Bob (10.50)"
    );
}

#[test]
fn the_people_already_on_an_expense() {
    let (doc, group, _) = dinner();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
    let mut expense = read_group(&doc).unwrap().expenses[0].clone();
    expense.payers = vec![ExpensePayer {
        participant_id: "payer".to_string(),
        amount_cents: 3000,
    }];
    expense.items = vec![ExpenseItem {
        name: "Wine".to_string(),
        amount_cents: 3000,
        participants: vec!["guest".to_string()],
    }];
    let people = people_of(&expense);
    assert_eq!(
        people,
        HashSet::from([alice.as_str(), bob.as_str(), "payer", "guest"])
    );
}

#[test]
fn a_new_amount_rewrites_the_shares_older_versions_read() {
    let (doc, group, id) = dinner();
    let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
    let splits = vec![fixed(alice, 1000), split(bob, 1)];
    update_expense(
        &doc,
        &id,
        "Dinner",
        3000,
        alice.clone(),
        splits.clone(),
        None,
        None,
        None,
    )
    .unwrap();
    // 1000 fixed out of 3000: one share for two.
    assert_eq!(legacy_splits(&doc), [split(alice, 1), split(bob, 2)]);

    // The same splits, but of another amount: the shares follow it.
    update_expense(
        &doc,
        &id,
        "Dinner",
        5000,
        alice.clone(),
        splits,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(legacy_splits(&doc), [split(alice, 1), split(bob, 4)]);
    assert_eq!(last_summary(&doc), "Amount changed from 30.00 to 50.00");
}

#[test]
fn an_edit_of_an_unknown_expense_is_refused() {
    let (doc, group, _) = dinner();
    let alice = &group.participants[0].id;
    let result = update_expense(
        &doc,
        "nope",
        "Dinner",
        3000,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
        None,
    );
    assert_eq!(result, Err("Expense not found".to_string()));
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
