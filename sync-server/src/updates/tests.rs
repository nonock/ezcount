use super::*;

#[test]
fn a_group_id_is_short_and_plain() {
    for good in [
        "g",
        "g-1_A",
        "3f2c1a9e-7b1d-4c58-9d0e-5a6b7c8d9e0f",
        &"a".repeat(64),
    ] {
        assert!(check_group_id(good).is_ok(), "{good}");
    }
    for bad in ["", "a/b", "a b", "é", "../etc", &"a".repeat(65)] {
        assert!(
            matches!(check_group_id(bad), Err(ApiError::BadRequest(_))),
            "{bad}"
        );
    }
}

fn authorization(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, value.parse().unwrap());
    headers
}

#[test]
fn the_key_is_a_bearer_token_kept_hashed() {
    let hash = key_hash(&authorization("Bearer 0123456789abcdef")).unwrap();
    assert_eq!(hash, Sha256::digest(b"0123456789abcdef").to_vec());
    let other = key_hash(&authorization("Bearer 0123456789abcdeg")).unwrap();
    assert_ne!(hash, other);
}

#[test]
fn a_missing_or_short_key_is_refused() {
    for headers in [
        HeaderMap::new(),
        authorization("Bearer short"),
        authorization("Basic 0123456789abcdef"),
        authorization("0123456789abcdef"),
    ] {
        assert!(matches!(key_hash(&headers), Err(ApiError::Unauthorized)));
    }
}

#[test]
fn the_stored_hash_of_a_group_the_relay_knows() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE groups (id TEXT PRIMARY KEY, key_hash BLOB NOT NULL)")
        .unwrap();
    db.execute(
        "INSERT INTO groups (id, key_hash) VALUES (?1, ?2)",
        params!["g-1", vec![1u8, 2, 3]],
    )
    .unwrap();
    assert_eq!(stored_hash(&db, "g-1").unwrap(), Some(vec![1, 2, 3]));
    assert_eq!(stored_hash(&db, "g-2").unwrap(), None);
}

#[test]
fn the_id_of_a_deleted_document_is_remembered() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE deleted_documents (id TEXT PRIMARY KEY);
         INSERT INTO deleted_documents (id) VALUES ('a-1');",
    )
    .unwrap();
    assert!(was_deleted(&db, "a-1").unwrap());
    assert!(!was_deleted(&db, "a-2").unwrap());
}
