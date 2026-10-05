use super::*;
use axum::http::Request;

const MINUTE: Duration = Duration::from_secs(60);

#[test]
fn a_counter_adds_up_to_its_limit() {
    let counters = Counters::default();
    assert_eq!(counters.count("uploads", MINUTE), 0);
    assert!(counters.try_add("uploads", 60, 100, MINUTE));
    assert!(counters.try_add("uploads", 40, 100, MINUTE));
    assert_eq!(counters.count("uploads", MINUTE), 100);
    // What doesn't fit isn't counted.
    assert!(!counters.try_add("uploads", 1, 100, MINUTE));
    assert_eq!(counters.count("uploads", MINUTE), 100);
}

#[test]
fn each_key_is_counted_apart() {
    let counters = Counters::default();
    assert!(counters.try_add("home", 1, 1, MINUTE));
    assert!(counters.try_add("cafe", 1, 1, MINUTE));
    assert!(!counters.try_add("home", 1, 1, MINUTE));
}

#[test]
fn a_count_starts_again_after_its_window() {
    let counters = Counters::default();
    assert!(counters.try_add("home", 1, 1, Duration::ZERO));
    assert_eq!(counters.count("home", Duration::ZERO), 0);
    assert!(counters.try_add("home", 1, 1, Duration::ZERO));
}

#[test]
fn a_cleared_key_starts_again() {
    let counters = Counters::default();
    assert!(counters.try_add("home", 1, 1, MINUTE));
    counters.clear("home");
    assert_eq!(counters.count("home", MINUTE), 0);
    assert!(counters.try_add("home", 1, 1, MINUTE));
}

#[test]
fn a_count_without_a_limit_stops_at_the_largest_number() {
    let counters = Counters::default();
    assert!(counters.try_add("home", 1, u64::MAX, MINUTE));
    assert!(counters.try_add("home", u64::MAX, u64::MAX, MINUTE));
    assert_eq!(counters.count("home", MINUTE), u64::MAX);
}

fn parts(forwarded: Option<&str>, peer: Option<&str>) -> Parts {
    let mut request = Request::builder();
    if let Some(value) = forwarded {
        request = request.header("x-forwarded-for", value);
    }
    let (mut parts, ()) = request.body(()).unwrap().into_parts();
    if let Some(peer) = peer {
        let address: SocketAddr = peer.parse().unwrap();
        parts.extensions.insert(ConnectInfo(address));
    }
    parts
}

#[test]
fn the_client_is_who_connected() {
    let ip = client_ip(&parts(None, Some("203.0.113.7:5000")), None);
    assert_eq!(ip, Some("203.0.113.7".parse().unwrap()));
    assert_eq!(client_ip(&parts(None, None), None), None);
    // Without a proxy configured, a header anyone can send proves nothing.
    let spoofed = parts(Some("198.51.100.1"), Some("203.0.113.7:5000"));
    assert_eq!(
        client_ip(&spoofed, None),
        Some("203.0.113.7".parse().unwrap())
    );
}

#[test]
fn behind_a_proxy_the_client_is_the_last_address_it_added() {
    let header = HeaderName::from_static("x-forwarded-for");
    let forwarded = parts(Some("198.51.100.1, 203.0.113.7"), Some("10.0.0.1:5000"));
    assert_eq!(
        client_ip(&forwarded, Some(&header)),
        Some("203.0.113.7".parse().unwrap())
    );
    assert_eq!(
        client_ip(&parts(None, Some("10.0.0.1:5000")), Some(&header)),
        None
    );
    assert_eq!(
        client_ip(&parts(Some("not an address"), None), Some(&header)),
        None
    );
}

#[test]
fn the_default_limits_suit_a_small_relay() {
    let limits = Limits::default();
    assert!(limits.max_document_bytes < limits.max_total_bytes);
    assert!(limits.login_failures_per_client < limits.login_failures_per_username);
    assert!(limits.rate_lookups_per_hour < limits.rate_lookups_per_hour_total);
    assert!(limits.feedback_per_hour < limits.feedback_per_hour_total);
}
