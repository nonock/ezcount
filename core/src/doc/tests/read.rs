use super::*;

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
