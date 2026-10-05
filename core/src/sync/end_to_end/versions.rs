use super::*;
use ezcount_sync_server::Settings;

/// A relay that only answers apps from `oldest` on.
async fn start_relay_asking_for(oldest: &str) -> (String, PathBuf) {
    start_relay_with(Settings {
        min_app_version: Some(oldest.parse().unwrap()),
        ..Default::default()
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn the_app_says_its_version_to_the_relay() {
    // A relay asking for this very version answers it: the app said which it is.
    let (url, relay_dir) = start_relay_asking_for(APP_VERSION).await;
    let device = Device::signed_up(&url, "ann").await;
    let trip = device.create("Trip", &["Ann", "Bob"]);
    device.sync().await;
    assert_eq!(device.state.sync_info(&trip.id).unwrap().last_error, None);
    assert!(!device.state.update_required());

    // Something that isn't the app, or an app from before versions were sent, says nothing.
    let silent = reqwest::Client::new();
    let asked = silent
        .get(updates_url(&url, &trip.id))
        .send()
        .await
        .unwrap();
    assert_eq!(asked.status(), 426);
    let older = silent
        .get(updates_url(&url, &trip.id))
        .header("ezcount-version", "0.0.1")
        .send()
        .await
        .unwrap();
    assert_eq!(older.status(), 426);
    // The pages stay open to browsers.
    for page in ["/health", "/join", "/privacy"] {
        let page = silent.get(format!("{url}{page}")).send().await.unwrap();
        assert_eq!(page.status(), 200);
    }

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_asking_for_a_newer_app_says_so_at_the_door() {
    let (url, relay_dir) = start_relay_asking_for("999.0.0").await;
    let device = Device::new();
    for refused in [
        sign_up(&device.state, &url, "ann", PASSWORD)
            .await
            .map(|_| ()),
        log_in(&device.state, &url, "ann", PASSWORD).await,
    ] {
        assert_eq!(refused.unwrap_err(), UPDATE_REQUIRED);
    }
    assert!(device.state.store().session().is_none());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_app_too_old_for_its_relay_knows_it_until_it_is_answered_again() {
    let relay = ReplaceableRelay::start().await;
    let device = Device::signed_up(&relay.url, "ann").await;
    let trip = device.create("Trip", &["Ann", "Bob"]);
    device.sync().await;
    assert!(
        !device
            .state
            .account_info()
            .unwrap()
            .unwrap()
            .update_required
    );

    // The relay comes back asking for a newer app.
    let relay = relay
        .restart_with(Settings {
            min_app_version: Some("999.0.0".parse().unwrap()),
            ..Default::default()
        })
        .await;
    let mut events = Vec::new();
    sync_all(&device.state, |event| events.push(event)).await;
    assert!(
        device
            .state
            .account_info()
            .unwrap()
            .unwrap()
            .update_required
    );
    assert!(events.contains(&SyncEvent::Account));
    assert_eq!(
        device
            .state
            .sync_info(&trip.id)
            .unwrap()
            .last_error
            .as_deref(),
        Some(UPDATE_REQUIRED)
    );
    // Nothing was lost meanwhile, and saying it once is enough.
    assert_eq!(device.group(&trip.id).participants.len(), 2);
    let mut again = Vec::new();
    sync_all(&device.state, |event| again.push(event)).await;
    assert!(!again.contains(&SyncEvent::Account));

    // Once the relay answers this version again (or the app is updated), it goes on.
    let relay = relay.restart_with(Settings::default()).await;
    let mut events = Vec::new();
    sync_all(&device.state, |event| events.push(event)).await;
    assert!(!device.state.update_required());
    assert!(events.contains(&SyncEvent::Account));
    assert_eq!(device.state.sync_info(&trip.id).unwrap().last_error, None);

    let _ = std::fs::remove_dir_all(&relay.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_in_a_newer_format_is_synced_and_left_alone() {
    let (url, relay_dir) = start_relay().await;
    let ann = Device::signed_up(&url, "ann").await;
    let trip = ann.create("Trip", &["Ann", "Bob"]);
    let me = trip.participants[0].id.clone();
    ann.sync().await;
    let bob = Device::signed_up(&url, "bob").await;
    join_group(&bob.state, &ann.invite(&trip.id)).await.unwrap();

    // As a newer version of the app would, on Ann's phone: it changes the group in a way
    // this version can't read, and says so in the group.
    ann.edit(&trip.id, |d| doc::require_format(d, doc::FORMAT + 1));
    ann.sync().await;
    bob.sync().await;

    let seen = bob.group(&trip.id);
    assert!(seen.needs_update);
    assert_eq!(seen.name, "Trip");
    assert!(seen.participants.is_empty());
    assert_eq!(bob.group_ids(), vec![trip.id.clone()]);
    let listed = crate::api::get_groups(&bob.state);
    assert!(listed.len() == 1 && listed[0].needs_update);

    // Nothing in it can be changed from here, by the user or by the app itself.
    let refused = bob.state.mutate(&trip.id, |d| {
        doc::add_expense(d, "Taxi", 3000, me.clone(), vec![split(&me)], None, None)
    });
    assert_eq!(refused.unwrap_err(), doc::NEWER_FORMAT);
    assert!(!bob.state.store().has_unpushed(&trip.id));
    // It still syncs: what the others do keeps coming, for the day the app is updated.
    bob.sync().await;
    assert_eq!(bob.state.sync_info(&trip.id).unwrap().last_error, None);
    // The account is not the group: it goes on as before.
    assert!(!bob.state.update_required());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_account_in_a_newer_format_is_neither_changed_nor_followed() {
    let (url, relay_dir) = start_relay().await;
    let phone = Device::signed_up(&url, "ann").await;
    let trip = phone.create("Trip", &["Ann", "Bob"]);
    phone.sync().await;
    let laptop = Device::logged_in(&url, "ann").await;
    assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);

    // As a newer version of the app would, on the phone: the list of groups moves somewhere
    // this version doesn't look, so here it reads as empty.
    phone
        .state
        .store()
        .update_account(|d| {
            account::remove_group(d, &trip.id)?;
            doc::require_format(d, account::FORMAT + 1)
        })
        .unwrap();
    sync_account(&phone.state).await.unwrap();

    let mut events = Vec::new();
    sync_all(&laptop.state, |event| events.push(event)).await;
    assert!(
        laptop
            .state
            .account_info()
            .unwrap()
            .unwrap()
            .update_required
    );
    assert!(events.contains(&SyncEvent::Account));
    // The group is not dropped for being missing from a list this version may misread.
    assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);
    assert_eq!(laptop.group(&trip.id).participants.len(), 2);

    // And the account is not written to: no new group, no other profile.
    let refused = create_group(&laptop.state, "Flat", "EUR", &["Ann".to_string()]);
    assert_eq!(refused.unwrap_err(), account::NEWER_FORMAT);
    assert_eq!(
        update_profile(&laptop.state, "Ann B.", None, None).unwrap_err(),
        account::NEWER_FORMAT
    );
    assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);

    let _ = std::fs::remove_dir_all(relay_dir);
}
