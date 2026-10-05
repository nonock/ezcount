use super::*;
use loro::ExportMode;

fn split(id: &str, shares: u32) -> ExpenseSplit {
    ExpenseSplit {
        participant_id: id.to_string(),
        shares,
        fixed_cents: None,
    }
}

fn sample() -> (LoroDoc, Group) {
    let doc = new_group_doc(" Trip ", "eur", &["Alice".into(), " ".into(), "Bob".into()]).unwrap();
    let group = read_group(&doc).unwrap();
    (doc, group)
}

/// Makes an independent replica, as another device would have after syncing.
fn fork(doc: &LoroDoc) -> LoroDoc {
    let other = LoroDoc::new();
    other
        .import(&doc.export(ExportMode::Snapshot).unwrap())
        .unwrap();
    other
}

fn merge(a: &LoroDoc, b: &LoroDoc) {
    a.import(&b.export(ExportMode::Snapshot).unwrap()).unwrap();
    b.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
}

fn fixed(id: &str, amount: i64) -> ExpenseSplit {
    ExpenseSplit {
        participant_id: id.to_string(),
        shares: 0,
        fixed_cents: Some(amount),
    }
}

fn net(doc: &LoroDoc, id: &str) -> i64 {
    engine::calculate_balances(&read_group(doc).unwrap())
        .into_iter()
        .find(|b| b.participant_id == id)
        .unwrap()
        .net_cents
}

/// The splits as an app version from before fixed amounts reads them.
fn legacy_splits(doc: &LoroDoc) -> Vec<ExpenseSplit> {
    entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES)
        .into_iter()
        .flat_map(|(_, e)| read_splits(&e.splits, true))
        .collect()
}

mod check;
mod comments;
mod edit;
mod expenses;
mod group;
mod items;
mod participants;
mod read;
mod recurring;
mod trash;
mod values;
