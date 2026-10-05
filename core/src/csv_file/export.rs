//! Writing a group as a CSV file.

use super::*;

/// The group as a CSV file.
pub fn export(group: &Group) -> Res<String> {
    // Everyone of the group, then any ID no participant matches (possible after merging edits
    // from several devices), so no money drops out of the file.
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
    // Columns are told apart by name, so namesakes get a number.
    let mut used = HashSet::new();
    let names: Vec<String> = ids
        .iter()
        .map(|id| {
            let name = group
                .participants
                .iter()
                .find(|p| p.id == *id)
                .map_or("Unknown participant", |p| p.name.as_str());
            let mut unique = guard(name);
            let mut n = 2;
            while !used.insert(unique.clone()) {
                unique = format!("{} ({n})", guard(name));
                n += 1;
            }
            unique
        })
        .collect();
    let column = |id: &str| ids.iter().position(|x| *x == id);

    let mut out = csv::Writer::from_writer(Vec::new());
    let write_err = |e: csv::Error| format!("Could not write the CSV file: {e}");
    out.write_record(
        COLUMNS
            .iter()
            .copied()
            .chain(std::iter::once(CATEGORY))
            .chain(names.iter().map(String::as_str)),
    )
    .map_err(write_err)?;
    for e in &group.expenses {
        let original = e.original.as_ref();
        let mut parts = vec![String::new(); ids.len()];
        let mut split = vec![NOT_IN.to_string(); ids.len()];
        let amounts = owed(e.amount_cents, original.map(|o| o.amount_cents), &e.splits);
        for (s, amount) in e.splits.iter().zip(amounts) {
            if let Some(i) = column(&s.participant_id) {
                parts[i] = cents(amount);
                split[i] = s.fixed_cents.map_or_else(|| s.shares.to_string(), cents);
            }
        }
        let fixed = [
            e.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            guard(&e.title),
            cents(e.amount_cents),
            group.currency.clone(),
            if e.payers.is_empty() {
                column(&e.paid_by)
                    .map(|i| names[i].clone())
                    .unwrap_or_default()
            } else {
                e.payers
                    .iter()
                    .filter_map(|p| {
                        let name = &names[column(&p.participant_id)?];
                        Some(format!("{name}={}", cents(p.amount_cents)))
                    })
                    .collect::<Vec<_>>()
                    .join(SEVERAL_PAYERS)
            },
            if e.is_reimbursement {
                PAYMENT
            } else if e.income {
                INCOME
            } else {
                EXPENSE
            }
            .to_string(),
            original.map(|o| cents(o.amount_cents)).unwrap_or_default(),
            original.map(|o| o.currency.clone()).unwrap_or_default(),
            original.map(|o| o.rate.clone()).unwrap_or_default(),
            // A payment's one column says it all.
            if e.is_reimbursement {
                String::new()
            } else {
                split.join(" ")
            },
        ];
        let category = e.category.clone().unwrap_or_default();
        out.write_record(
            fixed
                .iter()
                .chain(std::iter::once(&category))
                .chain(parts.iter()),
        )
        .map_err(write_err)?;
    }
    let bytes = out
        .into_inner()
        .map_err(|e| format!("Could not write the CSV file: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("Could not write the CSV file: {e}"))
}
