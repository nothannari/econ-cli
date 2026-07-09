//! Serde structs matching the ONS website timeseries JSON.
//!
//! NOTE: the old `api.ons.gov.uk` v0 API was retired on 25 Nov 2024.
//! Working endpoint pattern (verified 2026-07-09):
//!   https://www.ons.gov.uk/{taxonomy_path}/timeseries/{cdid}/{dataset}/data
//! e.g.
//!   GDP  (ABMI): https://www.ons.gov.uk/economy/grossdomesticproductgdp/timeseries/abmi/ukea/data
//!   CPIH (L55O): https://www.ons.gov.uk/economy/inflationandpriceindices/timeseries/l55o/mm23/data
//! The taxonomy path for any CDID can be looked up via:
//!   https://api.beta.ons.gov.uk/v1/search?content_type=timeseries&cdids={CDID}
//! (items[0].uri, then append "/data" on www.ons.gov.uk).
//!
//! Every field in the response is a string, including numeric values.
//! Exactly one of `years` / `quarters` / `months` is populated per series
//! frequency; the others are present but empty.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SeriesResponse {
    pub description: Description,
    #[serde(default)]
    pub years: Vec<Observation>,
    #[serde(default)]
    pub quarters: Vec<Observation>,
    #[serde(default)]
    pub months: Vec<Observation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    pub title: String,
    pub cdid: String,
    pub dataset_id: String,
    /// e.g. "%" for CPIH annual rate; empty for GDP £m (see `pre_unit`)
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub pre_unit: String,
    #[serde(default)]
    pub release_date: String,
    #[serde(default)]
    pub next_release: String,
}

#[derive(Debug, Deserialize)]
pub struct Observation {
    /// "1948" (annual), "2026 Q1" (quarterly), "2026 MAY" (monthly)
    pub date: String,
    /// numeric value as a string, e.g. "709598" or "3.0"
    pub value: String,
    pub year: String,
    /// "May" for monthly series, "" otherwise
    #[serde(default)]
    pub month: String,
    /// "Q1" for quarterly series, "" otherwise
    #[serde(default)]
    pub quarter: String,
}

impl SeriesResponse {
    /// The populated observation list for this series' native frequency.
    pub fn observations(&self) -> &[Observation] {
        if !self.months.is_empty() {
            &self.months
        } else if !self.quarters.is_empty() {
            &self.quarters
        } else {
            &self.years
        }
    }
}

impl Observation {
    pub fn year(&self) -> Option<i32> {
        self.year.parse().ok()
    }

    pub fn value(&self) -> Option<f64> {
        self.value.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_quarterly_gdp() {
        let json = include_str!("../samples/abmi_gdp.json");
        let series: SeriesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(series.description.cdid, "ABMI");
        assert!(series.months.is_empty());
        assert!(!series.quarters.is_empty());
        let obs = &series.observations()[0];
        assert!(obs.year().is_some());
        assert!(obs.value().is_some());
    }

    #[test]
    fn deserializes_monthly_cpih() {
        let json = include_str!("../samples/l55o_cpih.json");
        let series: SeriesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(series.description.cdid, "L55O");
        assert_eq!(series.description.unit, "%");
        assert!(!series.months.is_empty());
        assert_eq!(series.observations()[0].quarter, "");
    }
}
