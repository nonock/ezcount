use super::*;

#[test]
fn ibans_are_checked() {
    assert_eq!(
        check_iban(" fr76 3000 6000 0112 3456 7890 189 ").unwrap(),
        "FR7630006000011234567890189"
    );
    assert!(check_iban("GB82WEST12345698765432").is_ok());
    for wrong in [
        "",
        "FR76",
        "GB82WEST12345698765433",
        "1234567890123456",
        "FR76 30é0",
    ] {
        assert!(check_iban(wrong).is_err(), "{wrong}");
    }
    let (doc, g) = sample();
    let alice = &g.participants[0].id;
    set_participant_iban(&doc, alice, Some("de89 3704 0044 0532 0130 00")).unwrap();
    let iban = read_group(&doc).unwrap().participants.remove(0).iban;
    assert_eq!(iban.as_deref(), Some("DE89370400440532013000"));
    set_participant_iban(&doc, alice, None).unwrap();
    assert_eq!(read_group(&doc).unwrap().participants[0].iban, None);
}

#[test]
fn a_group_needs_a_name() {
    assert_eq!(group_name("  Trip "), Ok("Trip"));
    assert!(group_name("   ").is_err());
}

#[test]
fn a_picture_is_a_small_data_url_of_an_image() {
    for kind in ["jpeg", "png", "webp"] {
        assert!(check_image(&format!("data:image/{kind};base64,AAAA+/==")).is_ok());
    }
    for bad in [
        "",
        "https://example.com/cat.png",
        "data:image/svg+xml;base64,AAAA",
        "data:image/png;base64,",
        "data:image/png;base64,AA AA",
        "data:image/png,AAAA",
    ] {
        assert!(check_image(bad).is_err(), "{bad}");
    }
    let big = format!("data:image/png;base64,{}", "A".repeat(MAX_IMAGE_LEN));
    assert_eq!(
        check_image(&big),
        Err("This picture is too big".to_string())
    );
}

#[test]
fn a_currency_is_three_letters() {
    assert_eq!(currency_code(" eur "), Ok("EUR".to_string()));
    for bad in ["", "EU", "EURO", "E1R", "€€€"] {
        assert!(currency_code(bad).is_err(), "{bad}");
    }
}

#[test]
fn an_amount_is_above_zero_and_not_absurd() {
    assert!(check_amount(1).is_ok());
    assert!(check_amount(MAX_AMOUNT_CENTS).is_ok());
    assert_eq!(
        check_amount(0),
        Err("Amount must be greater than zero".to_string())
    );
    assert!(check_amount(-5).is_err());
    assert_eq!(
        check_amount(MAX_AMOUNT_CENTS + 1),
        Err("Amount is too large".to_string())
    );
}

#[test]
fn an_exchange_rate_is_a_positive_decimal() {
    assert_eq!(exchange_rate(" 0,9234 "), Ok("0.9234".to_string()));
    assert_eq!(exchange_rate("150"), Ok("150".to_string()));
    assert_eq!(exchange_rate(".5"), Ok(".5".to_string()));
    for bad in [
        "",
        "0",
        "0.00",
        "-1",
        "1e3",
        "1.2.3",
        "abc",
        "123456789012345678901",
    ] {
        assert!(exchange_rate(bad).is_err(), "{bad}");
    }
}

#[test]
fn an_original_amount_is_stored_as_typed_once_normal() {
    let (_, group) = sample();
    let typed = |currency: &str, rate: &str| OriginalAmount {
        currency: currency.to_string(),
        amount_cents: 1000,
        rate: rate.to_string(),
    };
    assert_eq!(normal_original(&group, None), Ok(None));
    assert_eq!(
        normal_original(&group, Some(typed("usd", "0,9"))),
        Ok(Some(typed("USD", "0.9")))
    );
    // The group's own currency needs no rate.
    assert!(normal_original(&group, Some(typed("eur", "1"))).is_err());
    assert!(normal_original(&group, Some(typed("USD", "zero"))).is_err());

    assert!(check_original(&typed("USD", "0.9")).is_ok());
    // Stored values are already normal: anything else was not written by the app.
    assert!(check_original(&typed("usd", "0.9")).is_err());
    assert!(check_original(&typed("USD", "0,9")).is_err());
}

#[test]
fn money_is_written_with_two_decimals() {
    assert_eq!(money(0), "0.00");
    assert_eq!(money(5), "0.05");
    assert_eq!(money(123_450), "1234.50");
}

#[test]
fn the_amounts_of_a_split_fit_the_expense() {
    assert!(check_amounts(1000, None, &[split("a", 1), split("b", 2)]).is_ok());
    assert!(check_amounts(1000, None, &[fixed("a", 400), split("b", 1)]).is_ok());
    assert!(check_amounts(1000, None, &[fixed("a", 400), fixed("b", 600)]).is_ok());

    assert_eq!(
        check_amounts(1000, None, &[]),
        Err("Expense must be split among at least one participant".to_string())
    );
    assert_eq!(
        check_amounts(1000, None, &[split("a", 0)]),
        Err("Shares must be at least 1".to_string())
    );
    assert_eq!(
        check_amounts(1000, None, &[fixed("a", 0), split("b", 1)]),
        Err("A fixed amount must be above zero".to_string())
    );
    assert_eq!(
        check_amounts(1000, None, &[fixed("a", 1200), split("b", 1)]),
        Err("The fixed amounts add up to 12.00, more than the expense's 10.00".to_string())
    );
    assert_eq!(
        check_amounts(1000, None, &[fixed("a", 400), fixed("b", 500)]),
        Err("The amounts add up to 9.00, not the expense's 10.00".to_string())
    );
    // A fixed amount with parts as well is neither.
    let both = ExpenseSplit {
        participant_id: "a".to_string(),
        shares: 2,
        fixed_cents: Some(400),
    };
    assert!(check_amounts(1000, None, &[both, split("b", 1)]).is_err());
}

#[test]
fn fixed_amounts_are_in_the_currency_paid() {
    let original = OriginalAmount {
        currency: "USD".to_string(),
        amount_cents: 2000,
        rate: "0.5".to_string(),
    };
    assert!(check_amounts(1000, Some(&original), &[fixed("a", 2000)]).is_ok());
    assert!(check_amounts(1000, Some(&original), &[fixed("a", 1000)]).is_err());
}

#[test]
fn a_category_is_a_short_key() {
    for good in ["food", "eating-out", "kids_2"] {
        assert!(check_category(good).is_ok(), "{good}");
    }
    let long = "a".repeat(MAX_CATEGORY_LEN + 1);
    for bad in ["", "Food", "eating out", "café", long.as_str()] {
        assert!(check_category(bad).is_err(), "{bad}");
    }
}

#[test]
fn several_payers_add_up_to_what_was_paid() {
    let payer = |id: &str, amount_cents: i64| ExpensePayer {
        participant_id: id.to_string(),
        amount_cents,
    };
    assert!(check_payers(1000, None, "a", &[]).is_ok());
    assert!(check_payers(1000, None, "a", &[payer("a", 600), payer("b", 400)]).is_ok());

    let invalid = Err("The payers of this expense are not valid".to_string());
    assert_eq!(check_payers(1000, None, "a", &[payer("a", 1000)]), invalid);
    // `paid_by` is the first of them.
    assert_eq!(
        check_payers(1000, None, "b", &[payer("a", 600), payer("b", 400)]),
        invalid
    );
    assert_eq!(
        check_payers(1000, None, "a", &[payer("a", 600), payer("a", 400)]),
        Err("A participant appears twice among the payers".to_string())
    );
    assert_eq!(
        check_payers(1000, None, "a", &[payer("a", 1000), payer("b", 0)]),
        Err("What each payer paid must be above zero".to_string())
    );
    assert_eq!(
        check_payers(1000, None, "a", &[payer("a", 600), payer("b", 300)]),
        Err("The payers paid 9.00 between them, not the expense's 10.00".to_string())
    );
}
