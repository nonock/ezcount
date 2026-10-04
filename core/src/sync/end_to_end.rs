use super::*;
use crate::models::ExpenseSplit;
use crate::storage::Store;
use std::path::PathBuf;

const PASSWORD: &str = "correct horse battery";

struct Device {
    state: AppState,
    dir: PathBuf,
}

impl Device {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ezcount-e2e-{}", uuid::Uuid::new_v4()));
        let (store, _) = Store::open(&dir.join("db.sqlite3")).unwrap();
        Self {
            state: AppState::new(store, Vec::new()).unwrap(),
            dir,
        }
    }

    async fn signed_up(url: &str, username: &str) -> Self {
        let device = Self::new();
        sign_up(&device.state, url, username, PASSWORD)
            .await
            .unwrap();
        device
    }

    async fn logged_in(url: &str, username: &str) -> Self {
        let device = Self::new();
        log_in(&device.state, url, username, PASSWORD)
            .await
            .unwrap();
        reconcile(&device.state).await.unwrap();
        device
    }

    fn group(&self, id: &str) -> Group {
        self.state.store().group(id).unwrap()
    }

    fn group_ids(&self) -> Vec<String> {
        self.state.store().group_ids()
    }

    fn identity(&self, group_id: &str) -> Option<String> {
        let store = self.state.store();
        account::identities(store.account_doc().unwrap())
            .unwrap()
            .remove(group_id)
    }

    fn create(&self, name: &str, people: &[&str]) -> Group {
        let people: Vec<String> = people.iter().map(|p| p.to_string()).collect();
        create_group(&self.state, name, "EUR", &people).unwrap()
    }

    fn invite(&self, id: &str) -> String {
        self.state.sync_info(id).unwrap().invite_code.unwrap()
    }

    fn edit(&self, id: &str, change: impl FnOnce(&LoroDoc) -> Res<()>) {
        self.state.mutate(id, change).unwrap();
    }

    /// One full round, like the background loop.
    async fn sync(&self) {
        sync_account(&self.state).await.unwrap();
        reconcile(&self.state).await.unwrap();
        let ids = self.state.store().synced_ids();
        let mut dropped = false;
        for id in ids {
            sync_group(&self.state, &id).await.unwrap();
            dropped |= drop_deleted(&self.state, &id);
        }
        // As the next pass of `sync_all` would: the account no longer lists them.
        if dropped {
            sync_account(&self.state).await.unwrap();
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn start_relay() -> (String, PathBuf) {
    let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let relay = ezcount_sync_server::Relay::open(&dir.join("relay.sqlite3")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(ezcount_sync_server::serve(listener, relay));
    (url, dir)
}

/// A stand-in for the rate service: it knows USD to EUR, on any day but in 2031, and
/// counts what it is asked.
fn start_rate_service() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/rate", listener.local_addr().unwrap());
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = std::sync::Arc::clone(&asked);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0u8; 2048];
            let n = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..n]).to_string();
            let path = request.split_whitespace().nth(1).unwrap_or_default();
            count.fetch_add(1, Ordering::SeqCst);
            let (status, body) = if !path.starts_with("/rate/USD/EUR") {
                ("422 Unprocessable", r#"{"status":422}"#.to_string())
            } else if path.contains("date=2031") {
                ("404 Not Found", r#"{"status":404}"#.to_string())
            } else {
                let date = path.split("date=").nth(1).unwrap_or("2026-10-03");
                (
                    "200 OK",
                    format!(r#"{{"date":"{date}","base":"USD","quote":"EUR","rate":0.85856}}"#),
                )
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (url, asked)
}

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

/// A relay that can be swapped for an empty one on the same address, as if its database
/// had been lost.
struct ReplaceableRelay {
    addr: std::net::SocketAddr,
    url: String,
    dir: PathBuf,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl ReplaceableRelay {
    async fn start() -> Self {
        let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Self::spawn(dir, "127.0.0.1:0".parse().unwrap(), "first").await
    }

    async fn spawn(dir: PathBuf, addr: std::net::SocketAddr, db: &str) -> Self {
        let relay = ezcount_sync_server::Relay::open(&dir.join(format!("{db}.sqlite3"))).unwrap();
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(ezcount_sync_server::serve(listener, relay));
        Self {
            addr,
            url: format!("http://{addr}"),
            dir,
            task,
        }
    }

    async fn replace_with_empty(self) -> Self {
        self.task.abort();
        let _ = self.task.await;
        Self::spawn(self.dir, self.addr, "second").await
    }
}

fn split(id: &str) -> ExpenseSplit {
    ExpenseSplit {
        participant_id: id.to_string(),
        shares: 1,
        fixed_cents: None,
    }
}

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

/// A relay with the given limits, taking the client's address from `x-test-client`.
async fn start_limited_relay(limits: ezcount_sync_server::Limits) -> (String, PathBuf) {
    let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let settings = ezcount_sync_server::Settings {
        client_ip_header: Some("x-test-client".parse().unwrap()),
        limits,
        ..Default::default()
    };
    let relay =
        ezcount_sync_server::Relay::open_with(&dir.join("relay.sqlite3"), settings).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(ezcount_sync_server::serve(listener, relay));
    (url, dir)
}

/// Uploads `size` bytes to a document as `client`; returns the HTTP status.
async fn raw_push(url: &str, client: &str, doc: &str, token: &str, size: usize) -> u16 {
    reqwest::Client::new()
        .post(updates_url(url, doc))
        .header("x-test-client", client)
        .bearer_auth(token)
        .body(vec![7u8; size])
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
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

const NEW_PASSWORD: &str = "juniper walrus lantern";

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

/// A relay that never stops paging: every request gets the same real update under a new
/// sequence number, with `has_more`. Returns its URL and how many requests it answered.
fn start_endless_relay(
    group_id: &str,
    secret: &Secret,
    update: &[u8],
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let keys = GroupKeys::derive(secret).unwrap();
    let sealed = STANDARD.encode(keys.seal(group_id, update).unwrap());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = std::sync::Arc::new(AtomicUsize::new(0));
    let answered = requests.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            // Read the whole request, body included, before answering.
            let mut request = Vec::new();
            let mut chunk = [0; 4096];
            while let Ok(n @ 1..) = stream.read(&mut chunk) {
                request.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&request).to_lowercase();
                if let Some(end) = text.find("\r\n\r\n") {
                    let body_len = text
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= end + 4 + body_len {
                        break;
                    }
                }
            }
            let seq = answered.fetch_add(1, Ordering::SeqCst) + 1;
            let body = format!(
                r#"{{"updates":[{{"seq":{seq},"data":"{sealed}"}}],"has_more":true,"relay_id":"endless"}}"#
            );
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (url, requests)
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
