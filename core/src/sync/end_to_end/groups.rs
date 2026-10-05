use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_group_is_archived_for_one_and_deleted_for_all() {
    let (url, relay_dir) = start_relay().await;
    let alice = Device::signed_up(&url, "alice").await;
    let trip = alice.create("Trip", &["Alice", "Bob"]);
    let flat = alice.create("Flat", &["Alice", "Bob"]);
    alice.sync().await;
    let bob = Device::signed_up(&url, "bob").await;
    for group in [&trip, &flat] {
        let joined = join_group(&bob.state, &alice.invite(&group.id))
            .await
            .unwrap();
        set_identity(&bob.state, &group.id, &joined.participants[1].id).unwrap();
    }
    let archived = |d: &Device| d.state.require_account_info().unwrap().archived;

    // Archiving is the user's own: their other devices see it, the other members don't.
    set_group_archived(&alice.state, &trip.id, true).unwrap();
    alice.sync().await;
    let laptop = Device::logged_in(&url, "alice").await;
    assert_eq!(archived(&laptop), vec![trip.id.clone()]);
    bob.sync().await;
    assert!(archived(&bob).is_empty());
    set_group_archived(&laptop.state, &trip.id, false).unwrap();
    laptop.sync().await;
    alice.sync().await;
    assert!(archived(&alice).is_empty());

    // Nobody owes anything in Flat: deleting it removes it for everyone.
    let flat_invite = alice.invite(&flat.id);
    assert_eq!(delete_group(&alice.state, &flat.id).unwrap(), None);
    assert!(!alice.state.store().groups().iter().any(|g| g.id == flat.id));
    alice.sync().await;
    assert_eq!(alice.group_ids(), vec![trip.id.clone()]);
    bob.sync().await;
    assert_eq!(bob.group_ids(), vec![trip.id.clone()]);
    laptop.sync().await;
    assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);
    let carol = Device::signed_up(&url, "carol").await;
    assert_eq!(
        join_group(&carol.state, &flat_invite).await.unwrap_err(),
        "This group was deleted"
    );

    // Bob owes Alice in Trip: it takes both of them.
    let (a, b) = (&trip.participants[0].id, &trip.participants[1].id);
    alice.edit(&trip.id, |d| {
        doc::add_expense(
            d,
            "Taxi",
            3000,
            a.clone(),
            vec![split(a), split(b)],
            None,
            None,
        )
    });
    let waiting = delete_group(&alice.state, &trip.id).unwrap().unwrap();
    assert_eq!(waiting.deletion_votes, vec![a.clone()]);
    alice.sync().await;
    bob.sync().await;
    assert_eq!(bob.group(&trip.id).deletion_votes, vec![a.clone()]);

    // Bob refuses, then Alice asks again and he agrees.
    bob.edit(&trip.id, doc::refuse_deletion);
    bob.sync().await;
    alice.sync().await;
    assert!(alice.group(&trip.id).deletion_votes.is_empty());
    delete_group(&alice.state, &trip.id).unwrap();
    alice.sync().await;
    bob.sync().await;
    assert_eq!(delete_group(&bob.state, &trip.id).unwrap(), None);
    bob.sync().await;
    assert!(bob.group_ids().is_empty());
    alice.sync().await;
    assert!(alice.group_ids().is_empty());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sync_says_what_the_others_did() {
    let (url, dir) = start_relay().await;
    let alice = Device::signed_up(&url, "alice").await;
    let flat = alice.create("Flat", &["Alice", "Bob"]);
    alice.sync().await;
    let bob = Device::signed_up(&url, "bob").await;
    join_group(&bob.state, &alice.invite(&flat.id))
        .await
        .unwrap();
    set_identity(&bob.state, &flat.id, &flat.participants[1].id).unwrap();
    bob.sync().await;

    // Alice adds a dinner: her own device has nothing to tell her.
    let me = flat.participants[0].id.clone();
    let splits = flat
        .participants
        .iter()
        .map(|p| crate::models::ExpenseSplit {
            participant_id: p.id.clone(),
            shares: 1,
            fixed_cents: None,
        })
        .collect();
    let adding = doc::Adding {
        by: Some(me.as_str()),
        ..Default::default()
    };
    alice.edit(&flat.id, |d| {
        doc::add_expense_as(d, "Dinner", 3000, me.clone(), splits, None, None, adding)
    });
    assert!(sync_all_noticing(&alice.state, |_| {}).await.is_empty());

    // Bob's next sync brings it, once.
    let told = sync_all_noticing(&bob.state, |_| {}).await;
    assert_eq!(told.len(), 1, "{told:?}");
    let notice = &told[0];
    assert_eq!((notice.kind, notice.group.as_str()), ("expense", "Flat"));
    assert_eq!(notice.by.as_deref(), Some("Alice"));
    assert_eq!(
        (notice.title.as_str(), notice.amount_cents),
        ("Dinner", 3000)
    );
    assert!(sync_all_noticing(&bob.state, |_| {}).await.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invite_opens_one_group_only() {
    let (url, relay_dir) = start_relay().await;
    let alice = Device::signed_up(&url, "alice").await;
    let trip = alice.create("Trip", &["Alice", "Bob"]);
    let flat = alice.create("Flat", &["Alice", "Chris"]);
    alice.sync().await;

    let bob = Device::signed_up(&url, "bob").await;
    join_group(&bob.state, &alice.invite(&trip.id))
        .await
        .unwrap();
    bob.sync().await;
    assert_eq!(bob.group_ids(), vec![trip.id.clone()]);

    // Alice's later groups don't reach Bob either, on any of his devices.
    let _party = alice.create("Party", &["Alice"]);
    alice.sync().await;
    bob.sync().await;
    assert_eq!(bob.group_ids(), vec![trip.id.clone()]);
    let bob_laptop = Device::logged_in(&url, "bob").await;
    assert_eq!(bob_laptop.group_ids(), vec![trip.id.clone()]);

    // The Trip key can't be used to open the Flat: each group has its own key.
    let trip_secret = bob
        .state
        .store()
        .sync_meta(&trip.id)
        .unwrap()
        .secret
        .clone();
    let borrowed = invite_code(&url, &flat.id, &trip_secret);
    let err = join_group(&bob.state, &borrowed).await.unwrap_err();
    assert!(err.contains("rejected"), "{err}");
    assert!(!bob.state.store().contains(&flat.id));

    // Nor can Alice's account be opened with a group key.
    let alice_account = alice.state.store().session().unwrap().account_id.clone();
    let err = join_group(&bob.state, &invite_code(&url, &alice_account, &trip_secret))
        .await
        .unwrap_err();
    assert!(err.contains("rejected"), "{err}");

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn members_share_and_converge() {
    let (url, relay_dir) = start_relay().await;
    let (a, b) = (
        Device::signed_up(&url, "alice").await,
        Device::signed_up(&url, "bob").await,
    );

    // A creates a group with one expense and uploads it.
    let group = a.create("Trip", &["Alice", "Bob"]);
    let (gid, alice, bob) = (
        group.id.clone(),
        group.participants[0].id.clone(),
        group.participants[1].id.clone(),
    );
    a.edit(&gid, |d| {
        doc::add_expense(
            d,
            "Hotel",
            20000,
            alice.clone(),
            vec![split(&alice), split(&bob)],
            None,
            None,
        )
    });
    sync_group(&a.state, &gid).await.unwrap();

    // B joins with the invite code and sees the same group.
    let joined = join_group(&b.state, &a.invite(&gid)).await.unwrap();
    assert_eq!(joined, a.group(&gid));
    assert!(
        join_group(&b.state, &a.invite(&gid)).await.is_err(),
        "joining twice is refused"
    );

    // Both edit while "offline", then sync in any order.
    let hotel = joined.expenses[0].id.clone();
    a.edit(&gid, |d| {
        doc::add_participant(d, "Charlie", doc::AddedBy::Member(None)).map(|_| ())
    });
    a.edit(&gid, |d| {
        doc::update_expense(
            d,
            &hotel,
            "Hotel",
            24000,
            alice.clone(),
            vec![split(&alice), split(&bob)],
            None,
            None,
            None,
        )
    });
    b.edit(&gid, |d| {
        doc::update_expense(
            d,
            &hotel,
            "Hotel Roma",
            20000,
            alice.clone(),
            vec![split(&alice), split(&bob)],
            None,
            None,
            None,
        )
    });
    b.edit(&gid, |d| {
        doc::add_expense(
            d,
            "Pizza",
            3000,
            bob.clone(),
            vec![split(&alice), split(&bob)],
            None,
            None,
        )
    });
    b.edit(&gid, |d| doc::remove_participant(d, &bob, None));

    assert!(
        !sync_group(&a.state, &gid).await.unwrap(),
        "nothing new from B yet"
    );
    assert!(
        sync_group(&b.state, &gid).await.unwrap(),
        "B receives A's edits"
    );
    assert!(
        sync_group(&a.state, &gid).await.unwrap(),
        "A receives B's edits"
    );

    let (ga, gb) = (a.group(&gid), b.group(&gid));
    assert_eq!(ga, gb, "devices converge");
    assert_eq!(ga.participants.len(), 3);
    assert!(ga.participants.iter().any(|p| p.id == bob && p.removed));
    let hotel = ga.expenses.iter().find(|e| e.id == hotel).unwrap();
    assert_eq!(
        (hotel.title.as_str(), hotel.amount_cents),
        ("Hotel Roma", 24000)
    );
    assert_eq!(ga.expenses.len(), 2);

    // Bob was removed but still owes money, so he still appears in the balances.
    let balances = crate::engine::calculate_balances(&ga);
    assert!(balances
        .iter()
        .any(|b| b.participant_id == bob && b.removed));
    assert_eq!(balances.iter().map(|b| b.net_cents).sum::<i64>(), 0);

    // Another sync round moves nothing.
    assert!(!sync_group(&a.state, &gid).await.unwrap());
    assert!(!sync_group(&b.state, &gid).await.unwrap());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_left_elsewhere_stays_until_its_edits_are_uploaded() {
    let (url, relay_dir) = start_relay().await;
    let phone = Device::signed_up(&url, "alice").await;
    let trip = phone.create("Trip", &["Alice"]);
    let alice = trip.participants[0].id.clone();
    let invite = phone.invite(&trip.id);
    phone.sync().await;
    let laptop = Device::logged_in(&url, "alice").await;

    // The phone can't reach the relay for this group when the laptop leaves it.
    let mut meta = phone.state.store().sync_meta(&trip.id).unwrap().clone();
    let relay_url = std::mem::replace(&mut meta.server_url, "http://127.0.0.1:9".into());
    phone
        .state
        .store()
        .set_sync(&trip.id, meta.clone())
        .unwrap();
    phone.edit(&trip.id, |d| {
        doc::add_expense(
            d,
            "Taxi",
            3000,
            alice.clone(),
            vec![split(&alice)],
            None,
            None,
        )
    });
    leave_group(&laptop.state, &trip.id).await.unwrap();
    laptop.sync().await;

    sync_account(&phone.state).await.unwrap();
    reconcile(&phone.state).await.unwrap();
    assert!(
        phone.state.store().contains(&trip.id),
        "the unuploaded edit is kept"
    );

    // Back online: the edit goes up first, then the group goes.
    meta.server_url = relay_url;
    phone.state.store().set_sync(&trip.id, meta).unwrap();
    reconcile(&phone.state).await.unwrap();
    assert!(!phone.state.store().contains(&trip.id));

    let bob = Device::signed_up(&url, "bob").await;
    let joined = join_group(&bob.state, &invite).await.unwrap();
    assert_eq!(
        joined.expenses.len(),
        1,
        "the edit reached the other members"
    );

    let _ = std::fs::remove_dir_all(relay_dir);
}
