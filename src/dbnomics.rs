//! Serde structs for the DBnomics v22 series API.
//!
//! Endpoint pattern (verified 2026-07-09, no auth):
//!   https://api.db.nomics.world/v22/series/{PROVIDER}/{DATASET}/{SERIES}?observations=1
//! e.g. wages (AWE total pay): ONS/LMS/KAB9.M
//!
//! Shape notes:
//! - observations live in parallel arrays `period` / `period_start_day` / `value`
//! - `period` is frequency-formatted ("2026-05", "2026-Q1"); `period_start_day`
//!   is always an ISO date — use that one with chrono
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
    pub num_found: u64,
}

#[derive(Debug, Deserialize)]
pub struct SeriesDoc {
    #[serde(rename = "@frequency")]
    pub frequency: String,
    pub provider_code: String,
    pub dataset_code: String,
    pub series_code: String,
    pub series_name: String,
    /// "2026-05" (monthly), "2026-Q1" (quarterly), "2026" (annual)
    pub period: Vec<String>,
    /// ISO dates, one per period — parse these with chrono
    pub period_start_day: Vec<String>,
    pub value: Vec<Value>,
}

/// A single observation value: a number, or "NA" when missing.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Number(f64),
    Text(String),
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
        assert_eq!(resp.series.num_found, 1);
        let doc = &resp.series.docs[0];
        assert_eq!(doc.series_code, "KAB9.M");
        assert_eq!(doc.period.len(), doc.value.len());
        assert_eq!(doc.period.len(), doc.period_start_day.len());
        // early observations are "NA", recent ones are numbers
        assert!(doc.value[0].as_f64().is_none());
        assert!(doc.value.last().unwrap().as_f64().is_some());
    }
}
