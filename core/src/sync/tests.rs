use super::*;

#[test]
fn invite_code_round_trips() {
    let secret = new_secret().unwrap();
    for server in [
        "https://sync.example.com",
        "http://192.168.1.10:8787",
        "https://example.com/relay",
    ] {
        let code = invite_code(server, "g-1", &secret);
        assert!(code.starts_with(&format!("{server}/join#")), "{code}");
        let invite = parse_invite(&format!("  {code}\n")).unwrap();
        assert_eq!(invite.server_url, server);
        assert_eq!(invite.group_id, "g-1");
        assert_eq!(invite.secret, secret);
    }
}

#[test]
fn accepts_app_links_from_the_join_page() {
    let secret = new_secret().unwrap();
    let mut url = Url::parse("ezcount://join").unwrap();
    url.query_pairs_mut()
        .append_pair("server", "https://sync.example.com")
        .append_pair("group", "g-1")
        .append_pair("key", secret.expose())
        .append_pair("v", "2");
    let invite = parse_invite(url.as_str()).unwrap();
    assert_eq!(invite.server_url, "https://sync.example.com");
    assert_eq!(invite.group_id, "g-1");
    assert_eq!(invite.secret, secret);
}

#[test]
fn rejects_bad_invites_and_urls() {
    let secret = new_secret().unwrap();
    let without_version = invite_code("http://a", "g", &secret).replace("v=2&", "");
    let err = parse_invite(&without_version).err().unwrap();
    assert!(err.contains("different version"), "{err}");
    assert!(parse_invite(&invite_code(
        "http://a",
        "g",
        &Secret::new("too-short".into())
    ))
    .is_err());
    assert!(parse_invite("https://example.com/join?group=x").is_err());
    let elsewhere = invite_code("http://a", "g", &secret).replace("/join#", "/other#");
    assert!(parse_invite(&elsewhere).is_err());
    assert!(parse_invite("ezcount://join?server=http%3A%2F%2Fa&group=g").is_err());
    assert!(normalize_server_url("ftp://example.com").is_err());
    assert_eq!(
        normalize_server_url(" http://192.168.1.10:8787/ ").unwrap(),
        "http://192.168.1.10:8787"
    );
}

#[test]
fn plain_http_only_on_this_device_or_a_private_network() {
    for local in [
        "http://localhost:8787",
        "http://127.0.0.1:8787",
        "http://192.168.1.10:8787",
        "http://10.0.2.2:8787",
        "http://172.20.0.5",
        "http://100.101.1.2:8787",
        "http://[::1]:8787",
        "http://[fd12::1]",
        "http://my-laptop.local:8787",
        "https://ezcount-relay.fly.dev",
        "https://203.0.113.9",
    ] {
        assert!(normalize_server_url(local).is_ok(), "{local}");
    }
    for public in [
        "http://ezcount-relay.fly.dev",
        "http://203.0.113.9:8787",
        "http://8.8.8.8",
        "http://[2001:db8::1]",
        "http://localhost.example.com",
    ] {
        let err = normalize_server_url(public).unwrap_err();
        assert!(err.contains("https://"), "{public}: {err}");
    }
    // Invites to such a server are refused too.
    let invite = invite_code("http://example.com", "g", &new_secret().unwrap());
    assert!(parse_invite(&invite).err().unwrap().contains("https://"));
}

#[test]
fn secrets_stay_out_of_debug_output() {
    let secret = new_secret().unwrap();
    let meta = SyncMeta::new("https://relay".into(), secret.clone());
    let printed = format!("{secret:?} {meta:?}");
    assert!(!printed.contains(secret.expose()), "{printed}");
    assert!(printed.contains("Secret(…)"));
}

#[test]
fn secrets_are_unique() {
    assert_ne!(new_secret().unwrap(), new_secret().unwrap());
}

#[test]
fn usernames_are_normalized_and_checked() {
    assert_eq!(normalize_username("  Alice.B ").unwrap(), "alice.b");
    assert_eq!(normalize_username("bob_42").unwrap(), "bob_42");
    for bad in ["ab", "-alice", "al ice", "élodie", &"x".repeat(33)] {
        assert!(normalize_username(bad).is_err(), "{bad}");
    }
}

#[test]
fn weak_passwords_are_refused() {
    for weak in [
        "password",
        "password123",
        "qwertyuiop",
        "alice2024",
        "ezcount2024",
    ] {
        let strength = password_strength(weak, "alice");
        assert!(!strength.acceptable, "{weak}: score {}", strength.score);
        assert!(check_password(weak, "alice").is_err(), "{weak}");
    }
    let common = password_strength("password", "alice");
    assert_eq!(common.score, 0);
    assert_eq!(
        common.warning.as_deref(),
        Some("This is a top-10 common password.")
    );
    let err = check_password("password", "alice").unwrap_err();
    assert!(
        err.contains("too easy to guess") && err.contains("top-10"),
        "{err}"
    );
    assert!(check_password("short", "alice")
        .unwrap_err()
        .contains("at least"));

    for strong in [
        "correct horse battery",
        "tangerine kayak mosaic",
        "v8#Lq2!mZr9@wT",
    ] {
        let strength = password_strength(strong, "alice");
        assert!(strength.acceptable, "{strong}: score {}", strength.score);
        assert!(check_password(strong, "alice").is_ok(), "{strong}");
    }
    // A strong password is still refused when it's mostly the username.
    assert!(!password_strength("maximilianmaximilian", "maximilian").acceptable);
}
