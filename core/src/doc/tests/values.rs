use super::*;

#[test]
fn the_greatest_common_divisor() {
    assert_eq!(gcd(12, 18), 6);
    assert_eq!(gcd(7, 5), 1);
    assert_eq!(gcd(0, 9), 9);
    assert_eq!(gcd(9, 0), 9);
}

#[test]
fn shares_older_versions_read_give_the_same_amounts() {
    // The smallest shares in the same proportions.
    assert_eq!(legacy_shares(&[400, 600]), [2, 3]);
    assert_eq!(legacy_shares(&[333, 333, 334]), [333, 333, 334]);
    assert_eq!(legacy_shares(&[500]), [1]);
    assert_eq!(legacy_shares(&[]), Vec::<u32>::new());
}

#[test]
fn someone_owing_nothing_keeps_a_share() {
    // A share of zero isn't read: one in a total counted in cents is the smallest error.
    assert_eq!(legacy_shares(&[0, 1000]), [1, 1000]);
}

#[test]
fn shares_too_large_are_halved_together() {
    let huge = i64::from(u32::MAX) * 2 + 1;
    assert_eq!(legacy_shares(&[huge, 1]), [u32::MAX, 1]);
}

/// A number in one of the splits, as the document holds it.
fn stored(value: &LoroValue, index: usize, key: &str) -> Option<i64> {
    let LoroValue::List(list) = value else {
        panic!("splits are a list");
    };
    let LoroValue::Map(map) = &list[index] else {
        panic!("a split is a map");
    };
    match map.get(key) {
        Some(LoroValue::I64(n)) => Some(*n),
        _ => None,
    }
}

#[test]
fn splits_by_parts_are_only_their_shares() {
    let value = splits_value(1000, None, &[split("a", 1), split("b", 3)]);
    assert_eq!(stored(&value, 1, "shares"), Some(3));
    assert_eq!(stored(&value, 1, "parts"), None);
    assert_eq!(stored(&value, 1, "fixed_cents"), None);
}

#[test]
fn splits_with_fixed_amounts_keep_shares_for_older_versions() {
    // 400 fixed, the 600 left shared 1 to 2: 400, 200 and 400 owed.
    let value = splits_value(1000, None, &[fixed("a", 400), split("b", 1), split("c", 2)]);
    assert_eq!(stored(&value, 0, "fixed_cents"), Some(400));
    assert_eq!(stored(&value, 0, "parts"), Some(0));
    assert_eq!(stored(&value, 2, "parts"), Some(2));
    let shares: Vec<_> = (0..3).map(|i| stored(&value, i, "shares")).collect();
    assert_eq!(shares, [Some(2), Some(1), Some(2)]);
}
