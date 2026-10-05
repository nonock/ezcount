use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn devices_upload_again_when_the_relay_loses_its_data() {
    let relay = ReplaceableRelay::start().await;
    let alice = Device::signed_up(&relay.url, "alice").await;
    let group = alice.create("Trip", &["Alice", "Bob"]);
    let (gid, a, b) = (
        group.id.clone(),
        group.participants[0].id.clone(),
        group.participants[1].id.clone(),
    );
    alice.sync().await;
    let bob = Device::signed_up(&relay.url, "bob").await;
    join_group(&bob.state, &alice.invite(&gid)).await.unwrap();
    bob.sync().await;

    let relay = relay.replace_with_empty().await;

    // Bob syncs first: the relay doesn't know the group any more, so he uploads all of it.
    bob.edit(&gid, |d| {
        doc::add_expense(
            d,
            "Museum",
            3000,
            b.clone(),
            vec![split(&a), split(&b)],
            None,
            None,
        )
    });
    bob.sync().await;
    // Alice's position refers to the old database. The new relay id tells her to start
    // over: she uploads everything too, and reads Bob's upload from the beginning.
    alice.edit(&gid, |d| {
        doc::add_expense(
            d,
            "Dinner",
            6000,
            a.clone(),
            vec![split(&a), split(&b)],
            None,
            None,
        )
    });
    alice.sync().await;
    bob.sync().await;

    let (ga, gb) = (alice.group(&gid), bob.group(&gid));
    assert_eq!(ga, gb, "both devices converge again");
    let mut titles: Vec<&str> = ga.expenses.iter().map(|e| e.title.as_str()).collect();
    titles.sort();
    assert_eq!(titles, vec!["Dinner", "Museum"]);

    // The accounts' documents were uploaded again as well, so their other devices can sync.
    let db = rusqlite::Connection::open(relay.dir.join("second.sqlite3")).unwrap();
    let alice_account = alice.state.store().session().unwrap().account_id.clone();
    let known: bool = db
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM groups WHERE id = ?1)",
            [&alice_account],
            |r| r.get(0),
        )
        .unwrap();
    assert!(known);
    // Another round moves nothing.
    assert!(!sync_group(&alice.state, &gid).await.unwrap());

    relay.task.abort();
    let _ = std::fs::remove_dir_all(&relay.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_serves_the_join_page_and_app_links() {
    let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let http = reqwest::Client::new();
    let mut urls = Vec::new();
    for android_app in [
        None,
        Some(ezcount_sync_server::AndroidApp {
            package: "com.example.app".into(),
            cert_sha256: vec!["AB:CD".into()],
        }),
    ] {
        let db = dir.join(format!("{}.sqlite3", urls.len()));
        let settings = ezcount_sync_server::Settings {
            android_app,
            ..Default::default()
        };
        let relay = ezcount_sync_server::Relay::open_with(&db, settings).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        urls.push(format!("http://{}", listener.local_addr().unwrap()));
        tokio::spawn(ezcount_sync_server::serve(listener, relay));
    }

    // The page an invite link opens builds the app link from the fragment.
    let page = http.get(format!("{}/join", urls[0])).send().await.unwrap();
    assert_eq!(page.status(), 200);
    assert!(page.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("default-src 'none'"));
    let html = page.text().await.unwrap();
    assert!(html.contains(r#"new URL("ezcount://join")"#));
    // No web version here: nothing at the root, and the page doesn't offer one.
    assert!(!html.contains("<body data-web>"));
    assert_eq!(http.get(&urls[0]).send().await.unwrap().status(), 404);

    let links = format!("{}/.well-known/assetlinks.json", urls[0]);
    assert_eq!(http.get(links).send().await.unwrap().status(), 404);
    let links = format!("{}/.well-known/assetlinks.json", urls[1]);
    let links: serde_json::Value = http.get(links).send().await.unwrap().json().await.unwrap();
    assert_eq!(links[0]["target"]["package_name"], "com.example.app");
    assert_eq!(links[0]["target"]["sha256_cert_fingerprints"][0], "AB:CD");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_serves_the_web_version() {
    let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
    let web = dir.join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(
        web.join("index.html"),
        "<!doctype html><title>ezcount</title>",
    )
    .unwrap();
    std::fs::write(web.join("assets").join("index-abc.js"), "start()").unwrap();
    std::fs::write(dir.join("secret.txt"), "not for the web").unwrap();
    let settings = ezcount_sync_server::Settings {
        web_dir: Some(web),
        ..Default::default()
    };
    let relay = ezcount_sync_server::Relay::open_with(&dir.join("db.sqlite3"), settings).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(ezcount_sync_server::serve(listener, relay));
    let http = reqwest::Client::new();
    let header =
        |r: &reqwest::Response, name: &str| r.headers()[name].to_str().unwrap().to_string();

    let page = http.get(format!("{url}/")).send().await.unwrap();
    assert_eq!(page.status(), 200);
    assert!(header(&page, "content-type").starts_with("text/html"));
    let csp = header(&page, "content-security-policy");
    assert!(
        csp.contains("script-src 'self' 'wasm-unsafe-eval';"),
        "{csp}"
    );
    assert!(csp.contains("connect-src 'self';"), "{csp}");
    assert_eq!(header(&page, "cache-control"), "no-cache");
    assert_eq!(header(&page, "x-content-type-options"), "nosniff");

    let asset = http
        .get(format!("{url}/assets/index-abc.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(asset.status(), 200);
    assert!(header(&asset, "cache-control").contains("immutable"));
    assert_eq!(header(&asset, "content-security-policy"), csp);
    assert_eq!(asset.text().await.unwrap(), "start()");

    // Nothing outside the web folder, and the API still answers.
    for path in [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/assets/..%2f..%2fsecret.txt",
    ] {
        let response = http.get(format!("{url}{path}")).send().await.unwrap();
        assert_ne!(response.status(), 200, "{path}");
    }
    let health = http.get(format!("{url}/health")).send().await.unwrap();
    assert_eq!(health.text().await.unwrap(), "ok");
    let join = http.get(format!("{url}/join")).send().await.unwrap();
    assert!(join.text().await.unwrap().contains("<body data-web>"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_of_updates_also_ends_at_a_size() {
    let (url, dir) = start_limited_relay(Default::default()).await;
    let token = "t".repeat(43);
    for _ in 0..3 {
        assert_eq!(
            raw_push(&url, "10.0.0.1", "big", &token, 1_500_000).await,
            200
        );
    }
    // How many updates a page after `after` holds, and whether more follow.
    let page = |after: i64| {
        let request = reqwest::Client::new()
            .get(updates_url(&url, "big"))
            .query(&[("after", after)])
            .bearer_auth(&token);
        async move {
            let page: serde_json::Value = request.send().await.unwrap().json().await.unwrap();
            (
                page["updates"].as_array().unwrap().len(),
                page["has_more"].as_bool().unwrap(),
            )
        }
    };
    // Two fit in a page's size; the third comes with the next one.
    assert_eq!(page(0).await, (2, true));
    assert_eq!(page(2).await, (1, false));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_caps_sizes_and_upload_rates() {
    let token = "t".repeat(43);
    let limits = |f: fn(&mut ezcount_sync_server::Limits)| {
        let mut limits = ezcount_sync_server::Limits::default();
        f(&mut limits);
        limits
    };

    // Per document, then in all: 413, then 507. Existing data stays readable.
    let (url, dir) = start_limited_relay(limits(|l| {
        l.max_document_bytes = 1000;
        l.max_total_bytes = 1500;
    }))
    .await;
    assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 600).await, 200);
    assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 600).await, 413);
    assert_eq!(raw_push(&url, "10.0.0.1", "g2", &token, 600).await, 200);
    assert_eq!(raw_push(&url, "10.0.0.2", "g3", &token, 600).await, 507);
    let _ = std::fs::remove_dir_all(&dir);

    // Per client: new documents and uploaded bytes per hour. Other clients are unaffected.
    let (url, dir) = start_limited_relay(limits(|l| {
        l.new_documents_per_hour = 2;
        l.upload_bytes_per_hour = 1000;
    }))
    .await;
    assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 100).await, 200);
    assert_eq!(raw_push(&url, "10.0.0.1", "g2", &token, 100).await, 200);
    assert_eq!(raw_push(&url, "10.0.0.1", "g3", &token, 100).await, 429);
    assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 900).await, 429);
    assert_eq!(raw_push(&url, "10.0.0.2", "g3", &token, 900).await, 200);
    // IPv6 clients count per /64: another address in it shares the quota.
    assert_eq!(
        raw_push(&url, "2001:db8:1:2::1", "g4", &token, 600).await,
        200
    );
    assert_eq!(
        raw_push(&url, "2001:db8:1:2::99", "g4", &token, 600).await,
        429
    );
    assert_eq!(
        raw_push(&url, "2001:db8:1:3::1", "g4", &token, 600).await,
        200
    );
    let _ = std::fs::remove_dir_all(&dir);

    // Sign-ups per client; the app reports the limit.
    let (url, dir) = start_limited_relay(limits(|l| l.sign_ups_per_hour = 1)).await;
    let _alice = Device::signed_up(&url, "alice").await;
    let err = sign_up(&Device::new().state, &url, "bob", PASSWORD)
        .await
        .unwrap_err();
    assert!(err.contains("too many requests"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn relay_sees_neither_data_nor_secrets() {
    use sha2::{Digest, Sha256};

    let (url, relay_dir) = start_relay().await;
    let a = Device::signed_up(&url, "alice").await;
    let marker = "Confidential-Hotel-Name-7391";
    let gid = a.create(marker, &["Zoe-Marker"]).id;
    // Sanity check: an unencrypted Loro update does contain the text.
    let plain = a
        .state
        .store()
        .doc(&gid)
        .unwrap()
        .export(ExportMode::Snapshot)
        .unwrap();
    assert!(plain.windows(marker.len()).any(|w| w == marker.as_bytes()));
    a.sync().await;
    let secret = a.state.store().sync_meta(&gid).unwrap().secret.clone();

    let relay = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
    let blobs: Vec<Vec<u8>> = relay
        .prepare("SELECT data FROM updates UNION ALL SELECT wrapped_key FROM accounts")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(blobs.len() >= 3, "group, account and wrapped key");
    // Neither the data, nor the group key kept in the account, nor the password.
    for blob in &blobs {
        for needle in [
            marker.as_bytes(),
            b"Zoe-Marker",
            secret.expose().as_bytes(),
            PASSWORD.as_bytes(),
        ] {
            assert!(
                !blob.windows(needle.len()).any(|w| w == needle),
                "plaintext leaked"
            );
        }
    }

    // The relay stores the hash of the derived token, never of the secret itself.
    let stored: Vec<u8> = relay
        .query_row("SELECT key_hash FROM groups WHERE id = ?1", [&gid], |r| {
            r.get(0)
        })
        .unwrap();
    let token = GroupKeys::derive(&secret).unwrap().auth_token;
    assert_eq!(stored, Sha256::digest(token.as_bytes()).to_vec());
    assert_ne!(stored, Sha256::digest(secret.expose().as_bytes()).to_vec());
    let login_hash: Vec<u8> = relay
        .query_row("SELECT login_hash FROM accounts", [], |r| r.get(0))
        .unwrap();
    assert_ne!(login_hash, Sha256::digest(PASSWORD.as_bytes()).to_vec());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn tampered_update_is_refused() {
    let (url, relay_dir) = start_relay().await;
    let (a, b) = (
        Device::signed_up(&url, "alice").await,
        Device::signed_up(&url, "bob").await,
    );
    let gid = a.create("Flat", &["Ann"]).id;
    sync_group(&a.state, &gid).await.unwrap();

    // A malicious or faulty relay flips one byte of the stored update.
    let relay = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
    let mut blob: Vec<u8> = relay
        .query_row(
            "SELECT data FROM updates WHERE group_id = ?1",
            [&gid],
            |r| r.get(0),
        )
        .unwrap();
    *blob.last_mut().unwrap() ^= 1;
    relay
        .execute(
            "UPDATE updates SET data = ?1 WHERE group_id = ?2",
            rusqlite::params![blob, gid],
        )
        .unwrap();

    let err = join_group(&b.state, &a.invite(&gid)).await.unwrap_err();
    assert!(err.contains("decrypted"), "{err}");
    assert!(!b.state.store().contains(&gid));

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_key_is_rejected() {
    let (url, relay_dir) = start_relay().await;
    let (a, b) = (
        Device::signed_up(&url, "alice").await,
        Device::signed_up(&url, "bob").await,
    );
    let gid = a.create("Flat", &["Ann"]).id;
    sync_group(&a.state, &gid).await.unwrap();

    let forged = invite_code(&url, &gid, &new_secret().unwrap());
    let err = join_group(&b.state, &forged).await.unwrap_err();
    assert!(err.contains("rejected"), "{err}");

    let missing = invite_code(&url, "no-such-group", &new_secret().unwrap());
    assert!(join_group(&b.state, &missing).await.is_err());

    let _ = std::fs::remove_dir_all(relay_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_that_never_stops_paging_cannot_hold_up_sync() {
    let device = Device::new();
    let doc = doc::new_group_doc("Trip", "EUR", &["Alice".into()]).unwrap();
    let update = doc.export(ExportMode::Snapshot).unwrap();
    let secret = new_secret().unwrap();
    let group = device.state.store().insert(doc, None).unwrap();
    let (url, requests) = start_endless_relay(&group.id, &secret, &update);
    let answered = || requests.load(std::sync::atomic::Ordering::SeqCst);

    let (copy, meta) = download(&device.state.http, &url, &secret, &group.id)
        .await
        .unwrap();
    assert_eq!(answered(), MAX_PAGES_PER_SYNC);
    assert_eq!(meta.cursor, MAX_PAGES_PER_SYNC as i64);
    assert_eq!(doc::read_group(&copy).unwrap(), group);

    device
        .state
        .store()
        .set_sync(&group.id, SyncMeta::new(url, secret))
        .unwrap();
    let before = answered();
    sync_group(&device.state, &group.id).await.unwrap();
    let used = answered() - before;
    // A first check and possibly an upload, then at most one budget of pages.
    assert!(used <= MAX_PAGES_PER_SYNC + 2, "{used} requests");
}
