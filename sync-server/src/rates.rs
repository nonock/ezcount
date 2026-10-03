//! Exchange rates, for the app's suggestion when an expense is paid in another currency.
//!
//! The relay asks a public rate service so the apps don't have to: the web version may only
//! talk to this origin, and the service sees the relay instead of every user. Answers are
//! remembered for a while, since everyone asks for the same few currencies and days.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::limits::{Client, HOUR};
use crate::{ApiError, Relay};

/// Frankfurter: daily rates from central banks, free and without a key. It answers
/// `GET <this>/USD/EUR?date=2026-08-29` with `{"date": …, "rate": 0.85856}`.
pub const DEFAULT_RATES_URL: &str = "https://api.frankfurter.dev/v2/rate";

/// A past day's rate doesn't change, and today's is published once: either can wait this long.
const KEEP: Duration = Duration::from_secs(6 * 60 * 60);
const TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Rates {
    /// Where to ask; none when the relay doesn't suggest rates.
    url: Option<String>,
    http: reqwest::Client,
    /// By "USD/EUR/2026-08-29".
    known: Mutex<HashMap<String, (Rate, Instant)>>,
}

#[derive(Clone, Serialize)]
pub(crate) struct Rate {
    /// Units of the second currency for one of the first, as a decimal number: "0.85856".
    rate: String,
    /// The day it is for, which is the last one with a rate when the asked one has none.
    date: String,
}

#[derive(Deserialize)]
pub(crate) struct RateQuery {
    /// `YYYY-MM-DD`; the latest rate without it.
    date: Option<String>,
}

impl Rates {
    pub(crate) fn new(url: Option<String>) -> Self {
        Self {
            url: url.map(|url| url.trim_end_matches('/').to_string()),
            http: reqwest::Client::builder()
                .timeout(TIMEOUT)
                .build()
                .unwrap_or_default(),
            known: Mutex::new(HashMap::new()),
        }
    }

    fn known(&self) -> std::sync::MutexGuard<'_, HashMap<String, (Rate, Instant)>> {
        self.known.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// One answer of the rate service; `None` when it has no rate for that.
    async fn ask(&self, url: &str, date: Option<&str>) -> Result<Option<Rate>, ApiError> {
        let unavailable = |e: String| ApiError::Unavailable(format!("rate service: {e}"));
        let mut request = self.http.get(url);
        if let Some(date) = date {
            request = request.query(&[("date", date)]);
        }
        let response = request
            .send()
            .await
            .map_err(|e| unavailable(e.to_string()))?;
        match response.status().as_u16() {
            200 => {}
            // An unknown currency, or a day it has nothing for.
            404 | 422 => return Ok(None),
            status => return Err(unavailable(format!("answered {status}"))),
        }
        let body = response
            .text()
            .await
            .map_err(|e| unavailable(e.to_string()))?;
        let answer: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| unavailable(e.to_string()))?;
        // The number as written, not through a float.
        let rate = answer["rate"].to_string();
        let date = answer["date"].as_str().unwrap_or_default().to_string();
        let plain = !rate.is_empty()
            && rate.len() <= 20
            && rate.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            && rate.bytes().any(|b| (b'1'..=b'9').contains(&b));
        if !plain || !is_date(&date) {
            return Err(unavailable(format!("unexpected answer {body:.200}")));
        }
        Ok(Some(Rate { rate, date }))
    }
}

fn currency(code: &str) -> Result<String, ApiError> {
    if code.len() == 3 && code.bytes().all(|b| b.is_ascii_alphabetic()) {
        Ok(code.to_ascii_uppercase())
    } else {
        Err(ApiError::BadRequest("currencies are three-letter codes"))
    }
}

fn is_date(date: &str) -> bool {
    date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// `GET /v1/rates/{from}/{to}`
pub(crate) async fn rate(
    State(relay): State<Arc<Relay>>,
    Client(client): Client,
    Path((from, to)): Path<(String, String)>,
    Query(query): Query<RateQuery>,
) -> Result<Json<Rate>, ApiError> {
    let (from, to) = (currency(&from)?, currency(&to)?);
    if from == to {
        return Err(ApiError::BadRequest("the two currencies are the same"));
    }
    if query.date.as_deref().is_some_and(|date| !is_date(date)) {
        return Err(ApiError::BadRequest("the date must be YYYY-MM-DD"));
    }
    let rates = &relay.rates;
    let Some(url) = &rates.url else {
        return Err(ApiError::NoRate);
    };

    let key = format!("{from}/{to}/{}", query.date.as_deref().unwrap_or("latest"));
    if let Some((rate, since)) = rates.known().get(&key) {
        if since.elapsed() < KEEP {
            return Ok(Json(rate.clone()));
        }
    }
    // Only what reaches the rate service counts: per client network, and for everyone
    // together, so that many addresses can't make the relay flood the service either.
    let limits = &relay.limits;
    let counters = &relay.counters;
    if !counters.try_add(
        &format!("rates:{client}"),
        1,
        limits.rate_lookups_per_hour,
        HOUR,
    ) || !counters.try_add("rates:all", 1, limits.rate_lookups_per_hour_total, HOUR)
    {
        return Err(ApiError::TooManyRequests);
    }

    let url = format!("{url}/{from}/{to}");
    let mut found = rates.ask(&url, query.date.as_deref()).await?;
    // Today may not have started where the rates are made, or have no rate yet.
    if found.is_none() && query.date.is_some() {
        found = rates.ask(&url, None).await?;
    }
    let rate = found.ok_or(ApiError::NoRate)?;

    let mut known = rates.known();
    if known.len() > 10_000 {
        known.retain(|_, (_, since)| since.elapsed() < KEEP);
    }
    known.insert(key, (rate.clone(), Instant::now()));
    Ok(Json(rate))
}
