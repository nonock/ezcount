use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_computer_shows_a_code_and_a_phone_sends() {
    let (url, dir) = start_relay().await;
    let phone = Device::signed_up(&url, "alice").await;
    let flat = phone.create("Flat", &["Alice", "Bob"]);
    phone.sync().await;

    // Bob's computer shows a code to join; nothing waits until Alice's phone scans it.
    let computer = Device::signed_up(&url, "bob").await;
    let link = receive_link(&url, "group").unwrap();
    assert_eq!(receive(&computer.state, &link).await.unwrap(), None);
    // A code to join a group doesn't hand an account over.
    assert!(send_login(&phone.state, &link, PASSWORD).await.is_err());
    send_group_invite(&phone.state, &flat.id, &link)
        .await
        .unwrap();
    let got = receive(&computer.state, &link).await.unwrap().unwrap();
    assert_eq!(got.group.unwrap().name, "Flat");
    assert_eq!(computer.group_ids(), vec![flat.id.clone()]);
    // It worked once.
    assert_eq!(receive(&computer.state, &link).await.unwrap(), None);

    // A computer that isn't logged in shows a code on its login screen.
    let laptop = Device::new();
    let link = receive_link(&url, "login").unwrap();
    assert_eq!(receive(&laptop.state, &link).await.unwrap(), None);
    assert_eq!(
        send_login(&phone.state, &link, "not the password")
            .await
            .unwrap_err(),
        "Wrong password"
    );
    send_login(&phone.state, &link, PASSWORD).await.unwrap();
    let got = receive(&laptop.state, &link).await.unwrap().unwrap();
    assert_eq!(got.account.unwrap().username, "alice");
    laptop.sync().await;
    assert_eq!(laptop.group_ids(), vec![flat.id.clone()]);

    // A code from another relay is refused before anything is sent.
    let elsewhere = receive_link("https://other.example.com", "group").unwrap();
    let refused = send_group_invite(&phone.state, &flat.id, &elsewhere).await;
    assert!(refused.unwrap_err().contains("another sync server"));
    assert!(receive_link(&url, "everything").is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_login_link_logs_another_device_in_once() {
    let (url, relay_dir) = start_relay().await;
    let laptop = Device::signed_up(&url, "alice").await;
    let trip = laptop.create("Trip", &["Alice", "Bob"]);
    laptop.sync().await;

    let err = create_login_link(&laptop.state, "not my password")
        .await
        .unwrap_err();
    assert_eq!(err, "Wrong password");
    let link = create_login_link(&laptop.state, PASSWORD).await.unwrap();
    assert_eq!(link.expires_in, 120);
    assert!(link.link.starts_with("ezcount://login?server=http"));

    // The relay is given the ticket, which is neither the code nor in the link.
    let (_, code) = parse_login_link(&link.link).unwrap();
    let ticket = LinkKeys::derive(&code).unwrap().ticket;
    assert!(ticket != code.expose() && !link.link.contains(&ticket));

    let phone = Device::new();
    log_in_with_link(&phone.state, &link.link).await.unwrap();
    reconcile(&phone.state).await.unwrap();
    assert_eq!(phone.group_ids(), vec![trip.id.clone()]);
    assert_eq!(phone.state.store().session().unwrap().username, "alice");
    // It is a full login: the phone's edits reach the laptop.
    phone.edit(&trip.id, |doc| {
        doc::add_participant(doc, "Carol", doc::AddedBy::Member(None)).map(|_| ())
    });
    phone.sync().await;
    laptop.sync().await;
    assert_eq!(laptop.group(&trip.id).participants.len(), 3);

    // A link works once.
    let tablet = Device::new();
    let err = log_in_with_link(&tablet.state, &link.link)
        .await
        .unwrap_err();
    assert!(err.starts_with("This code has expired"), "{err}");
    assert!(tablet.state.store().session().is_none());

    for bad in [
        "hello",
        "ezcount://join?server=x",
        "https://example.com/login?code=x",
    ] {
        let err = log_in_with_link(&tablet.state, bad).await.unwrap_err();
        assert_eq!(err, "This is not an ezcount login code", "{bad}");
    }
    let err = log_in_with_link(&phone.state, &link.link)
        .await
        .unwrap_err();
    assert_eq!(err, "This device is already logged in");

    let _ = std::fs::remove_dir_all(&relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_login_link_expires() {
    let (url, relay_dir) = start_limited_relay(ezcount_sync_server::Limits {
        link_lifetime: Duration::from_millis(50),
        ..Default::default()
    })
    .await;
    let laptop = Device::signed_up(&url, "alice").await;
    let link = create_login_link(&laptop.state, PASSWORD).await.unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;

    let phone = Device::new();
    let err = log_in_with_link(&phone.state, &link.link)
        .await
        .unwrap_err();
    assert!(err.starts_with("This code has expired"), "{err}");

    let _ = std::fs::remove_dir_all(&relay_dir);
}
