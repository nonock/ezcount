//! What the app refuses to write: names, amounts, currencies, pictures and IBANs that
//! can't be right.

use super::*;

pub(super) fn group_name(name: &str) -> Res<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Group name cannot be empty".to_string());
    }
    Ok(name)
}

/// Checks a picture is one the app stores: a `data:` URL of a JPEG, PNG or WebP, not too big.
pub fn check_image(image: &str) -> Res<()> {
    let is_picture = ["jpeg", "png", "webp"].iter().any(|kind| {
        image
            .strip_prefix("data:image/")
            .and_then(|rest| rest.strip_prefix(kind))
            .and_then(|rest| rest.strip_prefix(";base64,"))
            .is_some_and(|data| {
                !data.is_empty()
                    && data
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
            })
    });
    if !is_picture {
        return Err("This picture can't be used: pick a JPEG, PNG or WebP image".to_string());
    }
    if image.len() > MAX_IMAGE_LEN {
        return Err("This picture is too big".to_string());
    }
    Ok(())
}

/// An IBAN as stored: without spaces, upper-cased, with check digits that fit it.
pub fn check_iban(iban: &str) -> Res<String> {
    let iban = iban
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    let bytes = iban.as_bytes();
    let shaped = (15..=34).contains(&bytes.len())
        && bytes.iter().all(u8::is_ascii_alphanumeric)
        && bytes[..2].iter().all(u8::is_ascii_uppercase)
        && bytes[2..4].iter().all(u8::is_ascii_digit);
    // The country and check digits go last, letters count as 10 to 35, and what that number
    // leaves when divided by 97 is 1.
    let valid = shaped
        && bytes[4..].iter().chain(&bytes[..4]).fold(0u32, |rest, b| {
            if b.is_ascii_digit() {
                (rest * 10 + u32::from(b - b'0')) % 97
            } else {
                (rest * 100 + u32::from(b - b'A') + 10) % 97
            }
        }) == 1;
    if valid {
        Ok(iban)
    } else {
        Err("This IBAN is not valid".to_string())
    }
}

/// A currency code as stored: three letters, upper-cased.
pub(super) fn currency_code(currency: &str) -> Res<String> {
    let code = currency.trim().to_uppercase();
    if code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(code)
    } else {
        Err("The currency must be a three-letter code, such as EUR".to_string())
    }
}

pub(super) fn check_amount(amount_cents: i64) -> Res<()> {
    if amount_cents <= 0 {
        return Err("Amount must be greater than zero".to_string());
    }
    if amount_cents > MAX_AMOUNT_CENTS {
        return Err("Amount is too large".to_string());
    }
    Ok(())
}

/// An exchange rate as stored: a positive decimal number with a point, such as "0.9234".
pub(crate) fn exchange_rate(rate: &str) -> Res<String> {
    let rate = rate.trim().replace(',', ".");
    let (whole, decimals) = rate.split_once('.').unwrap_or((&rate, ""));
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if rate.len() > 20
        || !digits(whole)
        || !digits(decimals)
        || !rate.chars().any(|c| matches!(c, '1'..='9'))
    {
        return Err("The exchange rate must be a number above zero, such as 0.92".to_string());
    }
    Ok(rate)
}

pub(super) fn check_original(original: &OriginalAmount) -> Res<()> {
    check_amount(original.amount_cents)?;
    if currency_code(&original.currency)? != original.currency
        || exchange_rate(&original.rate)? != original.rate
    {
        return Err("The expense's currency or exchange rate is not valid".to_string());
    }
    Ok(())
}

/// What the user typed for an expense in another currency, as stored. An expense in the
/// group's own currency has no original amount.
pub(super) fn normal_original(
    group: &Group,
    original: Option<OriginalAmount>,
) -> Res<Option<OriginalAmount>> {
    let Some(original) = original else {
        return Ok(None);
    };
    let currency = currency_code(&original.currency)?;
    if currency == group.currency {
        return Err(format!(
            "The group is in {currency} already: leave out the exchange rate"
        ));
    }
    check_amount(original.amount_cents)?;
    Ok(Some(OriginalAmount {
        currency,
        amount_cents: original.amount_cents,
        rate: exchange_rate(&original.rate)?,
    }))
}

pub(super) fn money(cents: i128) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

/// `original` is taken as checked. Fixed amounts are in its currency when there is one.
pub(crate) fn check_amounts(
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    splits: &[ExpenseSplit],
) -> Res<()> {
    check_amount(amount_cents)?;
    if splits.is_empty() {
        return Err("Expense must be split among at least one participant".to_string());
    }
    let mut fixed: i128 = 0;
    let mut parts = false;
    for s in splits {
        match s.fixed_cents {
            Some(amount) if amount > 0 && amount <= MAX_AMOUNT_CENTS && s.shares == 0 => {
                fixed += i128::from(amount);
            }
            Some(_) => return Err("A fixed amount must be above zero".to_string()),
            None if s.shares == 0 => return Err("Shares must be at least 1".to_string()),
            None => parts = true,
        }
    }
    let paid = i128::from(original.map_or(amount_cents, |o| o.amount_cents));
    if fixed > paid {
        return Err(format!(
            "The fixed amounts add up to {}, more than the expense's {}",
            money(fixed),
            money(paid)
        ));
    }
    if !parts && fixed != paid {
        return Err(format!(
            "The amounts add up to {}, not the expense's {}",
            money(fixed),
            money(paid)
        ));
    }
    Ok(())
}

/// Longest category key.
pub(super) const MAX_CATEGORY_LEN: usize = 30;

/// A category is a short key ("food", "transport"): lower-case letters, digits, `-` and `_`.
/// The interface names the ones it knows, so that each language has its own words.
pub(super) fn check_category(category: &str) -> Res<()> {
    let valid = !category.is_empty()
        && category.len() <= MAX_CATEGORY_LEN
        && category
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err("This category can't be used".to_string())
    }
}

/// Several payers are at least two different people, `paid_by` first, whose amounts add up to
/// what was paid (`original`'s amount for an expense in another currency).
pub(super) fn check_payers(
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    paid_by: &str,
    payers: &[ExpensePayer],
) -> Res<()> {
    let Some(first) = payers.first() else {
        return Ok(());
    };
    if payers.len() < 2 || first.participant_id != paid_by {
        return Err("The payers of this expense are not valid".to_string());
    }
    let mut seen = HashSet::new();
    if !payers
        .iter()
        .all(|p| seen.insert(p.participant_id.as_str()))
    {
        return Err("A participant appears twice among the payers".to_string());
    }
    if !payers
        .iter()
        .all(|p| p.amount_cents > 0 && p.amount_cents <= MAX_AMOUNT_CENTS)
    {
        return Err("What each payer paid must be above zero".to_string());
    }
    let total: i128 = payers.iter().map(|p| i128::from(p.amount_cents)).sum();
    let paid = i128::from(original.map_or(amount_cents, |o| o.amount_cents));
    if total != paid {
        return Err(format!(
            "The payers paid {} between them, not the expense's {}",
            money(total),
            money(paid)
        ));
    }
    Ok(())
}

/// `grandfathered` lists IDs that may be used even if removed: the people already on an
/// expense being edited, so editing an old expense doesn't force dropping them.
pub(super) fn validate_expense(
    group: &Group,
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    paid_by: &str,
    payers: &[ExpensePayer],
    splits: &[ExpenseSplit],
    grandfathered: &HashSet<&str>,
) -> Res<()> {
    check_amounts(amount_cents, original, splits)?;
    check_payers(amount_cents, original, paid_by, payers)?;
    let mut seen = HashSet::new();
    if !splits
        .iter()
        .all(|s| seen.insert(s.participant_id.as_str()))
    {
        return Err("A participant appears twice in the split".to_string());
    }
    let usable = |id: &str| {
        group
            .participants
            .iter()
            .any(|p| p.id == id && (!p.removed || grandfathered.contains(id)))
    };
    if !usable(paid_by) || !payers.iter().all(|p| usable(&p.participant_id)) {
        return Err("The payer is not an active member of this group".to_string());
    }
    if !splits.iter().all(|s| usable(&s.participant_id)) {
        return Err(
            "The split includes someone who is not an active member of this group".to_string(),
        );
    }
    Ok(())
}
