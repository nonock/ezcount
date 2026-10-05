use super::*;

#[test]
fn a_currency_is_a_three_letter_code() {
    assert_eq!(currency("eur").unwrap(), "EUR");
    assert_eq!(currency("USD").unwrap(), "USD");
    for bad in ["", "EU", "EURO", "E1R", "€€€", "a/b"] {
        assert!(
            matches!(currency(bad), Err(ApiError::BadRequest(_))),
            "{bad}"
        );
    }
}

#[test]
fn a_date_is_a_year_a_month_and_a_day() {
    assert!(is_date("2026-03-15"));
    for bad in [
        "",
        "2026-3-15",
        "15/03/2026",
        "2026-03-15T10:00",
        "2026-03-1x",
        "20260315--",
    ] {
        assert!(!is_date(bad), "{bad}");
    }
}
