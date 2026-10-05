//! Reading a CSV file back into a group.

use super::*;

/// A group read from a file. Its name is not in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedGroup {
    pub currency: String,
    pub participants: Vec<String>,
    pub expenses: Vec<ImportedExpense>,
}

/// Reads a group from a CSV file in the format `export` writes.
pub fn import(text: &str) -> Res<ImportedGroup> {
    let text = text.trim_start_matches('\u{feff}');
    let mut records = csv::ReaderBuilder::new()
        .delimiter(delimiter_of(text))
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes())
        .into_records();
    let header = records
        .next()
        .ok_or_else(not_ours)?
        .map_err(|_| not_ours())?;
    let columns = Columns::read(&header)?;

    let mut currency: Option<String> = None;
    let mut expenses = Vec::new();
    for (index, record) in records.enumerate() {
        // As a spreadsheet numbers them, the header being line 1.
        let number = index + 2;
        let record = record.map_err(|e| format!("Line {number} can't be read: {e}"))?;
        if record.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        let line = Line {
            number,
            record: &record,
            columns: &columns,
        };
        expenses.push(line.expense(&mut currency)?);
    }

    Ok(ImportedGroup {
        // A file without expenses doesn't say; the group's currency can be changed later.
        currency: currency.unwrap_or_else(|| "EUR".to_string()),
        participants: columns.participants,
        expenses,
    })
}

/// Spreadsheets save with the separator of their language: the one the first line has most.
pub(super) fn delimiter_of(text: &str) -> u8 {
    let first_line = text.lines().next().unwrap_or_default();
    b",;\t"
        .iter()
        .copied()
        .max_by_key(|d| first_line.bytes().filter(|b| b == d).count())
        .unwrap_or(b',')
}

fn not_ours() -> String {
    format!(
        "This is not an ezcount CSV file: its first line should be {}, then one column per person",
        COLUMNS.join(", ")
    )
}

/// What the header says of the file: who the people are, and where their columns start.
struct Columns {
    participants: Vec<String>,
    has_category: bool,
    first_person: usize,
}

impl Columns {
    fn read(header: &csv::StringRecord) -> Res<Self> {
        if header.len() <= COLUMNS.len()
            || !COLUMNS
                .iter()
                .zip(header.iter())
                .all(|(expected, found)| found.trim().eq_ignore_ascii_case(expected))
        {
            return Err(not_ours());
        }
        // Files from before categories, or from elsewhere, have the people right after.
        let has_category = header
            .get(COLUMNS.len())
            .is_some_and(|name| name.trim().eq_ignore_ascii_case(CATEGORY));
        let first_person = COLUMNS.len() + usize::from(has_category);
        if header.len() <= first_person {
            return Err(not_ours());
        }
        let participants: Vec<String> = header
            .iter()
            .skip(first_person)
            .map(|name| unguard(name.trim()).to_string())
            .collect();
        let mut seen = HashSet::new();
        for name in &participants {
            if name.is_empty() {
                return Err("A person's column has no name".to_string());
            }
            if !seen.insert(name.as_str()) {
                return Err(format!("Two columns are named {name}"));
            }
        }
        Ok(Self {
            participants,
            has_category,
            first_person,
        })
    }

    /// The person with this name, as their place among the people.
    fn person(&self, name: &str) -> Option<usize> {
        self.participants.iter().position(|n| n == name)
    }
}

/// One line of the file: an expense, a payment or money that came in.
struct Line<'a> {
    number: usize,
    record: &'a csv::StringRecord,
    columns: &'a Columns,
}

impl<'a> Line<'a> {
    fn cell(&self, i: usize) -> &'a str {
        self.record.get(i).unwrap_or_default().trim()
    }

    /// What is wrong, with the line it is on.
    fn at(&self, problem: String) -> String {
        format!("Line {}: {problem}", self.number)
    }

    fn expense(&self, currency: &mut Option<String>) -> Res<ImportedExpense> {
        let created_at = self.date()?;
        let amount_cents = self.amount()?;
        self.currency(currency)?;
        let (paid_by, payers) = self.payers()?;
        let (is_reimbursement, income) = self.kind()?;
        let original = self.original(amount_cents)?;
        let (people, owes) = self.people_amounts(amount_cents)?;

        let mut title = unguard(self.cell(1)).to_string();
        let splits = if is_reimbursement {
            let to = self.payment_to(&people)?;
            if title.is_empty() {
                title = payment_title(unguard(self.cell(4)), &self.columns.participants[to]);
            }
            vec![(to, 1, None)]
        } else {
            self.splits(amount_cents, original.as_ref(), &people, &owes)?
        };
        let category = Some(self.cell(COLUMNS.len()).to_lowercase())
            .filter(|category| self.columns.has_category && !category.is_empty());
        Ok(ImportedExpense {
            title,
            category,
            amount_cents,
            original,
            paid_by,
            payers,
            splits,
            created_at,
            is_reimbursement,
            income,
        })
    }

    fn date(&self) -> Res<DateTime<Utc>> {
        let text = self.cell(0);
        parse_date(text)
            .ok_or_else(|| self.at(format!("\"{text}\" is not a date, such as 2026-12-31")))
    }

    fn amount(&self) -> Res<i64> {
        let text = self.cell(2);
        parse_cents(text)
            .filter(|amount| *amount > 0)
            .ok_or_else(|| self.at(format!("\"{text}\" is not an amount above zero")))
    }

    /// The first line that names a currency gives the group's; the others must agree.
    fn currency(&self, known: &mut Option<String>) -> Res<()> {
        let text = self.cell(3);
        if text.is_empty() {
            return Ok(());
        }
        match known {
            None => *known = Some(text.to_uppercase()),
            Some(c) if c.eq_ignore_ascii_case(text) => {}
            Some(c) => {
                return Err(format!(
                    "Line {} is in {text}, the lines before in {c}: a group has one currency \
                     (another one goes in Original currency)",
                    self.number
                ))
            }
        }
        Ok(())
    }

    /// Who paid, and when several did, what each paid.
    fn payers(&self) -> Res<(usize, Vec<(usize, i64)>)> {
        let payer = unguard(self.cell(4));
        if let Some(paid_by) = self.columns.person(payer) {
            return Ok((paid_by, Vec::new()));
        }
        // Not a name: several payers, each with their amount.
        let payers = payer
            .split(SEVERAL_PAYERS)
            .map(|entry| {
                let (name, amount) = entry.rsplit_once('=')?;
                Some((
                    self.columns.person(unguard(name.trim()))?,
                    parse_cents(amount.trim())?,
                ))
            })
            .collect::<Option<Vec<(usize, i64)>>>()
            .filter(|payers| payers.len() > 1)
            .ok_or_else(|| self.at(format!("{payer} paid, but has no column")))?;
        Ok((payers[0].0, payers))
    }

    /// Whether the line is a payment, and whether it is money that came in.
    fn kind(&self) -> Res<(bool, bool)> {
        match self.cell(5).to_lowercase().as_str() {
            "" | EXPENSE => Ok((false, false)),
            PAYMENT => Ok((true, false)),
            INCOME => Ok((false, true)),
            other => Err(self.at(format!(
                "the type is \"{other}\", not {EXPENSE}, {PAYMENT} or {INCOME}"
            ))),
        }
    }

    /// What was paid in another currency. Without a rate, the one the two amounts give.
    fn original(&self, amount_cents: i64) -> Res<Option<OriginalAmount>> {
        match (self.cell(6), self.cell(7)) {
            ("", "") => Ok(None),
            ("", _) | (_, "") => {
                Err(self.at("the original amount and its currency go together".to_string()))
            }
            (amount, currency) => {
                let original_cents = parse_cents(amount)
                    .filter(|amount| *amount > 0)
                    .ok_or_else(|| self.at(format!("\"{amount}\" is not an amount above zero")))?;
                Ok(Some(OriginalAmount {
                    currency: currency.to_uppercase(),
                    amount_cents: original_cents,
                    rate: match self.cell(8) {
                        "" => rate_between(amount_cents, original_cents),
                        rate => rate.to_string(),
                    },
                }))
            }
        }
    }

    /// The people with an amount in their column, and those amounts, which add up to the
    /// line's.
    fn people_amounts(&self, amount_cents: i64) -> Res<(Vec<usize>, Vec<i64>)> {
        let mut people = Vec::new();
        let mut owes = Vec::new();
        for (person, name) in self.columns.participants.iter().enumerate() {
            let text = self.cell(self.columns.first_person + person);
            if text.is_empty() {
                continue;
            }
            let amount = parse_cents(text)
                .ok_or_else(|| self.at(format!("\"{text}\" is not an amount, for {name}")))?;
            if amount > 0 {
                people.push(person);
                owes.push(amount);
            }
        }
        if !people.is_empty() {
            let total = owes
                .iter()
                .try_fold(0i64, |sum, o| sum.checked_add(*o))
                .unwrap_or(i64::MAX);
            if total != amount_cents {
                return Err(self.at(format!(
                    "the people's parts add up to {}, not {}",
                    cents(total),
                    cents(amount_cents)
                )));
            }
        }
        Ok((people, owes))
    }

    /// The one person a payment goes to.
    fn payment_to(&self, people: &[usize]) -> Res<usize> {
        match people {
            [to] => Ok(*to),
            _ => Err(self.at(
                "a payment goes to one person, with the whole amount in their column".to_string(),
            )),
        }
    }

    /// How an expense is split: as the `Split` column says when it gives the people's
    /// amounts, as worked out from those amounts otherwise.
    fn splits(
        &self,
        amount_cents: i64,
        original: Option<&OriginalAmount>,
        people: &[usize],
        owes: &[i64],
    ) -> Res<Vec<Split>> {
        let everyone = self.columns.participants.len();
        let declared = match self.cell(9) {
            "" => None,
            split => Some(parse_split(split, everyone).ok_or_else(|| {
                self.at(format!(
                    "Split needs an entry per person: a number of parts, \
                     an amount such as 12.50, or {NOT_IN}"
                ))
            })?),
        };
        // What the split gives each person, to compare with the file's amounts.
        let gives = |splits: &[Split]| -> Vec<i64> {
            let amounts = owed(
                amount_cents,
                original.map(|o| o.amount_cents),
                &anonymous(splits),
            );
            let mut per_person = vec![0; everyone];
            for ((person, ..), amount) in splits.iter().zip(amounts) {
                per_person[*person] = amount;
            }
            per_person
        };
        let mut in_file = vec![0; everyone];
        for (person, amount) in people.iter().zip(owes) {
            in_file[*person] = *amount;
        }
        match declared {
            Some(declared) if people.is_empty() => {
                check_amounts(amount_cents, original, &anonymous(&declared))
                    .map_err(|e| self.at(e))?;
                Ok(declared)
            }
            // Another app rounds its cents elsewhere; more than that, and someone
            // changed the amounts.
            Some(declared)
                if check_amounts(amount_cents, original, &anonymous(&declared)).is_ok()
                    && gives(&declared)
                        .iter()
                        .zip(&in_file)
                        .all(|(a, b)| (a - b).abs() <= 1) =>
            {
                Ok(declared)
            }
            _ if people.is_empty() => Err(self.at("nobody shares this expense".to_string())),
            _ => split_from_amounts(amount_cents, original.is_some(), people, owes)
                .ok_or_else(|| self.at("the parts are too uneven to import".to_string())),
        }
    }
}
