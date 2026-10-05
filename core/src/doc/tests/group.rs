use super::*;

#[test]
fn new_group_round_trips() {
    let (_, group) = sample();
    assert_eq!(group.name, "Trip");
    assert_eq!(group.currency, "EUR");
    let names: Vec<_> = group.participants.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Alice", "Bob"], "blank names dropped, order kept");
}

#[test]
fn group_name_and_currency_can_change() {
    let (doc, g) = sample();
    let alice = &g.participants[0].id;
    add_expense(
        &doc,
        "Taxi",
        1000,
        alice.clone(),
        vec![split(alice, 1)],
        None,
        None,
    )
    .unwrap();

    update_group(&doc, " Lisbon ", "usd", "", None).unwrap();
    let g = read_group(&doc).unwrap();
    assert_eq!((g.name.as_str(), g.currency.as_str()), ("Lisbon", "USD"));
    assert_eq!(
        g.expenses[0].amount_cents, 1000,
        "amounts are not converted"
    );

    assert!(update_group(&doc, "  ", "EUR", "", None).is_err());
    for bad in ["", "EURO", "€", "E1R"] {
        assert!(
            update_group(&doc, "Lisbon", bad, "", None).is_err(),
            "{bad:?} accepted"
        );
    }
    assert!(new_group_doc("Trip", "euro", &[]).is_err());
}

#[test]
fn deleting_takes_settled_balances_or_everyone() {
    // Nobody owes anything: anyone deletes, without saying who they are.
    let (doc, _) = sample();
    assert!(delete_or_vote(&doc, None).unwrap());
    assert!(read_group(&doc).unwrap().deleted);

    let (doc, group) = sample();
    let ids: Vec<String> = group.participants.iter().map(|p| p.id.clone()).collect();
    let (alice, bob) = (&ids[0], &ids[1]);
    add_expense(
        &doc,
        "Dinner",
        9000,
        alice.clone(),
        vec![split(alice, 1), split(bob, 1)],
        None,
        None,
    )
    .unwrap();
    assert!(delete_or_vote(&doc, None).is_err(), "who asks?");
    assert!(delete_or_vote(&doc, Some("ghost")).is_err());

    // Everyone agrees one after the other; the last one deletes.
    for (i, id) in ids.iter().enumerate() {
        let last = i == ids.len() - 1;
        assert_eq!(delete_or_vote(&doc, Some(id)).unwrap(), last, "{i}");
        let read = read_group(&doc).unwrap();
        assert_eq!(read.deleted, last);
        assert_eq!(read.deletion_votes, ids[..=i]);
    }

    // A refusal starts over, and a removed member has no say.
    let (doc, group) = sample();
    let ids: Vec<String> = group.participants.iter().map(|p| p.id.clone()).collect();
    add_expense(
        &doc,
        "Dinner",
        9000,
        ids[0].clone(),
        vec![split(&ids[0], 1), split(&ids[1], 1)],
        None,
        None,
    )
    .unwrap();
    assert!(!delete_or_vote(&doc, Some(&ids[0])).unwrap());
    refuse_deletion(&doc).unwrap();
    assert!(read_group(&doc).unwrap().deletion_votes.is_empty());
    for id in &ids[2..] {
        remove_participant(&doc, id, None).unwrap();
    }
    assert!(!delete_or_vote(&doc, Some(&ids[0])).unwrap());
    assert!(delete_or_vote(&doc, Some(&ids[1])).unwrap());
}

#[test]
fn a_group_and_its_people_have_pictures() {
    let (doc, group) = sample();
    let picture = "data:image/webp;base64,UklGRg==";
    assert_eq!((group.description.as_str(), &group.image), ("", &None));

    update_group(&doc, "Trip", "EUR", " A week away ", Some(picture)).unwrap();
    let alice = group.participants[0].id.clone();
    set_participant_avatar(&doc, &alice, Some(picture)).unwrap();
    let read = read_group(&doc).unwrap();
    assert_eq!(read.description, "A week away");
    assert_eq!(read.image.as_deref(), Some(picture));
    assert_eq!(read.participants[0].avatar.as_deref(), Some(picture));
    assert_eq!(read.participants[1].avatar, None);

    // Not pictures, or too big.
    for bad in [
        "https://example.com/a.png",
        "data:image/svg+xml;base64,AAAA",
        "data:image/png;base64,<script>",
        "data:image/png;base64,",
    ] {
        assert!(
            update_group(&doc, "Trip", "EUR", "", Some(bad)).is_err(),
            "{bad}"
        );
    }
    let big = format!("data:image/png;base64,{}", "A".repeat(MAX_IMAGE_LEN));
    assert!(set_participant_avatar(&doc, &alice, Some(&big)).is_err());
    let long = "a".repeat(MAX_DESCRIPTION_CHARS + 1);
    assert!(update_group(&doc, "Trip", "EUR", &long, None).is_err());

    update_group(&doc, "Trip", "EUR", "", None).unwrap();
    set_participant_avatar(&doc, &alice, None).unwrap();
    let read = read_group(&doc).unwrap();
    assert_eq!((read.description.as_str(), &read.image), ("", &None));
    assert_eq!(read.participants[0].avatar, None);

    // A picture written by something else than the app is left out, not shown.
    doc.get_map(META)
        .insert("image", "javascript:alert(1)")
        .unwrap();
    assert_eq!(read_group(&doc).unwrap().image, None);
}

#[test]
fn concurrent_rename_and_currency_change_both_survive() {
    let (a, _) = sample();
    a.commit();
    let b = fork(&a);
    update_group(&a, "Lisbon", "EUR", "", None).unwrap();
    update_group(&b, "Trip", "CHF", "", None).unwrap();
    a.commit();
    b.commit();
    merge(&a, &b);

    let g = read_group(&a).unwrap();
    assert_eq!((g.name.as_str(), g.currency.as_str()), ("Lisbon", "CHF"));
    assert_eq!(g, read_group(&b).unwrap());
}
