use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn one_account_on_two_devices() {
    let (url, relay_dir) = start_relay().await;
    let phone = Device::signed_up(&url, "Alice").await;
    let trip = phone.create("Trip", &["Alice", "Bob"]);
    let alice = trip.participants[0].id.clone();
    assert_eq!(
        phone.identity(&trip.id),
        Some(alice.clone()),
        "creator is the first person"
    );
    phone.sync().await;

    // Logging in on a laptop brings the same groups and the same identity.
    let laptop = Device::logged_in(&url, "alice").await;
    assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);
    assert_eq!(laptop.group(&trip.id), phone.group(&trip.id));
    assert_eq!(laptop.identity(&trip.id), Some(alice.clone()));

    // A group created on the laptop shows up on the phone.
    let flat = laptop.create("Flat", &["Alice", "Chris"]);
    laptop.edit(&trip.id, |d| {
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
    laptop.sync().await;
    phone.sync().await;
    let mut ids = phone.group_ids();
    ids.sort();
    let mut expected = vec![trip.id.clone(), flat.id.clone()];
    expected.sort();
    assert_eq!(ids, expected);
    assert_eq!(phone.group(&trip.id).expenses.len(), 1);

    // Leaving on the phone removes the group from the laptop too.
    leave_group(&phone.state, &flat.id).await.unwrap();
    phone.sync().await;
    laptop.sync().await;
    assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_profile_shows_in_every_group() {
    let (url, relay_dir) = start_relay().await;
    let picture = "data:image/webp;base64,UklGRg==";
    let alice = Device::signed_up(&url, "alice").await;
    let trip = alice.create("Trip", &["Alice", "Bob"]);
    let flat = alice.create("Flat", &["Al", "Chris"]);
    let me = |group: &Group| group.participants[0].clone();

    let iban = "DE89370400440532013000";
    update_profile(&alice.state, " Alice M. ", Some(picture), Some(iban)).unwrap();
    for id in [&trip.id, &flat.id] {
        let group = alice.group(id);
        assert_eq!(me(&group).name, "Alice M.");
        assert_eq!(me(&group).avatar.as_deref(), Some(picture));
        assert_eq!(me(&group).iban.as_deref(), Some(iban));
        assert_eq!(group.participants[1].avatar, None);
    }
    let info = alice.state.require_account_info().unwrap();
    assert_eq!(info.display_name.as_deref(), Some("Alice M."));
    assert_eq!(info.avatar.as_deref(), Some(picture));

    // A new group gets the picture, and the other members see it.
    let dinner = alice.create("Dinner", &["Alice", "Bob"]);
    assert_eq!(me(&dinner).avatar.as_deref(), Some(picture));
    alice.sync().await;
    let bob = Device::signed_up(&url, "bob").await;
    let joined = join_group(&bob.state, &alice.invite(&dinner.id))
        .await
        .unwrap();
    assert_eq!(me(&joined).avatar.as_deref(), Some(picture));

    // The profile is on the account's other devices.
    let laptop = Device::logged_in(&url, "alice").await;
    let info = laptop.state.require_account_info().unwrap();
    assert_eq!(info.display_name.as_deref(), Some("Alice M."));

    // Saying to be someone else moves the picture and the name there.
    let other = dinner.participants[1].id.clone();
    set_identity(&alice.state, &dinner.id, &other).unwrap();
    let group = alice.group(&dinner.id);
    assert_eq!(me(&group).avatar, None);
    assert_eq!(group.participants[1].name, "Alice M.");
    assert_eq!(group.participants[1].avatar.as_deref(), Some(picture));

    // Without a name or a picture, groups keep their names and lose the picture.
    update_profile(&alice.state, "", None, None).unwrap();
    let group = alice.group(&trip.id);
    assert_eq!(me(&group).name, "Alice M.");
    assert_eq!(me(&group).avatar, None);
    assert!(update_profile(&alice.state, "Alice", Some("not a picture"), None).is_err());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn identity_chosen_on_one_device_applies_everywhere() {
    let (url, relay_dir) = start_relay().await;
    let alice = Device::signed_up(&url, "alice").await;
    let group = alice.create("Dinner", &["Alice", "Bob"]);
    alice.sync().await;

    let bob_phone = Device::signed_up(&url, "bob").await;
    let joined = join_group(&bob_phone.state, &alice.invite(&group.id))
        .await
        .unwrap();
    assert_eq!(
        bob_phone.identity(&group.id),
        None,
        "joiners pick who they are"
    );
    let bob = joined.participants[1].id.clone();
    set_identity(&bob_phone.state, &group.id, &bob).unwrap();
    assert!(set_identity(&bob_phone.state, &group.id, "nobody").is_err());
    bob_phone.sync().await;

    let bob_laptop = Device::logged_in(&url, "bob").await;
    assert_eq!(bob_laptop.identity(&group.id), Some(bob));

    // Someone new adds themselves instead of picking an existing name.
    let carol = Device::signed_up(&url, "carol").await;
    join_group(&carol.state, &alice.invite(&group.id))
        .await
        .unwrap();
    let updated = add_self(&carol.state, &group.id, "Carol").unwrap();
    let me = carol.identity(&group.id).unwrap();
    assert_eq!(
        updated
            .participants
            .iter()
            .find(|p| p.id == me)
            .unwrap()
            .name,
        "Carol"
    );

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_logins_lock_one_network_not_the_account() {
    let (url, dir) = start_limited_relay(ezcount_sync_server::Limits {
        login_failures_per_client: 3,
        login_failures_per_username: 5,
        ..Default::default()
    })
    .await;
    // The app sends no x-test-client header: it counts as another network.
    let _alice = Device::signed_up(&url, "alice").await;
    let wrong_login = |client: &'static str| {
        let url = url.clone();
        async move {
            reqwest::Client::new()
                .post(format!("{url}/v1/accounts/login"))
                .header("x-test-client", client)
                .json(&serde_json::json!({ "username": "alice", "login_token": "w".repeat(43) }))
                .send()
                .await
                .unwrap()
                .status()
                .as_u16()
        }
    };
    for _ in 0..3 {
        assert_eq!(wrong_login("203.0.113.7").await, 401);
    }
    assert_eq!(
        wrong_login("203.0.113.7").await,
        429,
        "that network is blocked"
    );
    let phone = Device::new();
    log_in(&phone.state, &url, "alice", PASSWORD).await.unwrap();

    // Past the per-username ceiling, spread over networks, everyone waits.
    for _ in 0..2 {
        assert_eq!(wrong_login("198.51.100.1").await, 401);
    }
    let err = log_in(&Device::new().state, &url, "alice", PASSWORD)
        .await
        .unwrap_err();
    assert!(err.contains("Too many failed attempts"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forgotten_password_is_reset_with_the_recovery_key() {
    let (url, relay_dir) = start_relay().await;
    let laptop = Device::new();
    let key = sign_up(&laptop.state, &url, "alice", PASSWORD)
        .await
        .unwrap()
        .expect("the relay stores recovery keys");
    let trip = laptop.create("Trip", &["Alice", "Bob"]).id;
    laptop.sync().await;

    let phone = Device::new();
    let err = recover_account(
        &phone.state,
        &url,
        "alice",
        &new_recovery_key().unwrap(),
        NEW_PASSWORD,
    )
    .await
    .unwrap_err();
    assert_eq!(err, "Wrong username or recovery key");
    let err = recover_account(&phone.state, &url, "alice", &key, "password123")
        .await
        .unwrap_err();
    assert!(err.contains("too easy to guess"), "{err}");

    // Typed as people do: lowercase, spaces instead of dashes.
    let typed = key.to_lowercase().replace('-', " ");
    let next = recover_account(&phone.state, &url, "alice", &typed, NEW_PASSWORD)
        .await
        .unwrap();
    assert_ne!(next, key, "a recovery key works once");
    reconcile(&phone.state).await.unwrap();
    assert_eq!(phone.group_ids(), vec![trip.clone()]);

    let other = Device::new();
    let err = log_in(&other.state, &url, "alice", PASSWORD)
        .await
        .unwrap_err();
    assert_eq!(err, "Wrong username or password");
    let err = recover_account(&other.state, &url, "alice", &key, NEW_PASSWORD)
        .await
        .unwrap_err();
    assert_eq!(
        err, "Wrong username or recovery key",
        "the used key is gone"
    );
    log_in(&other.state, &url, "alice", NEW_PASSWORD)
        .await
        .unwrap();
    // The laptop never needed the password again.
    laptop.sync().await;

    let _ = std::fs::remove_dir_all(&relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn changing_the_password_keeps_other_devices_in() {
    let (url, relay_dir) = start_relay().await;
    let laptop = Device::signed_up(&url, "alice").await;
    let trip = laptop.create("Trip", &["Alice", "Bob"]);
    laptop.sync().await;
    let phone = Device::logged_in(&url, "alice").await;

    let err = change_password(&laptop.state, "not my password", NEW_PASSWORD)
        .await
        .unwrap_err();
    assert_eq!(err, "Your current password is wrong");
    let err = change_password(&laptop.state, PASSWORD, "password123")
        .await
        .unwrap_err();
    assert!(err.contains("too easy to guess"), "{err}");
    change_password(&laptop.state, PASSWORD, NEW_PASSWORD)
        .await
        .unwrap();

    let other = Device::new();
    assert!(log_in(&other.state, &url, "alice", PASSWORD).await.is_err());
    log_in(&other.state, &url, "alice", NEW_PASSWORD)
        .await
        .unwrap();

    // The phone still syncs, without the new password.
    phone.sync().await;
    let (a, b) = (
        trip.participants[0].id.clone(),
        trip.participants[1].id.clone(),
    );
    phone.edit(&trip.id, |d| {
        doc::add_expense(
            d,
            "Taxi",
            1200,
            a.clone(),
            vec![split(&a), split(&b)],
            None,
            None,
        )
    });
    phone.sync().await;
    laptop.sync().await;
    assert_eq!(laptop.group(&trip.id).expenses.len(), 1);

    let _ = std::fs::remove_dir_all(&relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_recovery_key_replaces_the_old_one() {
    let (url, relay_dir) = start_relay().await;
    let laptop = Device::signed_up(&url, "alice").await;
    // As for an account from before recovery keys: the relay has none for it.
    let db = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
    db.execute(
        "UPDATE accounts SET recovery_hash = NULL, recovery_wrapped_key = NULL",
        [],
    )
    .unwrap();

    let err = replace_recovery_key(&laptop.state, "not my password")
        .await
        .unwrap_err();
    assert_eq!(err, "Wrong password");
    let first = replace_recovery_key(&laptop.state, PASSWORD).await.unwrap();
    let second = replace_recovery_key(&laptop.state, PASSWORD).await.unwrap();

    let phone = Device::new();
    let err = recover_account(&phone.state, &url, "alice", &first, NEW_PASSWORD)
        .await
        .unwrap_err();
    assert_eq!(err, "Wrong username or recovery key", "replaced");
    recover_account(&phone.state, &url, "alice", &second, NEW_PASSWORD)
        .await
        .unwrap();

    let _ = std::fs::remove_dir_all(&relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn sign_up_and_log_in_errors() {
    let (url, relay_dir) = start_relay().await;
    let _alice = Device::signed_up(&url, "alice").await;

    let other = Device::new();
    let err = sign_up(&other.state, &url, "ALICE", PASSWORD)
        .await
        .unwrap_err();
    assert!(err.contains("already taken"), "{err}");
    let err = sign_up(&other.state, &url, "dave", "short")
        .await
        .unwrap_err();
    assert!(err.contains("at least"), "{err}");
    let err = sign_up(&other.state, &url, "dave", "password123")
        .await
        .unwrap_err();
    assert!(err.contains("too easy to guess"), "{err}");

    let err = log_in(&other.state, &url, "alice", "wrong password")
        .await
        .unwrap_err();
    assert_eq!(err, "Wrong username or password");
    let err = log_in(&other.state, &url, "nobody", PASSWORD)
        .await
        .unwrap_err();
    assert_eq!(
        err, "Wrong username or password",
        "unknown user looks the same"
    );
    assert!(other.state.store().session().is_none());

    // After repeated failures the relay refuses even the right password for a while.
    for _ in 0..4 {
        let _ = log_in(&other.state, &url, "alice", "wrong password").await;
    }
    let err = log_in(&other.state, &url, "alice", PASSWORD)
        .await
        .unwrap_err();
    assert!(err.contains("Too many"), "{err}");

    let offline = Device::new();
    let err = sign_up(&offline.state, "http://127.0.0.1:9", "erin", PASSWORD)
        .await
        .unwrap_err();
    assert!(err.contains("reach"), "{err}");
    assert!(offline.state.store().session().is_none());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn local_groups_join_the_account_and_log_out_clears_the_device() {
    let (url, relay_dir) = start_relay().await;
    let device = Device::new();
    let doc = doc::new_group_doc("Before accounts", "EUR", &["Ann".into()]).unwrap();
    let old = device.state.store().insert(doc, None).unwrap();

    sign_up(&device.state, &url, "ann", PASSWORD).await.unwrap();
    assert!(device.state.sync_info(&old.id).unwrap().enabled);
    device.sync().await;

    log_out(&device.state, false).await.unwrap();
    assert!(device.state.store().session().is_none());
    assert!(device.group_ids().is_empty());

    log_in(&device.state, &url, "ann", PASSWORD).await.unwrap();
    reconcile(&device.state).await.unwrap();
    assert_eq!(device.group(&old.id).name, "Before accounts");

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn log_out_keeps_unuploaded_changes_unless_forced() {
    let device = Device::new();
    let (url, relay_dir) = start_relay().await;
    sign_up(&device.state, &url, "ann", PASSWORD).await.unwrap();
    // Point the account at a dead relay so nothing can be uploaded.
    device
        .state
        .store()
        .insert(
            doc::new_group_doc("Offline", "EUR", &["Ann".into()]).unwrap(),
            Some(SyncMeta::new(
                "http://127.0.0.1:9".into(),
                new_secret().unwrap(),
            )),
        )
        .unwrap();

    let err = log_out(&device.state, false).await.unwrap_err();
    assert!(err.contains("not uploaded"), "{err}");
    assert!(device.state.store().session().is_some());
    log_out(&device.state, true).await.unwrap();
    assert!(device.state.store().session().is_none());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_an_account_takes_its_password_and_frees_its_name() {
    let (url, relay_dir) = start_relay().await;
    let phone = Device::signed_up(&url, "ann").await;
    phone.create("Trip", &["Ann", "Bob"]);
    phone.sync().await;

    let err = delete_account(&phone.state, "not my password")
        .await
        .unwrap_err();
    assert_eq!(err, "Wrong password");
    assert!(phone.state.store().session().is_some());
    assert_eq!(phone.group_ids().len(), 1);

    delete_account(&phone.state, PASSWORD).await.unwrap();
    assert!(phone.state.store().session().is_none());
    assert!(phone.group_ids().is_empty());
    assert_eq!(
        delete_account(&phone.state, PASSWORD).await.unwrap_err(),
        "You are not logged in"
    );

    // Nothing is left to log in to, and the name can be taken again, with nothing of before.
    let other = Device::new();
    let err = log_in(&other.state, &url, "ann", PASSWORD)
        .await
        .unwrap_err();
    assert_eq!(err, "Wrong username or password");
    sign_up(&other.state, &url, "ann", PASSWORD).await.unwrap();
    other.sync().await;
    assert!(other.group_ids().is_empty());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_account_leaves_its_groups_to_the_others_without_its_profile() {
    let (url, relay_dir) = start_relay().await;
    let ann = Device::signed_up(&url, "ann").await;
    let trip = ann.create("Trip", &["Ann", "Bob"]);
    let me = trip.participants[0].id.clone();
    update_profile(
        &ann.state,
        "Ann",
        Some("data:image/webp;base64,UklGRg=="),
        Some("DE89370400440532013000"),
    )
    .unwrap();
    ann.sync().await;
    let bob = Device::signed_up(&url, "bob").await;
    join_group(&bob.state, &ann.invite(&trip.id)).await.unwrap();
    let seen = bob.group(&trip.id).participants[0].clone();
    assert!(seen.avatar.is_some() && seen.iban.is_some());

    // What Ann hadn't uploaded yet goes to the group before her device is emptied.
    ann.edit(&trip.id, |d| {
        doc::add_expense(d, "Taxi", 3000, me.clone(), vec![split(&me)], None, None)
    });
    delete_account(&ann.state, PASSWORD).await.unwrap();

    bob.sync().await;
    let group = bob.group(&trip.id);
    let left = &group.participants[0];
    assert_eq!(left.name, "Ann");
    assert_eq!((left.avatar.as_ref(), left.iban.as_ref()), (None, None));
    assert!(!left.removed);
    assert_eq!(group.expenses.len(), 1);

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_other_devices_of_a_deleted_account_log_out() {
    let (url, relay_dir) = start_relay().await;
    let phone = Device::signed_up(&url, "ann").await;
    let trip = phone.create("Trip", &["Ann", "Bob"]);
    let me = trip.participants[0].id.clone();
    phone.sync().await;
    let laptop = Device::logged_in(&url, "ann").await;
    assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);
    let bob = Device::signed_up(&url, "bob").await;
    join_group(&bob.state, &phone.invite(&trip.id))
        .await
        .unwrap();
    let (account_id, token) = {
        let store = laptop.state.store();
        let id = store.session().unwrap().account_id.clone();
        let keys = GroupKeys::derive(&store.sync_meta(&id).unwrap().secret).unwrap();
        (id, keys.auth_token)
    };

    // The laptop has an expense and a new name for the account that it didn't upload yet.
    laptop.edit(&trip.id, |d| {
        doc::add_expense(d, "Taxi", 3000, me.clone(), vec![split(&me)], None, None)
    });
    update_profile(&laptop.state, "Ann B.", None, None).unwrap();
    delete_account(&phone.state, PASSWORD).await.unwrap();

    let mut events = Vec::new();
    sync_all(&laptop.state, |event| events.push(event)).await;
    assert!(laptop.state.store().session().is_none());
    assert!(laptop.group_ids().is_empty());
    assert_eq!(events.last(), Some(&SyncEvent::Account));
    // The group got the expense first.
    bob.sync().await;
    assert_eq!(bob.group(&trip.id).expenses.len(), 1);

    // The account's document didn't come back with the laptop's changes, and can't.
    assert_eq!(raw_push(&url, "laptop", &account_id, &token, 10).await, 410);
    let pulled = reqwest::Client::new()
        .get(updates_url(&url, &account_id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(pulled.status(), 410);
    // A group nobody deleted is only unknown, as before.
    assert_eq!(
        reqwest::Client::new()
            .get(updates_url(&url, "no-such-group"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );

    let _ = std::fs::remove_dir_all(relay_dir);
}
