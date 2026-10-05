use super::*;
use crate::models::{Expense, ExpensePayer, ExpenseSplit, OriginalAmount, Participant};
use chrono::Utc;

fn setup_test_group() -> Group {
    Group {
        id: "group_1".to_string(),
        name: "Vacation".to_string(),
        description: String::new(),
        image: None,
        currency: "EUR".to_string(),
        created_at: Utc::now(),
        participants: vec![
            Participant {
                id: "p1".to_string(),
                name: "Alice".to_string(),
                removed: false,
                avatar: None,
                added_at: None,
                added_by: None,
                removed_at: None,
                removed_by: None,
                iban: None,
            },
            Participant {
                id: "p2".to_string(),
                name: "Bob".to_string(),
                removed: false,
                avatar: None,
                added_at: None,
                added_by: None,
                removed_at: None,
                removed_by: None,
                iban: None,
            },
            Participant {
                id: "p3".to_string(),
                name: "Charlie".to_string(),
                removed: false,
                avatar: None,
                added_at: None,
                added_by: None,
                removed_at: None,
                removed_by: None,
                iban: None,
            },
        ],
        expenses: vec![],
        deleted: false,
        deletion_votes: vec![],
        trash: Vec::new(),
        recurring: Vec::new(),
    }
}

#[test]
fn test_zero_sum_invariant() {
    let mut group = setup_test_group();
    // Alice pays 60.00 for all 3
    group.expenses.push(Expense {
        id: "e1".to_string(),
        group_id: "group_1".to_string(),
        title: "Dinner".to_string(),
        amount_cents: 6000,
        paid_by: "p1".to_string(),
        payers: Vec::new(),
        category: None,
        splits: vec![
            ExpenseSplit {
                participant_id: "p1".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p2".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p3".to_string(),
                shares: 1,
                fixed_cents: None,
            },
        ],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        history: vec![],
        is_reimbursement: false,
        income: false,
        added_at: None,
        added_by: None,
        recurring: None,
        items: Vec::new(),
        comments: Vec::new(),
        original: None,
    });
    // Bob pays 30.00 for Alice & Bob
    group.expenses.push(Expense {
        id: "e2".to_string(),
        group_id: "group_1".to_string(),
        title: "Taxi".to_string(),
        amount_cents: 3000,
        paid_by: "p2".to_string(),
        payers: Vec::new(),
        category: None,
        splits: vec![
            ExpenseSplit {
                participant_id: "p1".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p2".to_string(),
                shares: 1,
                fixed_cents: None,
            },
        ],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        history: vec![],
        is_reimbursement: false,
        income: false,
        added_at: None,
        added_by: None,
        recurring: None,
        items: Vec::new(),
        comments: Vec::new(),
        original: None,
    });

    let balances = calculate_balances(&group);
    let sum_net: i64 = balances.iter().map(|b| b.net_cents).sum();
    assert_eq!(sum_net, 0, "Sum of net balances must always be zero");

    // Alice: paid 60, owed 20+15=35 -> net +25
    let alice = balances.iter().find(|b| b.participant_id == "p1").unwrap();
    assert_eq!(alice.paid_cents, 6000);
    assert_eq!(alice.owed_cents, 3500);
    assert_eq!(alice.net_cents, 2500);

    // Bob: paid 30, owed 20+15=35 -> net -5
    let bob = balances.iter().find(|b| b.participant_id == "p2").unwrap();
    assert_eq!(bob.net_cents, -500);

    // Charlie: paid 0, owed 20 -> net -20
    let charlie = balances.iter().find(|b| b.participant_id == "p3").unwrap();
    assert_eq!(charlie.net_cents, -2000);

    // Settlements should have Bob pay 5 to Alice and Charlie pay 20 to Alice
    let settlements = calculate_settlements(&group);
    assert_eq!(settlements.len(), 2);
    let total_settled: i64 = settlements.iter().map(|s| s.amount_cents).sum();
    assert_eq!(total_settled, 2500);
}

#[test]
fn test_odd_cents_division() {
    let mut group = setup_test_group();
    // 10.00 EUR (1000 cents) split 3 ways -> 334, 333, 333 = 1000 cents
    group.expenses.push(Expense {
        id: "e1".to_string(),
        group_id: "group_1".to_string(),
        title: "Snacks".to_string(),
        amount_cents: 1000,
        paid_by: "p1".to_string(),
        payers: Vec::new(),
        category: None,
        splits: vec![
            ExpenseSplit {
                participant_id: "p1".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p2".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p3".to_string(),
                shares: 1,
                fixed_cents: None,
            },
        ],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        history: vec![],
        is_reimbursement: false,
        income: false,
        added_at: None,
        added_by: None,
        recurring: None,
        items: Vec::new(),
        comments: Vec::new(),
        original: None,
    });

    let balances = calculate_balances(&group);
    let total_owed: i64 = balances.iter().map(|b| b.owed_cents).sum();
    assert_eq!(total_owed, 1000, "No cents lost in division");
}

#[test]
fn test_weighted_parts_division() {
    let mut group = setup_test_group();
    // Alice pays 10.00 EUR (1000 cents) split: Alice 1 part, Bob 2 parts (Bob pays for 2)
    // Total = 3 parts.
    // Base: Alice 1000*1/3 = 333 (rem 1), Bob 1000*2/3 = 666 (rem 2)
    // Remainder 1000 - 999 = 1 cent goes to Bob (larger remainder) -> Bob owes 667, Alice owes 333
    group.expenses.push(Expense {
        id: "e1".to_string(),
        group_id: "group_1".to_string(),
        title: "Groceries".to_string(),
        amount_cents: 1000,
        paid_by: "p1".to_string(),
        payers: Vec::new(),
        category: None,
        splits: vec![
            ExpenseSplit {
                participant_id: "p1".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p2".to_string(),
                shares: 2,
                fixed_cents: None,
            },
        ],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        history: vec![],
        is_reimbursement: false,
        income: false,
        added_at: None,
        added_by: None,
        recurring: None,
        items: Vec::new(),
        comments: Vec::new(),
        original: None,
    });

    let balances = calculate_balances(&group);
    let alice = balances.iter().find(|b| b.participant_id == "p1").unwrap();
    let bob = balances.iter().find(|b| b.participant_id == "p2").unwrap();

    assert_eq!(alice.owed_cents, 333, "Alice owes 1 share (3.33 €)");
    assert_eq!(bob.owed_cents, 667, "Bob owes 2 shares (6.67 €)");
    assert_eq!(
        alice.owed_cents + bob.owed_cents,
        1000,
        "Exact total preserved"
    );

    let sum_net: i64 = balances.iter().map(|b| b.net_cents).sum();
    assert_eq!(sum_net, 0, "Zero sum invariant preserved");

    let settlements = calculate_settlements(&group);
    assert_eq!(settlements.len(), 1);
    assert_eq!(settlements[0].from_id, "p2");
    assert_eq!(settlements[0].to_id, "p1");
    assert_eq!(settlements[0].amount_cents, 667);
}

#[test]
fn test_reimbursement_settles_debt() {
    let mut group = setup_test_group();
    // Alice pays 60.00 for all 3 (Bob owes 20, Charlie owes 20)
    group.expenses.push(Expense {
        id: "e1".to_string(),
        group_id: "group_1".to_string(),
        title: "Dinner".to_string(),
        amount_cents: 6000,
        paid_by: "p1".to_string(),
        payers: Vec::new(),
        category: None,
        splits: vec![
            ExpenseSplit {
                participant_id: "p1".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p2".to_string(),
                shares: 1,
                fixed_cents: None,
            },
            ExpenseSplit {
                participant_id: "p3".to_string(),
                shares: 1,
                fixed_cents: None,
            },
        ],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        history: vec![],
        is_reimbursement: false,
        income: false,
        added_at: None,
        added_by: None,
        recurring: None,
        items: Vec::new(),
        comments: Vec::new(),
        original: None,
    });

    // Bob reimburses Alice 20.00
    group.expenses.push(Expense {
        id: "e2".to_string(),
        group_id: "group_1".to_string(),
        title: "Payment: Bob -> Alice".to_string(),
        amount_cents: 2000,
        paid_by: "p2".to_string(),
        payers: Vec::new(),
        category: None,
        splits: vec![ExpenseSplit {
            participant_id: "p1".to_string(),
            shares: 1,
            fixed_cents: None,
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        history: vec![],
        is_reimbursement: true,
        income: false,
        added_at: None,
        added_by: None,
        recurring: None,
        items: Vec::new(),
        comments: Vec::new(),
        original: None,
    });

    let balances = calculate_balances(&group);
    let bob = balances.iter().find(|b| b.participant_id == "p2").unwrap();
    assert_eq!(bob.net_cents, 0, "Bob is fully settled up");

    let charlie = balances.iter().find(|b| b.participant_id == "p3").unwrap();
    assert_eq!(charlie.net_cents, -2000, "Charlie still owes 20.00");

    let alice = balances.iter().find(|b| b.participant_id == "p1").unwrap();
    assert_eq!(alice.net_cents, 2000, "Alice is owed 20.00");

    let settlements = calculate_settlements(&group);
    assert_eq!(settlements.len(), 1, "Only 1 transfer remains");
    assert_eq!(settlements[0].from_name, "Charlie");
    assert_eq!(settlements[0].to_name, "Alice");
    assert_eq!(settlements[0].amount_cents, 2000);
}

#[test]
fn several_payers_are_each_credited_what_they_paid() {
    let mut group = setup_test_group();
    let payer = |id: &str, amount_cents: i64| ExpensePayer {
        participant_id: id.to_string(),
        amount_cents,
    };
    // 90.00 paid 60 / 30 by Alice and Bob, shared by the three.
    let mut dinner = expense("e1", 9000, "p1", &["p1", "p2", "p3"]);
    dinner.payers = vec![payer("p1", 6000), payer("p2", 3000)];
    // 100.00 USD paid 75 / 25 by Bob and Charlie, worth 91.01 here.
    let mut taxi = expense("e2", 9101, "p2", &["p1", "p2"]);
    taxi.original = Some(OriginalAmount {
        currency: "USD".to_string(),
        amount_cents: 10000,
        rate: "0.9101".to_string(),
    });
    taxi.payers = vec![payer("p2", 7500), payer("p3", 2500)];
    assert_eq!(paid(&taxi), vec![("p2", 6826), ("p3", 2275)]);
    group.expenses = vec![dinner, taxi];

    let balances = calculate_balances(&group);
    let of = |id: &str| {
        let b = balances.iter().find(|b| b.participant_id == id).unwrap();
        (b.paid_cents, b.owed_cents)
    };
    assert_eq!(of("p1"), (6000, 3000 + 4551));
    assert_eq!(of("p2"), (3000 + 6826, 3000 + 4550));
    assert_eq!(of("p3"), (2275, 3000));
    assert_eq!(balances.iter().map(|b| b.net_cents).sum::<i64>(), 0);
}

#[test]
fn income_is_owed_by_who_received_it() {
    let mut group = setup_test_group();
    group
        .expenses
        .push(expense("e1", 3000, "p1", &["p1", "p2", "p3"]));
    // Alice gets a 9.00 refund for the three of them.
    let mut refund = expense("e2", 900, "p1", &["p1", "p2", "p3"]);
    refund.income = true;
    group.expenses.push(refund);

    let balances = calculate_balances(&group);
    let net = |id: &str| {
        let b = balances.iter().find(|b| b.participant_id == id).unwrap();
        b.net_cents
    };
    assert_eq!((net("p1"), net("p2"), net("p3")), (1400, -700, -700));
    let alice = &balances[0];
    assert_eq!((alice.paid_cents, alice.owed_cents), (2100, 700));
}

fn expense(id: &str, amount_cents: i64, paid_by: &str, split_ids: &[&str]) -> Expense {
    Expense {
        id: id.to_string(),
        group_id: "group_1".to_string(),
        title: id.to_string(),
        amount_cents,
        paid_by: paid_by.to_string(),
        payers: Vec::new(),
        category: None,
        splits: split_ids
            .iter()
            .map(|p| ExpenseSplit {
                participant_id: p.to_string(),
                shares: 1,
                fixed_cents: None,
            })
            .collect(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        history: vec![],
        is_reimbursement: false,
        income: false,
        added_at: None,
        added_by: None,
        recurring: None,
        items: Vec::new(),
        comments: Vec::new(),
        original: None,
    }
}

#[test]
fn test_removed_participant_keeps_open_balance() {
    let mut group = setup_test_group();
    group
        .expenses
        .push(expense("e1", 3000, "p1", &["p1", "p2", "p3"]));
    group.participants[1].removed = true; // Bob owes 10.00
    group.participants[2].removed = true;
    group.expenses.push(expense("e2", 1000, "p3", &["p1"])); // Charlie is now even

    let balances = calculate_balances(&group);
    let bob = balances.iter().find(|b| b.participant_id == "p2").unwrap();
    assert!(bob.removed);
    assert_eq!(bob.net_cents, -1000);
    assert!(
        !balances.iter().any(|b| b.participant_id == "p3"),
        "removed and settled participants are hidden"
    );
    assert_eq!(balances.iter().map(|b| b.net_cents).sum::<i64>(), 0);
    assert_eq!(calculate_settlements(&group)[0].from_name, "Bob");
}

#[test]
fn test_unknown_participant_ids_keep_zero_sum() {
    let mut group = setup_test_group();
    group
        .expenses
        .push(expense("e1", 900, "ghost", &["p1", "p2", "ghost"]));

    let balances = calculate_balances(&group);
    assert_eq!(balances.iter().map(|b| b.net_cents).sum::<i64>(), 0);
    let ghost = balances
        .iter()
        .find(|b| b.participant_id == "ghost")
        .unwrap();
    assert_eq!(ghost.participant_name, "Unknown participant");
    assert!(ghost.removed);
    assert_eq!(ghost.net_cents, 600);

    let settlements = calculate_settlements(&group);
    assert!(settlements
        .iter()
        .all(|s| s.to_name == "Unknown participant"));
    assert_eq!(settlements.iter().map(|s| s.amount_cents).sum::<i64>(), 600);
}

#[test]
fn test_extreme_amounts_and_shares_do_not_overflow() {
    let mut group = setup_test_group();
    let mut huge = expense("e1", i64::MAX, "p1", &["p1", "p2"]);
    huge.splits[1].shares = u32::MAX;
    group.expenses.push(huge);
    group.expenses.push(expense("e2", i64::MAX, "p1", &["p3"]));

    let balances = calculate_balances(&group);
    let find = |id: &str| balances.iter().find(|b| b.participant_id == id).unwrap();
    assert_eq!(
        find("p1").paid_cents,
        i64::MAX,
        "saturates instead of wrapping"
    );
    assert_eq!(find("p3").net_cents, -i64::MAX);
    assert_eq!(
        i128::from(find("p1").owed_cents) + i128::from(find("p2").owed_cents),
        i128::from(i64::MAX),
        "the split still adds up exactly"
    );
    assert!(!calculate_settlements(&group).is_empty());
}
