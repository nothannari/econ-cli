//! Parsing for the Bank of England IADB CSV endpoint (Bank Rate etc.).
//!
//! Endpoint pattern (verified 2026-07-09, no auth):
//!   https://www.bankofengland.co.uk/boeapps/iadb/fromshowcolumns.asp?
//!     csv.x=yes&Datefrom={dd/Mon/yyyy}&Dateto=now&SeriesCodes={CODE}
//!     &CSVF=TN&UsingCodes=Y&VPD=Y&VFD=N
//! e.g. Bank Rate: SeriesCodes=IUDBEDR
//!
//! Gotchas for the reqwest call:
//! - the server 302-redirects to _iadb-FromShowColumns.asp; reqwest follows
//!   redirects by default, so this is fine, but don't disable it
//! - a browser-ish User-Agent is required; the default/no UA gets rejected
//!
//! CSV shape: header "DATE,{SERIES_CODE}", then one row per day,
//! dates formatted "02 Jan 2024".

use chrono::NaiveDate;

/// One row of the two-column CSV. The value column's header is the series
/// code, so rows are deserialized by position, not by name.
#[derive(Debug)]
pub struct BoeRow {
    pub date: String,
    pub value: f64,
}

impl BoeRow {
    pub fn date(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&self.date, "%d %b %Y").ok()
    }
}

/// Parses the full CSV body into rows.
pub fn parse(csv_text: &str) -> Result<Vec<BoeRow>, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv_text.as_bytes());
    reader
        .deserialize::<(String, f64)>()
        .map(|row| row.map(|(date, value)| BoeRow { date, value }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn parses_bank_rate_csv() {
        let text = include_str!("../samples/boe_iudbedr_rate.csv");
        let rows = parse(text).unwrap();
        assert!(!rows.is_empty());
        let first = &rows[0];
        let date = first.date().unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(first.value, 5.25);
    }
}
