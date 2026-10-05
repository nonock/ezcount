use super::*;
use crate::{Limits, Settings};

#[test]
fn usernames_follow_the_apps_rule() {
    for good in ["abc", "alice", "a.b-c_1", "007", &"a".repeat(32)] {
        assert!(check_username(good).is_ok(), "{good}");
    }
    for bad in [
        "",
        "ab",
        "Alice",
        ".alice",
        "-alice",
        "al ice",
        "élise",
        &"a".repeat(33),
    ] {
        assert!(
            matches!(check_username(bad), Err(ApiError::BadRequest(_))),
            "{bad}"
        );
    }
}

#[test]
fn tokens_are_kept_hashed() {
    let token = "0123456789abcdef";
    assert_eq!(
        token_hash(token).unwrap(),
        Sha256::digest(token.as_bytes()).to_vec()
    );
    assert!(matches!(
        token_hash("too short"),
        Err(ApiError::BadRequest(_))
    ));
}

#[test]
fn a_wrapped_key_is_base64_of_a_reasonable_size() {
    assert_eq!(wrapped_key("AQID").unwrap(), [1, 2, 3]);
    let largest = STANDARD.encode(vec![0u8; 1024]);
    assert_eq!(wrapped_key(&largest).unwrap().len(), 1024);
    let too_large = STANDARD.encode(vec![0u8; 1025]);
    for bad in ["", "not base64!", too_large.as_str()] {
        assert!(matches!(wrapped_key(bad), Err(ApiError::BadRequest(_))));
    }
}

#[test]
fn a_token_comes_with_its_wrapped_key() {
    let token = "0123456789abcdef".to_string();
    let wrapped = "AQID".to_string();
    let (hash, key) = credential(Some(&token), Some(&wrapped)).unwrap().unwrap();
    assert_eq!(hash, token_hash(&token).unwrap());
    assert_eq!(key, [1, 2, 3]);
    assert!(credential(None, None).unwrap().is_none());
    assert!(credential(Some(&token), None).is_err());
    assert!(credential(None, Some(&wrapped)).is_err());
    // Both there, and both checked.
    assert!(credential(Some(&"short".to_string()), Some(&wrapped)).is_err());
}

#[test]
fn failed_logins_are_counted_per_network_and_per_username() {
    let home = Client("203.0.113.7".to_string());
    assert_eq!(
        login_keys("alice", &home),
        (
            "login alice 203.0.113.7".to_string(),
            "login alice".to_string()
        )
    );
}

/// A relay in memory that locks a network out after 2 failed logins, a username after 3.
fn strict_relay() -> Arc<Relay> {
    let settings = Settings {
        limits: Limits {
            login_failures_per_client: 2,
            login_failures_per_username: 3,
            ..Limits::default()
        },
        ..Settings::default()
    };
    Relay::open_with(std::path::Path::new(":memory:"), settings).unwrap()
}

#[test]
fn a_deleted_account_leaves_the_id_of_its_document_and_the_groups() {
    let relay = strict_relay();
    let db = relay.db.lock().unwrap();
    db.execute_batch(
        "INSERT INTO accounts (username, account_id, login_hash, wrapped_key)
             VALUES ('alice', 'a-1', x'01', x'02');
         INSERT INTO groups (id, key_hash, bytes) VALUES ('a-1', x'03', 7), ('g-1', x'04', 5);
         INSERT INTO updates (group_id, data)
             VALUES ('a-1', x'00000000000000'), ('g-1', x'0000000000');",
    )
    .unwrap();
    let count = |table: &str| -> i64 {
        db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    };

    assert_eq!(remove_account(&db, "alice", "a-1").unwrap(), 7);
    assert_eq!(count("accounts"), 0);
    // The group is its members': the relay can't tell it was Alice's too.
    assert_eq!(count("groups"), 1);
    assert_eq!(count("updates"), 1);
    assert!(was_deleted(&db, "a-1").unwrap());
    assert!(!was_deleted(&db, "g-1").unwrap());
    // Asked twice, nothing more goes.
    assert_eq!(remove_account(&db, "alice", "a-1").unwrap(), 0);
    assert_eq!(count("groups"), 1);
}

#[test]
fn too_many_failures_lock_one_network_out_of_one_username() {
    let relay = strict_relay();
    let (home, cafe) = (Client("home".to_string()), Client("cafe".to_string()));
    relay.record_login("alice", &home, false);
    assert!(relay.check_throttle("alice", &home).is_ok());
    relay.record_login("alice", &home, false);
    assert!(matches!(
        relay.check_throttle("alice", &home),
        Err(ApiError::TooManyAttempts)
    ));
    // Alice can still log in from elsewhere, and others from there.
    assert!(relay.check_throttle("alice", &cafe).is_ok());
    assert!(relay.check_throttle("bob", &home).is_ok());
}

#[test]
fn a_login_that_works_forgives_that_networks_failures() {
    let relay = strict_relay();
    let home = Client("home".to_string());
    relay.record_login("alice", &home, false);
    relay.record_login("alice", &home, true);
    relay.record_login("alice", &home, false);
    assert!(relay.check_throttle("alice", &home).is_ok());
}

#[test]
fn guesses_from_many_networks_lock_the_username() {
    let relay = strict_relay();
    for network in ["a", "b", "c"] {
        relay.record_login("alice", &Client(network.to_string()), false);
    }
    assert!(matches!(
        relay.check_throttle("alice", &Client("d".to_string())),
        Err(ApiError::TooManyAttempts)
    ));
    assert!(relay
        .check_throttle("bob", &Client("d".to_string()))
        .is_ok());
}
