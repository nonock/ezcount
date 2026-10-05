use super::*;

fn version(text: &str) -> AppVersion {
    text.parse().unwrap()
}

#[test]
fn a_version_is_three_numbers() {
    assert_eq!(version("1.2.3"), AppVersion(1, 2, 3));
    assert_eq!(version(" 0.4.0 "), AppVersion(0, 4, 0));
    assert_eq!(version("12.0.345").to_string(), "12.0.345");
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "1.2.x",
        "v1.2.3",
        "1.2.3-beta",
        "1..3",
        "+1.2.3",
        "1.-2.3",
        "1. 2.3",
    ] {
        assert!(bad.parse::<AppVersion>().is_err(), "{bad}");
    }
}

#[test]
fn versions_are_ordered_number_by_number() {
    assert!(version("0.10.0") > version("0.9.9"));
    assert!(version("1.0.0") > version("0.99.99"));
    assert!(version("0.4.1") > version("0.4.0"));
    assert_eq!(version("0.4.0"), version("0.4.0"));
}

#[test]
fn a_relay_without_a_minimum_answers_every_app() {
    for sent in [None, Some("0.0.1"), Some("nonsense")] {
        assert!(!too_old(None, &Method::POST, "/v1/accounts/login", sent));
    }
}

#[test]
fn an_app_older_than_the_minimum_is_refused() {
    let oldest = Some(version("0.5.0"));
    let refused = |sent| too_old(oldest, &Method::POST, "/v1/accounts/login", sent);
    assert!(refused(Some("0.4.9")));
    assert!(!refused(Some("0.5.0")));
    assert!(!refused(Some("1.0.0")));
    // An app from before versions were sent, or one that sends something else.
    assert!(refused(None));
    assert!(refused(Some("latest")));
}

#[test]
fn only_what_the_app_asks_is_refused() {
    let oldest = Some(version("0.5.0"));
    for (method, path) in [
        (Method::GET, "/v1/groups/g-1/updates"),
        (Method::POST, "/v1/groups/g-1/updates"),
        (Method::POST, "/v1/feedback"),
        (Method::GET, "/v1/rates/USD/EUR"),
    ] {
        assert!(too_old(oldest, &method, path, None), "{method} {path}");
    }
    // Browsers don't say a version: the pages, and the relay's own reading of the feedback.
    for (method, path) in [
        (Method::GET, "/"),
        (Method::GET, "/join"),
        (Method::GET, "/privacy"),
        (Method::GET, "/health"),
        (Method::GET, "/assets/index.js"),
        (Method::GET, "/.well-known/assetlinks.json"),
        (Method::GET, "/v1/feedback"),
    ] {
        assert!(!too_old(oldest, &method, path, None), "{method} {path}");
    }
}
