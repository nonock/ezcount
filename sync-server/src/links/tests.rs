use super::*;

const MINUTE: Duration = Duration::from_secs(60);

#[test]
fn a_ticket_is_random_bytes_in_base64url() {
    for good in ["a".repeat(32), "A1-_".repeat(11), "z".repeat(64)] {
        assert!(check_ticket(&good).is_ok(), "{good}");
    }
    for bad in [
        "".to_string(),
        "a".repeat(31),
        "a".repeat(65),
        "+".repeat(32),
        "é".repeat(20),
    ] {
        assert!(
            matches!(check_ticket(&bad), Err(ApiError::BadRequest(_))),
            "{bad}"
        );
    }
}

#[test]
fn what_waits_under_a_ticket_is_handed_out_once() {
    let links = Links::default();
    assert!(links.put("ticket".to_string(), vec![1, 2, 3], MINUTE));
    assert_eq!(links.take("ticket"), Some(vec![1, 2, 3]));
    assert_eq!(links.take("ticket"), None);
    assert_eq!(links.take("another"), None);
}

#[test]
fn a_ticket_already_taken_keeps_what_it_holds() {
    let links = Links::default();
    assert!(links.put("ticket".to_string(), vec![1], MINUTE));
    assert!(!links.put("ticket".to_string(), vec![2], MINUTE));
    assert_eq!(links.take("ticket"), Some(vec![1]));
}

#[test]
fn a_link_expires() {
    let links = Links::default();
    assert!(links.put("ticket".to_string(), vec![1], Duration::ZERO));
    assert_eq!(links.take("ticket"), None);
    // An expired link frees its ticket.
    assert!(links.put("ticket".to_string(), vec![2], Duration::ZERO));
    assert!(links.put("ticket".to_string(), vec![3], MINUTE));
    assert_eq!(links.take("ticket"), Some(vec![3]));
}

#[test]
fn only_so_many_links_wait_at_once() {
    let links = Links::default();
    for i in 0..MAX_WAITING {
        assert!(links.put(format!("ticket-{i}"), Vec::new(), MINUTE));
    }
    assert!(!links.put("one-more".to_string(), Vec::new(), MINUTE));
    links.take("ticket-0");
    assert!(links.put("one-more".to_string(), Vec::new(), MINUTE));
}
