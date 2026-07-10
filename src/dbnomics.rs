//! Serde structs for the DBnomics v22 series API.
//!
//! Endpoint pattern (verified 2026-07-09, no auth):
//!   https://api.db.nomics.world/v22/series/{PROVIDER}/{DATASET}/{SERIES}?observations=1
//! e.g. wages (AWE total pay): ONS/LMS/KAB9.M
//!
//! Shape notes:
//! - observations live in parallel arrays `period` / `period_start_day` / `value`
//! - `period` is frequency-formatted ("2026-05", "2026-Q1"); `period_start_day`
//!   is always an ISO date, so use that one with chrono
//! - `value` entries are EITHER a JSON number or the literal string "NA"
//!   for missing observations, hence the untagged enum

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DbnomicsResponse {
    pub series: SeriesPage,
}

#[derive(Debug, Deserialize)]
pub struct SeriesPage {
    pub docs: Vec<SeriesDoc>,
}

/// Only the fields the app reads; the response also carries @frequency,
/// provider/dataset/series codes, and period_start_day (ISO dates, useful
/// if period-string parsing ever stops being enough).
#[derive(Debug, Deserialize)]
pub struct SeriesDoc {
    pub series_name: String,
    /// "2026-05" (monthly), "2026-Q1" (quarterly), "2026" (annual)
    pub period: Vec<String>,
    pub value: Vec<Value>,
}

/// A single observation value: a number, or "NA" when missing.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Number(f64),
    // the string ("NA") is needed as a deserialization target but never read
    Text(#[allow(dead_code)] String),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Text(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_wages_series() {
        let json = include_str!("../samples/dbnomics_kab9_wages.json");
        let resp: DbnomicsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.series.docs.len(), 1);
        let doc = &resp.series.docs[0];
        assert!(doc.series_name.starts_with("AWE"));
        assert_eq!(doc.period.len(), doc.value.len());
        // early observations are "NA", recent ones are numbers
        assert!(doc.value[0].as_f64().is_none());
        assert!(doc.value.last().unwrap().as_f64().is_some());
    }
}
