use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn feedback_reaches_who_runs_the_relay() {
    let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let relay = ezcount_sync_server::Relay::open_with(
        &dir.join("relay.sqlite3"),
        ezcount_sync_server::Settings {
            admin_token: Some("s3cret".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(ezcount_sync_server::serve(listener, relay));

    let device = Device::signed_up(&url, "alice").await;
    let state = &device.state;
    assert!(send_feedback(state, "  ", None, None).await.is_err());
    send_feedback(
        state,
        " Budgets, please ",
        Some("alice@example.com"),
        Some("0.3.0"),
    )
    .await
    .unwrap();

    // Only the admin token reads them.
    let read = |token: &'static str| {
        let request = state.http.get(format!("{url}/v1/feedback"));
        async move { request.bearer_auth(token).send().await.unwrap() }
    };
    assert_eq!(read("guess").await.status(), 401);
    let page = state.http.get(format!("{url}/feedback")).send().await;
    assert_eq!(page.unwrap().status(), 200);
    let kept: serde_json::Value = read("s3cret").await.json().await.unwrap();
    assert_eq!(kept[0]["message"], "Budgets, please");
    assert_eq!(kept[0]["contact"], "alice@example.com");
    assert_eq!(kept[0]["app"], "0.3.0");

    // One network can't flood them.
    for _ in 0..4 {
        send_feedback(state, "Again", None, None).await.unwrap();
    }
    let refused = send_feedback(state, "Again", None, None).await.unwrap_err();
    assert!(refused.starts_with("Too many messages"), "{refused}");

    // A relay without a token keeps the messages to itself.
    let (plain, plain_dir) = start_relay().await;
    let other = Device::signed_up(&plain, "bob").await;
    send_feedback(&other.state, "Hello", None, None)
        .await
        .unwrap();
    let answer = other.state.http.get(format!("{plain}/v1/feedback"));
    assert_eq!(answer.send().await.unwrap().status(), 404);
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(plain_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_suggests_exchange_rates() {
    let (rates_url, asked) = start_rate_service();
    let asked = || asked.load(std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let relay = ezcount_sync_server::Relay::open_with(
        &dir.join("relay.sqlite3"),
        ezcount_sync_server::Settings {
            rates_url: Some(rates_url),
            ..Default::default()
        },
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(ezcount_sync_server::serve(listener, relay));

    let device = Device::signed_up(&url, "alice").await;
    let rate = |from: &'static str, date: Option<&'static str>| {
        let state = &device.state;
        async move { suggested_rate(state, from, "EUR", date).await.unwrap() }
    };
    let found = Some("0.85856".to_string());
    assert_eq!(rate("USD", Some("2026-08-29")).await, found);
    // The relay remembers the answer.
    assert_eq!(rate("usd", Some("2026-08-29")).await, found);
    assert_eq!(asked(), 1);
    // A day without a rate gets the latest one.
    assert_eq!(rate("USD", Some("2031-01-01")).await, found);
    assert_eq!(asked(), 3);
    assert_eq!(rate("USD", None).await, found);
    // A currency the service doesn't know, and something that isn't one.
    assert_eq!(rate("XXX", Some("2026-08-29")).await, None);
    assert_eq!(rate("../x", None).await, None);

    // A relay without a rate service, like one from before them, suggests nothing.
    let (plain, plain_dir) = start_relay().await;
    let other = Device::signed_up(&plain, "bob").await;
    assert_eq!(
        suggested_rate(&other.state, "USD", "EUR", None)
            .await
            .unwrap(),
        None
    );
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(plain_dir);
}
