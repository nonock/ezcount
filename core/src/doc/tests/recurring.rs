use super::*;

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

#[test]
fn occurrences_count_from_the_start() {
    let day = |text: &str| -> DateTime<Utc> { format!("{text}T10:00:00Z").parse().unwrap() };
    let start = day("2026-01-31");
    assert_eq!(occurrence(start, "week", 0), Some(start));
    assert_eq!(occurrence(start, "week", 2), Some(day("2026-02-14")));
    // The day of the month is kept: the 31st gives the 28th in February, then the 31st again.
    assert_eq!(occurrence(start, "month", 1), Some(day("2026-02-28")));
    assert_eq!(occurrence(start, "month", 2), Some(day("2026-03-31")));
    let leap = day("2024-02-29");
    assert_eq!(occurrence(leap, "year", 1), Some(day("2025-02-28")));
    assert_eq!(occurrence(leap, "year", 4), Some(day("2028-02-29")));
    assert_eq!(occurrence(start, "day", 1), None);
    assert_eq!(occurrence(start, "year", u32::MAX), None);
}
