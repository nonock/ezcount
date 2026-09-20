use std::collections::HashMap;

use crate::models::{Group, ParticipantBalance, SettlementTransfer};

/// Calculates the detailed balance (paid, owed, net) for each participant in a group.
pub fn calculate_balances(group: &Group) -> Vec<ParticipantBalance> {
    let mut paid_map: HashMap<String, i64> = HashMap::new();
    let mut owed_map: HashMap<String, i64> = HashMap::new();

    // Ensure all participants exist in the maps
    for p in &group.participants {
        paid_map.insert(p.id.clone(), 0);
        owed_map.insert(p.id.clone(), 0);
    }

    for expense in &group.expenses {
        // Payer is credited the full expense amount
        *paid_map.entry(expense.paid_by.clone()).or_default() += expense.amount_cents;

        let count = expense.split_among.len() as i64;
        if count > 0 {
            // Integer division with exact remainder distribution so no cents are lost
            let base_share = expense.amount_cents / count;
            let remainder = (expense.amount_cents % count) as usize;

            for (idx, member_id) in expense.split_among.iter().enumerate() {
                let share = if idx < remainder {
                    base_share + 1
                } else {
                    base_share
                };
                *owed_map.entry(member_id.clone()).or_default() += share;
            }
        }
    }

    group
        .participants
        .iter()
        .map(|p| {
            let paid = paid_map.get(&p.id).copied().unwrap_or(0);
            let owed = owed_map.get(&p.id).copied().unwrap_or(0);
            let net = paid - owed;
            ParticipantBalance {
                participant_id: p.id.clone(),
                participant_name: p.name.clone(),
                paid_cents: paid,
                owed_cents: owed,
                net_cents: net,
            }
        })
        .collect()
}

/// Simplifies debts into the minimum number of direct transfers using a greedy settlement algorithm.
pub fn calculate_settlements(group: &Group) -> Vec<SettlementTransfer> {
    let balances = calculate_balances(group);
    let name_map: HashMap<String, String> = group
        .participants
        .iter()
        .map(|p| (p.id.clone(), p.name.clone()))
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
    use crate::models::{Expense, Participant};
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
                },
                Participant {
                    id: "p2".to_string(),
                    name: "Bob".to_string(),
                },
                Participant {
                    id: "p3".to_string(),
                    name: "Charlie".to_string(),
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
            split_among: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
            created_at: Utc::now(),
            is_reimbursement: false,
        });
        // Bob pays 30.00 for Alice & Bob
        group.expenses.push(Expense {
            id: "e2".to_string(),
            group_id: "group_1".to_string(),
            title: "Taxi".to_string(),
            amount_cents: 3000,
            paid_by: "p2".to_string(),
            split_among: vec!["p1".to_string(), "p2".to_string()],
            created_at: Utc::now(),
            is_reimbursement: false,
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
            split_among: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
            created_at: Utc::now(),
            is_reimbursement: false,
        });

        let balances = calculate_balances(&group);
        let total_owed: i64 = balances.iter().map(|b| b.owed_cents).sum();
        assert_eq!(total_owed, 1000, "No cents lost in division");
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
            split_among: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
            created_at: Utc::now(),
            is_reimbursement: false,
        });

        // Bob reimburses Alice 20.00
        group.expenses.push(Expense {
            id: "e2".to_string(),
            group_id: "group_1".to_string(),
            title: "Payment: Bob -> Alice".to_string(),
            amount_cents: 2000,
            paid_by: "p2".to_string(),
            split_among: vec!["p1".to_string()],
            created_at: Utc::now(),
            is_reimbursement: true,
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
}
