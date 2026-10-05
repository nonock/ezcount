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

const NEW_PASSWORD: &str = "juniper walrus lantern";

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

mod accounts;
mod groups;
mod links;
mod relay;
mod services;
