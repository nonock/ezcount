use std::collections::{HashMap, HashSet};

use crate::models::{ExpenseSplit, Group, Participant, ParticipantBalance, SettlementTransfer};

/// Totals are summed in `i128`, which no realistic number of expenses can overflow, and
/// saturate into `i64` (symmetrically, so negating one is always safe).
fn to_cents(total: i128) -> i64 {
    total.clamp(-i128::from(i64::MAX), i128::from(i64::MAX)) as i64
}

/// `amount_cents` divided in proportion to `weights`, in the same order: integer division,
/// with the leftover cents given by the Largest Remainder Method. All zero without weights.
fn split_weighted(amount_cents: i64, weights: &[i64]) -> Vec<i64> {
    let amount = i128::from(amount_cents);
    let total: i128 = weights.iter().map(|w| i128::from(*w)).sum();
    if total <= 0 {
        return vec![0; weights.len()];
    }
    let mut allocated: Vec<(i128, i128)> = weights
        .iter()
        .map(|w| {
            let weight = i128::from(*w);
            ((amount * weight) / total, (amount * weight) % total)
        })
        .collect();

    let total_allocated: i128 = allocated.iter().map(|(base, _)| *base).sum();
    // Below the number of splits for a positive amount; `read_group` drops the others.
    let remainder_cents = usize::try_from(amount - total_allocated).unwrap_or(0);

    // Sort indices by remainder descending to give leftover cents to highest fractional remainders
    let mut order: Vec<usize> = (0..allocated.len()).collect();
    order.sort_by(|&a, &b| allocated[b].1.cmp(&allocated[a].1));

    for &idx in order.iter().take(remainder_cents) {
        allocated[idx].0 += 1;
    }
    allocated
        .into_iter()
        .map(|(base, _)| to_cents(base))
        .collect()
}

/// What each of `shares` owes of `amount_cents`, in the same order.
pub fn split_amount(amount_cents: i64, shares: &[u32]) -> Vec<i64> {
    let weights: Vec<i64> = shares.iter().map(|s| i64::from(*s)).collect();
    split_weighted(amount_cents, &weights)
}

/// What each split owes of an expense, in the group's currency and in the same order.
///
/// Fixed amounts come first, and the parts share what is left. For an expense paid in another
/// currency (`original_cents`), that happens in that currency, and `amount_cents` is then
/// divided in the same proportions.
pub fn owed(amount_cents: i64, original_cents: Option<i64>, splits: &[ExpenseSplit]) -> Vec<i64> {
    let shares: Vec<u32> = splits.iter().map(|s| s.shares).collect();
    if splits.iter().all(|s| s.fixed_cents.is_none()) {
        return split_amount(amount_cents, &shares);
    }
    let paid = original_cents.unwrap_or(amount_cents);
    let fixed: i128 = splits
        .iter()
        .filter_map(|s| s.fixed_cents)
        .map(i128::from)
        .sum();
    let rest = to_cents((i128::from(paid) - fixed).max(0));
    let there: Vec<i64> = splits
        .iter()
        .zip(split_amount(rest, &shares))
        .map(|(s, part)| s.fixed_cents.unwrap_or(part))
        .collect();
    match original_cents {
        Some(_) => split_weighted(amount_cents, &there),
        None => there,
    }
}

/// Calculates the detailed balance (paid, owed, net) for each participant in a group.
pub fn calculate_balances(group: &Group) -> Vec<ParticipantBalance> {
    let mut paid_map: HashMap<String, i128> = HashMap::new();
    let mut owed_map: HashMap<String, i128> = HashMap::new();

    // Ensure all participants exist in the maps
    for p in &group.participants {
        paid_map.insert(p.id.clone(), 0);
        owed_map.insert(p.id.clone(), 0);
    }

    for expense in &group.expenses {
        // Payer is credited the full expense amount
        *paid_map.entry(expense.paid_by.clone()).or_default() += i128::from(expense.amount_cents);

        let owed = owed(
            expense.amount_cents,
            expense.original.as_ref().map(|o| o.amount_cents),
            &expense.splits,
        );
        for (s, owed) in expense.splits.iter().zip(owed) {
            *owed_map.entry(s.participant_id.clone()).or_default() += i128::from(owed);
        }
    }

    // Known participants first, then any ID no participant matches (possible after merging
    // edits from several devices), so money never silently drops out of the totals.
    let mut seen = HashSet::new();
    let ids: Vec<&str> = group
        .participants
        .iter()
        .map(|p| p.id.as_str())
        .chain(group.expenses.iter().flat_map(|e| {
            std::iter::once(e.paid_by.as_str())
                .chain(e.splits.iter().map(|s| s.participant_id.as_str()))
        }))
        .filter(|id| seen.insert(*id))
        .collect();

    let participants: HashMap<&str, &Participant> = group
        .participants
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    ids.into_iter()
        .filter_map(|id| {
            let participant = participants.get(id);
            let paid = paid_map.get(id).copied().unwrap_or(0);
            let owed = owed_map.get(id).copied().unwrap_or(0);
            let removed = participant.is_none_or(|p| p.removed);
            // Removed people only matter while they have something to settle.
            if removed && paid == owed {
                return None;
            }
            Some(ParticipantBalance {
                participant_id: id.to_string(),
                participant_name: participant
                    .map_or_else(|| "Unknown participant".to_string(), |p| p.name.clone()),
                paid_cents: to_cents(paid),
                owed_cents: to_cents(owed),
                net_cents: to_cents(paid - owed),
                removed,
            })
        })
        .collect()
}

/// Simplifies debts into the minimum number of direct transfers using a greedy settlement algorithm.
pub fn calculate_settlements(group: &Group) -> Vec<SettlementTransfer> {
    let balances = calculate_balances(group);
    let name_map: HashMap<String, String> = balances
        .iter()
        .map(|b| (b.participant_id.clone(), b.participant_name.clone()))
        .collect();

    // Debtors owe money (net < 0). Store as positive amount owed.
    let mut debtors: Vec<(String, i64)> = balances
        .iter()
        .filter(|b| b.net_cents < 0)
        .map(|b| (b.participant_id.clone(), -b.net_cents))
        .collect();

    // Creditors are owed money (net > 0).
    let mut creditors: Vec<(String, i64)> = balances
        .iter()
        .filter(|b| b.net_cents > 0)
        .map(|b| (b.participant_id.clone(), b.net_cents))
        .collect();

    // Sort largest debts/credits first to optimize resolution
    debtors.sort_by_key(|a| std::cmp::Reverse(a.1));
    creditors.sort_by_key(|a| std::cmp::Reverse(a.1));

    let mut transfers = Vec::new();
    let mut d_idx = 0;
    let mut c_idx = 0;

    while d_idx < debtors.len() && c_idx < creditors.len() {
        let (debtor_id, debtor_owed) = &mut debtors[d_idx];
        let (creditor_id, creditor_owed) = &mut creditors[c_idx];

        let settle_amount = (*debtor_owed).min(*creditor_owed);

        if settle_amount > 0 {
            transfers.push(SettlementTransfer {
                from_id: debtor_id.clone(),
                from_name: name_map
                    .get(debtor_id)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string()),
                to_id: creditor_id.clone(),
                to_name: name_map
                    .get(creditor_id)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string()),
                amount_cents: settle_amount,
            });
        }

        *debtor_owed -= settle_amount;
        *creditor_owed -= settle_amount;

        if *debtor_owed == 0 {
            d_idx += 1;
        }
        if *creditor_owed == 0 {
            c_idx += 1;
        }
    }

    transfers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Expense, ExpenseSplit, Participant};
    use chrono::Utc;

    fn setup_test_group() -> Group {
        Group {
            id: "group_1".to_string(),
            name: "Vacation".to_string(),
            currency: "EUR".to_string(),
            created_at: Utc::now(),
            participants: vec![
                Participant {
                    id: "p1".to_string(),
                    name: "Alice".to_string(),
                    removed: false,
                },
                Participant {
                    id: "p2".to_string(),
                    name: "Bob".to_string(),
                    removed: false,
                },
                Participant {
                    id: "p3".to_string(),
                    name: "Charlie".to_string(),
                    removed: false,
                },
            ],
            expenses: vec![],
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
            original: None,
        });
        // Bob pays 30.00 for Alice & Bob
        group.expenses.push(Expense {
            id: "e2".to_string(),
            group_id: "group_1".to_string(),
            title: "Taxi".to_string(),
            amount_cents: 3000,
            paid_by: "p2".to_string(),
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
            original: None,
        });

        // Bob reimburses Alice 20.00
        group.expenses.push(Expense {
            id: "e2".to_string(),
            group_id: "group_1".to_string(),
            title: "Payment: Bob -> Alice".to_string(),
            amount_cents: 2000,
            paid_by: "p2".to_string(),
            splits: vec![ExpenseSplit {
                participant_id: "p1".to_string(),
                shares: 1,
                fixed_cents: None,
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            history: vec![],
            is_reimbursement: true,
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

    fn expense(id: &str, amount_cents: i64, paid_by: &str, split_ids: &[&str]) -> Expense {
        Expense {
            id: id.to_string(),
            group_id: "group_1".to_string(),
            title: id.to_string(),
            amount_cents,
            paid_by: paid_by.to_string(),
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
}
