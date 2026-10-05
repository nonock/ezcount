use super::*;
use crate::doc;
use crate::engine::calculate_balances;

const HEADER: &str = "Date,Title,Amount,Currency,Paid by,Type,Original amount,\
                      Original currency,Exchange rate,Split";

fn group_of(file: &str) -> Group {
    let imported = import(file).unwrap();
    let doc = doc::imported_group_doc(
        "Imported",
        &imported.currency,
        &imported.participants,
        &imported.expenses,
    )
    .unwrap();
    doc::read_group(&doc).unwrap()
}

#[test]
fn categories_survive_export_and_import() {
    let file = format!(
        "{HEADER},Category,Alice,Bob\n\
         2026-08-29T10:00:00Z,Dinner,30.00,EUR,Alice,expense,,,,1 1,Food,15.00,15.00\n\
         2026-08-30T10:00:00Z,Taxi,10.00,EUR,Bob,expense,,,,1 1,,5.00,5.00\n"
    );
    let group = group_of(&file);
    let categories = |group: &Group| -> Vec<Option<String>> {
        group.expenses.iter().map(|e| e.category.clone()).collect()
    };
    assert_eq!(group.participants.len(), 2, "Category is not a person");
    assert_eq!(categories(&group), vec![Some("food".to_string()), None]);

    let exported = export(&group).unwrap();
    assert!(exported.starts_with(&format!("{HEADER},Category,Alice,Bob")));
    assert_eq!(categories(&group_of(&exported)), categories(&group));

    // A file from before categories has the people right after Split.
    let old = format!(
        "{HEADER},Alice,Bob\n2026-08-29T10:00:00Z,Dinner,30.00,EUR,Alice,expense,,,,1 1,15.00,15.00\n"
    );
    let group = group_of(&old);
    assert_eq!(group.participants.len(), 2);
    assert_eq!(categories(&group), vec![None]);
}

#[test]
fn several_payers_survive_export_and_import() {
    let file = format!(
        "{HEADER},Alice,Bob,Carol\n\
         2026-08-29T10:00:00Z,Dinner,90.00,EUR,Alice=30.00 + Bob=60.00,expense,,,,1 1 1,30.00,30.00,30.00\n\
         2026-08-30T10:00:00Z,Taxi,46.17,EUR,Bob=40.00 + Carol=10.00,expense,50.00,USD,0.9234,1 1 -,23.09,23.08,\n"
    );
    let group = group_of(&file);
    let payers = |i: usize| -> Vec<(String, i64)> {
        group.expenses[i]
            .payers
            .iter()
            .map(|p| {
                let name = &group.participants.iter().find(|x| x.id == p.participant_id);
                (name.unwrap().name.clone(), p.amount_cents)
            })
            .collect()
    };
    // Who paid the most first.
    assert_eq!(
        payers(0),
        vec![("Bob".to_string(), 6000), ("Alice".to_string(), 3000)]
    );
    assert_eq!(
        payers(1),
        vec![("Bob".to_string(), 4000), ("Carol".to_string(), 1000)]
    );
    assert_eq!(
        nets(&group),
        vec![
            ("Alice".to_string(), 3000 - 3000 - 2309),
            ("Bob".to_string(), 6000 + 3694 - 3000 - 2308),
            ("Carol".to_string(), 923 - 3000),
        ]
    );

    let again = group_of(&export(&group).unwrap());
    assert_eq!(nets(&again), nets(&group));
    assert!(export(&group).unwrap().contains("Bob=60.00 + Alice=30.00"));

    // Amounts that aren't the expense's, or someone without a column.
    for bad in [
        "Alice=30.00 + Bob=50.00",
        "Alice=30.00 + Dave=60.00",
        "Alice + Bob",
    ] {
        let file = format!(
            "{HEADER},Alice,Bob\n2026-08-29T10:00:00Z,Dinner,90.00,EUR,{bad},expense,,,,1 1,45.00,45.00\n"
        );
        let imported = import(&file)
            .and_then(|g| doc::imported_group_doc("x", &g.currency, &g.participants, &g.expenses));
        assert!(imported.is_err(), "{bad}");
    }
}

/// (name, net balance), in the group's order.
fn nets(group: &Group) -> Vec<(String, i64)> {
    calculate_balances(group)
        .into_iter()
        .map(|b| (b.participant_name, b.net_cents))
        .collect()
}

/// (person's position, shares, fixed amount) of each expense.
fn splits(group: &Group) -> Vec<Vec<Split>> {
    let position = |id: &str| group.participants.iter().position(|p| p.id == id).unwrap();
    group
        .expenses
        .iter()
        .map(|e| {
            e.splits
                .iter()
                .map(|s| (position(&s.participant_id), s.shares, s.fixed_cents))
                .collect()
        })
        .collect()
}

#[test]
fn a_group_survives_export_and_import() {
    let names = ["Alice", "Bob", "=Carol, \"C\""].map(String::from);
    let doc = doc::new_group_doc("Trip", "CHF", &names).unwrap();
    let ids: Vec<String> = doc::read_group(&doc)
        .unwrap()
        .participants
        .into_iter()
        .map(|p| p.id)
        .collect();
    let split = |i: usize, shares: u32| ExpenseSplit {
        participant_id: ids[i].clone(),
        shares,
        fixed_cents: None,
    };
    let fixed = |i: usize, amount: i64| ExpenseSplit {
        participant_id: ids[i].clone(),
        shares: 0,
        fixed_cents: Some(amount),
    };
    let at = |day: u32| parse_date(&format!("2026-03-{day:02}T08:30:00Z"));
    doc::add_expense(
        &doc,
        "Dinner, with \"wine\"",
        1000,
        ids[0].clone(),
        vec![split(0, 1), split(1, 1), split(2, 1)],
        at(1),
        None,
    )
    .unwrap();
    doc::add_expense(
        &doc,
        "-50% tickets",
        13400,
        ids[1].clone(),
        vec![split(1, 2), split(2, 1)],
        at(2),
        None,
    )
    .unwrap();
    // Two fixed amounts, and the rest by parts.
    doc::add_expense(
        &doc,
        "Groceries",
        14660,
        ids[2].clone(),
        vec![fixed(0, 4436), fixed(1, 5550), split(2, 5)],
        at(3),
        None,
    )
    .unwrap();
    // Paid in dollars: the fixed amount is in dollars too.
    doc::add_expense(
        &doc,
        "Taxi",
        4617,
        ids[1].clone(),
        vec![fixed(0, 2000), split(1, 1)],
        at(4),
        Some(OriginalAmount {
            currency: "USD".to_string(),
            amount_cents: 5000,
            rate: "0.9234".to_string(),
        }),
    )
    .unwrap();
    doc::record_reimbursement(&doc, ids[1].clone(), ids[0].clone(), 250, None, None).unwrap();
    // Money that came in: a refund Alice received for the three.
    doc::add_expense_as(
        &doc,
        "Refund",
        900,
        ids[0].clone(),
        vec![split(0, 1), split(1, 1), split(2, 1)],
        at(6),
        None,
        doc::Adding {
            income: true,
            ..doc::Adding::default()
        },
    )
    .unwrap();
    let group = doc::read_group(&doc).unwrap();

    let file = export(&group).unwrap();
    assert!(
        file.contains(",Refund,9.00,CHF,Alice,income,,,,1 1 1,,3.00,3.00,3.00\n"),
        "{file}"
    );
    let expected = format!(
        "{HEADER},Category,Alice,Bob,\"'=Carol, \"\"C\"\"\"\n\
         2026-03-01T08:30:00Z,\"Dinner, with \"\"wine\"\"\",10.00,CHF,Alice,expense,,,,1 1 1,,3.34,3.33,3.33\n\
         2026-03-02T08:30:00Z,'-50% tickets,134.00,CHF,Bob,expense,,,,- 2 1,,,89.33,44.67\n\
         2026-03-03T08:30:00Z,Groceries,146.60,CHF,\"'=Carol, \"\"C\"\"\",expense,,,,44.36 55.50 5,,44.36,55.50,46.74\n\
         2026-03-04T08:30:00Z,Taxi,46.17,CHF,Bob,expense,50.00,USD,0.9234,20.00 1 -,,18.47,27.70,\n"
    );
    assert_eq!(&file[..expected.len()], expected);

    let back = group_of(&file);
    assert_eq!(back.currency, "CHF");
    assert_eq!(nets(&back), nets(&group));
    assert_eq!(splits(&back), splits(&group));
    let fields = |g: &Group| -> Vec<_> {
        g.expenses
            .iter()
            .map(|e| {
                (
                    e.title.clone(),
                    e.amount_cents,
                    e.original.clone(),
                    e.created_at,
                    e.is_reimbursement,
                )
            })
            .collect()
    };
    assert_eq!(fields(&back), fields(&group));
}

#[test]
fn reads_what_spreadsheets_and_other_apps_write() {
    // Semicolons, decimal commas, a BOM, dates in several forms, an empty line, and
    // equal parts whose odd cent went to someone else than here.
    let group = group_of(
        "\u{feff}date;title;amount;currency;paid by;type;original amount;original currency;\
         exchange rate;split;Ann;Ben;Cleo\r\n\
         2026-08-29 02:00:00.000000;Gift;8,50;eur;Ann;;;;;;2,83;2,83;2,84\r\n\
         31/12/2025;Train;1 234,5;EUR;Ben;Expense;;;;;;617,25;617,25\r\n\
         ;;;;;;;;;;;;\r\n\
         2026-01-02;;10;EUR;Cleo;payment;;;;;10;0;\r\n\
         2026-02-01;Hotel;92,34;EUR;Ann;;100;usd;;;46,17;46,17;\r\n",
    );
    assert_eq!(group.currency, "EUR");
    assert_eq!(
        splits(&group),
        [
            vec![(1, 1, None), (2, 1, None)],
            vec![(0, 1, None)],
            vec![(0, 1, None), (1, 1, None)],
            vec![(0, 1, None), (1, 1, None), (2, 1, None)]
        ]
    );
    assert_eq!(group.expenses[1].title, "Payment: Cleo → Ann");
    assert!(group.expenses[1].is_reimbursement);
    assert_eq!(
        group.expenses[1].created_at.to_rfc3339(),
        "2026-01-02T12:00:00+00:00"
    );
    assert_eq!(group.expenses[0].amount_cents, 123_450);
    // The rate is worked out when the file has none.
    assert_eq!(
        group.expenses[2].original,
        Some(OriginalAmount {
            currency: "USD".to_string(),
            amount_cents: 10_000,
            rate: "0.9234".to_string(),
        })
    );
}

#[test]
fn the_split_column_is_a_hint() {
    let file = |line: &str| format!("{HEADER},Ann,Ben,Cleo\n{line}\n");
    let split = |line: &str| splits(&group_of(&file(line))).remove(0);
    // Alone, it says everything.
    assert_eq!(
        split("2026-01-01,Gift,10,EUR,Ann,,,,,2.50 1 2,,,"),
        [(0, 0, Some(250)), (1, 1, None), (2, 2, None)]
    );
    // With amounts a cent off, as another app rounds: still the split.
    assert_eq!(
        split("2026-01-01,Gift,10,EUR,Ann,,,,,1 1 1,3.33,3.33,3.34"),
        [(0, 1, None), (1, 1, None), (2, 1, None)]
    );
    // With other amounts, someone edited them: they win.
    assert_eq!(
        split("2026-01-01,Gift,10,EUR,Ann,,,,,1 1 1,5.00,5.00,"),
        [(0, 1, None), (1, 1, None)]
    );
    assert_eq!(
        split("2026-01-01,Gift,10,EUR,Ann,,,,,1 1 1,1.00,2.50,6.50"),
        [(0, 0, Some(100)), (1, 0, Some(250)), (2, 0, Some(650))]
    );
    let error = |line: &str| import(&file(line)).unwrap_err();
    assert!(error("2026-01-01,Gift,10,EUR,Ann,,,,,1 1,,,").starts_with("Line 2: Split needs"));
    assert_eq!(
        error("2026-01-01,Gift,10,EUR,Ann,,,,,7.00 4.00 -,,,"),
        "Line 2: The fixed amounts add up to 11.00, more than the expense's 10.00"
    );
    assert_eq!(
        error("2026-01-01,Gift,10,EUR,Ann,,,,,,,,"),
        "Line 2: nobody shares this expense"
    );
}

#[test]
fn finds_the_shares_behind_amounts() {
    assert_eq!(small_shares(1000, &[334, 333, 333]), Some(vec![1, 1, 1]));
    assert_eq!(small_shares(13400, &[8933, 4467]), Some(vec![2, 1]));
    assert_eq!(
        small_shares(12149, &[2429, 2430, 4860, 2430]),
        Some(vec![1, 1, 2, 1])
    );
    assert_eq!(small_shares(700, &[300, 400]), Some(vec![3, 4]));
    assert_eq!(small_shares(14660, &[4436, 3830, 1720, 4674]), None);
    // In another currency there are no fixed amounts to fall back on.
    assert_eq!(
        split_from_amounts(14660, true, &[0, 1, 2, 3], &[4436, 3830, 1720, 4674]),
        Some(vec![
            (0, 2218, None),
            (1, 1915, None),
            (2, 860, None),
            (3, 2337, None)
        ])
    );
}

#[test]
fn says_what_is_wrong_with_a_file() {
    let header = format!("{HEADER},Ann,Ben\n");
    let error = |lines: &str| import(&format!("{header}{lines}")).unwrap_err();
    assert!(import("Who,What\nAnn,Gift\n")
        .unwrap_err()
        .starts_with("This is not an ezcount CSV file"));
    assert!(import("")
        .unwrap_err()
        .starts_with("This is not an ezcount"));
    assert_eq!(
        import(&format!("{HEADER},Ann,Ann\n")).unwrap_err(),
        "Two columns are named Ann"
    );
    assert_eq!(
        error("soon,Gift,10,EUR,Ann,,,,,,5,5"),
        "Line 2: \"soon\" is not a date, such as 2026-12-31"
    );
    assert_eq!(
        error("2026-01-01,Gift,-10,EUR,Ann,,,,,,5,5"),
        "Line 2: \"-10\" is not an amount above zero"
    );
    assert_eq!(
        error("2026-01-01,Gift,10,EUR,Zoe,,,,,,5,5"),
        "Line 2: Zoe paid, but has no column"
    );
    assert_eq!(
        error("2026-01-01,Gift,10,EUR,Ann,,,,,,5,4"),
        "Line 2: the people's parts add up to 9.00, not 10.00"
    );
    assert!(
        error("2026-01-01,Gift,10,EUR,Ann,,,,,,5,5\n2026-01-01,Gift,10,USD,Ann,,,,,,5,5")
            .starts_with("Line 3 is in USD, the lines before in EUR")
    );
    assert_eq!(
        error("2026-01-01,Gift,10,EUR,Ann,,12,,,,5,5"),
        "Line 2: the original amount and its currency go together"
    );
    assert!(error("2026-01-01,,10,EUR,Ann,payment,,,,,5,5").starts_with("Line 2: a payment goes"));
    assert!(error("2026-01-01,,10,EUR,Ann,refund,,,,,5,5").starts_with("Line 2: the type is"));
    // A file with only people is a group without expenses.
    assert_eq!(import(&header).unwrap().participants, ["Ann", "Ben"]);
}

#[test]
fn amounts_are_read_as_spreadsheets_write_them() {
    for (text, cents) in [
        ("12.5", 1250),
        ("12,50", 1250),
        ("12", 1200),
        (".5", 50),
        ("1 234,50", 123_450),
        ("1,234.50", 123_450),
        ("1.234,5", 123_450),
        // Three digits after the separator: it groups thousands.
        ("1,234", 123_400),
        ("0", 0),
    ] {
        assert_eq!(parse_cents(text), Some(cents), "{text}");
    }
    for bad in ["", ",", "abc", "-5", "12.5€", "1e3"] {
        assert_eq!(parse_cents(bad), None, "{bad}");
    }
    assert_eq!(parse_cents("99999999999999999999"), None);
}

#[test]
fn amounts_are_written_with_two_decimals() {
    assert_eq!(cents(0), "0.00");
    assert_eq!(cents(7), "0.07");
    assert_eq!(cents(123_450), "1234.50");
}

#[test]
fn dates_are_read_with_or_without_a_time() {
    let at = |text: &str| parse_date(text).map(|d| d.to_rfc3339());
    let noon = Some("2026-12-31T12:00:00+00:00".to_string());
    assert_eq!(
        at("2026-12-31T10:00:00+02:00"),
        Some("2026-12-31T08:00:00+00:00".to_string())
    );
    assert_eq!(
        at("2026-12-31 08:30:00"),
        Some("2026-12-31T08:30:00+00:00".to_string())
    );
    assert_eq!(
        at("31/12/2026 08:30:00"),
        Some("2026-12-31T08:30:00+00:00".to_string())
    );
    // A date alone is noon: the same day in most places.
    assert_eq!(at("2026-12-31"), noon);
    assert_eq!(at("31/12/2026"), noon);
    for bad in ["", "yesterday", "2026-13-01", "12/31/2026"] {
        assert_eq!(at(bad), None, "{bad}");
    }
}

#[test]
fn the_rate_between_two_amounts() {
    assert_eq!(rate_between(4617, 5000), "0.9234");
    assert_eq!(rate_between(1000, 1000), "1");
    assert_eq!(rate_between(15_000, 100), "150");
    // To six decimals, rounded.
    assert_eq!(rate_between(1, 3), "0.333333");
    assert_eq!(rate_between(2, 3), "0.666667");
}

#[test]
fn texts_that_look_like_formulas_are_quoted() {
    for text in ["=SUM(A1)", "+1", "-1", "@home", "\tx", "\rx"] {
        let guarded = guard(text);
        assert_eq!(guarded, format!("'{text}"));
        assert_eq!(unguard(&guarded), text);
    }
    assert_eq!(guard("Dinner"), "Dinner");
    // A quote someone typed stays.
    assert_eq!(unguard("'Dinner'"), "'Dinner'");
    assert_eq!(unguard(""), "");
}

#[test]
fn the_split_column_lists_parts_fixed_amounts_and_who_is_out() {
    assert_eq!(
        parse_split("1 2", 2),
        Some(vec![(0, 1, None), (1, 2, None)])
    );
    assert_eq!(
        parse_split("20.00 1 -", 3),
        Some(vec![(0, 0, Some(2000)), (1, 1, None)])
    );
    // An entry per person, or the column is ignored.
    assert_eq!(parse_split("1 1", 3), None);
    assert_eq!(parse_split("", 2), None);
    assert_eq!(parse_split("x y", 2), None);
}

#[test]
fn the_separator_is_the_one_the_first_line_has_most() {
    assert_eq!(delimiter_of("Date,Title,Amount\n1;2;3;4;5"), b',');
    assert_eq!(delimiter_of("Date;Title;Amount, in euros"), b';');
    assert_eq!(delimiter_of("Date\tTitle\tAmount"), b'\t');
}

#[test]
fn a_file_says_which_line_is_wrong() {
    let people = format!("{HEADER},Alice,Bob");
    let line = |cells: &str| import(&format!("{people}\n{cells}")).unwrap_err();
    assert_eq!(
        line("someday,Taxi,10.00,EUR,Alice,expense,,,,,5.00,5.00"),
        "Line 2: \"someday\" is not a date, such as 2026-12-31"
    );
    assert_eq!(
        line("2026-03-15,Taxi,free,EUR,Alice,expense,,,,,5.00,5.00"),
        "Line 2: \"free\" is not an amount above zero"
    );
    assert_eq!(
        line("2026-03-15,Taxi,10.00,EUR,Carol,expense,,,,,5.00,5.00"),
        "Line 2: Carol paid, but has no column"
    );
    assert_eq!(
        line("2026-03-15,Taxi,10.00,EUR,Alice,gift,,,,,5.00,5.00"),
        "Line 2: the type is \"gift\", not expense, payment or income"
    );
    assert_eq!(
        line("2026-03-15,Taxi,10.00,EUR,Alice,expense,12.00,,,,5.00,5.00"),
        "Line 2: the original amount and its currency go together"
    );
    assert_eq!(
        line("2026-03-15,Taxi,10.00,EUR,Alice,expense,,,,,5.00,4.00"),
        "Line 2: the people's parts add up to 9.00, not 10.00"
    );
    assert_eq!(
        line("2026-03-15,,10.00,EUR,Alice,payment,,,,,5.00,5.00"),
        "Line 2: a payment goes to one person, with the whole amount in their column"
    );
    assert_eq!(
        line("2026-03-15,Taxi,10.00,EUR,Alice,expense,,,,,,"),
        "Line 2: nobody shares this expense"
    );
    // Blank lines are skipped, and still counted as a spreadsheet numbers them.
    assert_eq!(
        line(",,,,,,,,,,,\n2026-03-15,Taxi,10.00,USD,Alice,expense,,,,,5.00,5.00\n2026-03-16,Bus,2.00,EUR,Bob,expense,,,,,1.00,1.00"),
        "Line 4 is in EUR, the lines before in USD: a group has one currency \
         (another one goes in Original currency)"
    );
}
