use super::*;

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

fn item(amount_cents: i64, people: &[&str]) -> ExpenseItem {
    ExpenseItem {
        name: "Line".to_string(),
        amount_cents,
        participants: people.iter().map(|p| p.to_string()).collect(),
    }
}

#[test]
fn a_line_is_shared_equally_the_odd_cents_to_the_first() {
    let owed = items_owed(&[item(1000, &["a", "b", "c"]), item(500, &["b"])]);
    assert_eq!(
        owed,
        [
            ("a".to_string(), 334),
            ("b".to_string(), 833),
            ("c".to_string(), 333)
        ]
    );
    assert!(items_owed(&[item(1000, &[])]).is_empty());
}

#[test]
fn lines_give_fixed_amounts() {
    let splits = items_splits(&[item(1000, &["a", "b"]), item(1, &["a", "c"])]);
    // The cent of the second line went to its first person: nothing for the other.
    assert_eq!(splits, [fixed("a", 501), fixed("b", 500)]);
}

#[test]
fn lines_are_checked() {
    assert!(check_items(1500, &[item(1000, &["a", "b"]), item(500, &["a"])]).is_ok());
    assert_eq!(
        check_items(1500, &[item(1000, &["a"])]),
        Err("The items add up to 10.00, not the expense's 15.00".to_string())
    );
    assert_eq!(
        check_items(0, &[item(0, &["a"])]),
        Err("An item's amount must be above zero".to_string())
    );
    assert_eq!(
        check_items(1000, &[item(1000, &[])]),
        Err("Each item needs at least one person".to_string())
    );
    assert_eq!(
        check_items(1000, &[item(1000, &["a", "a"])]),
        Err("A participant appears twice on an item".to_string())
    );
    let mut named = item(1000, &["a"]);
    named.name = "é".repeat(MAX_ITEM_NAME_CHARS);
    assert!(check_items(1000, std::slice::from_ref(&named)).is_ok());
    named.name.push('!');
    assert!(check_items(1000, &[named]).is_err());
    let many = vec![item(1, &["a"]); MAX_ITEMS + 1];
    assert!(check_items(many.len() as i64, &many).is_err());
}
