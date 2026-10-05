use super::*;
use crate::doc::{self, Adding};
use crate::models::ExpenseSplit;

#[test]
fn what_the_others_did_is_news() {
    let names = ["Alice".to_string(), "Bob".to_string()];
    let doc = doc::new_group_doc("Flat", "EUR", &names).unwrap();
    let before = doc::read_group(&doc).unwrap();
    let (alice, bob) = (&before.participants[0].id, &before.participants[1].id);
    let splits = || -> Vec<ExpenseSplit> {
        [alice, bob]
            .iter()
            .map(|id| ExpenseSplit {
                participant_id: id.to_string(),
                shares: 1,
                fixed_cents: None,
            })
            .collect()
    };
    let add = |title: &str, by: &String| {
        let adding = Adding {
            by: Some(by.as_str()),
            ..Adding::default()
        };
        doc::add_expense_as(
            &doc,
            title,
            3000,
            alice.clone(),
            splits(),
            None,
            None,
            adding,
        )
        .unwrap();
    };
    add("Dinner", alice);
    add("Taxi", bob);
    doc::record_reimbursement(&doc, bob.clone(), alice.clone(), 1500, None, Some(bob)).unwrap();
    let group = doc::read_group(&doc).unwrap();
    let dinner = group.expenses.iter().find(|e| e.title == "Dinner").unwrap();
    doc::add_comment(&doc, &dinner.id, "With the tip", Some(bob)).unwrap();
    let after = doc::read_group(&doc).unwrap();

    // Alice hears of what Bob did, not of her own dinner.
    let told = news(&before, &after, Some(alice.as_str()));
    let said: Vec<_> = told
        .iter()
        .map(|n| (n.kind, n.by.as_deref(), n.text.as_deref()))
        .collect();
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(said.contains(&("expense", Some("Bob"), None)));
    assert!(said.contains(&("payment", Some("Bob"), Some("Alice"))));
    assert!(said.contains(&("comment", Some("Bob"), Some("With the tip"))));
    assert_eq!(told[0].group, "Flat");

    // Bob only of her dinner, and nothing is news twice.
    let told = news(&before, &after, Some(bob.as_str()));
    assert_eq!(told.len(), 1);
    assert_eq!(
        (told[0].title.as_str(), told[0].amount_cents),
        ("Dinner", 3000)
    );
    assert!(news(&after, &after, None).is_empty());
}
