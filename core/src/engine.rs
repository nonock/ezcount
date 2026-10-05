use std::collections::{HashMap, HashSet};

use crate::models::{
    Expense, ExpenseSplit, Group, Participant, ParticipantBalance, SettlementTransfer,
};

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

/// Who paid an expense and how much, in the group's currency: `paid_by` all of it, or each of
/// the `payers` their amount. Those are in the currency paid, so for an expense paid in another
/// one `amount_cents` is divided in the same proportions.
pub fn paid(expense: &Expense) -> Vec<(&str, i64)> {
    if expense.payers.is_empty() {
        return vec![(expense.paid_by.as_str(), expense.amount_cents)];
    }
    let there: Vec<i64> = expense.payers.iter().map(|p| p.amount_cents).collect();
    let here = match expense.original {
        Some(_) => split_weighted(expense.amount_cents, &there),
        None => there,
    };
    expense
        .payers
        .iter()
        .map(|p| p.participant_id.as_str())
        .zip(here)
        .collect()
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
        // Money that came in counts the other way: who received it holds what the others
        // share, so both sides are taken off.
        let sign: i128 = if expense.income { -1 } else { 1 };
        // Payers are credited what they paid, the full amount between them
        for (payer, amount) in paid(expense) {
            *paid_map.entry(payer.to_string()).or_default() += sign * i128::from(amount);
        }

        let owed = owed(
            expense.amount_cents,
            expense.original.as_ref().map(|o| o.amount_cents),
            &expense.splits,
        );
        for (s, owed) in expense.splits.iter().zip(owed) {
            *owed_map.entry(s.participant_id.clone()).or_default() += sign * i128::from(owed);
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
                .chain(e.payers.iter().map(|p| p.participant_id.as_str()))
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
mod tests;
