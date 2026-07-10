mod boe;
mod cache;
mod dbnomics;
mod ons;

use std::error::Error;
use std::process::ExitCode;

use chrono::Datelike;
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "econ-cli", version, about = "UK macroeconomic data from the ONS, DBnomics and the Bank of England")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// GDP, chained volume measures, seasonally adjusted £m (quarterly)
    Gdp(SeriesArgs),
    /// CPIH annual inflation rate, all items (monthly)
    Inflation(SeriesArgs),
    /// Bank of England Bank Rate (daily)
    Rate(SeriesArgs),
    /// Average weekly earnings, whole economy total pay £ (monthly)
    Wages(SeriesArgs),
    /// Unemployment rate, aged 16 and over, seasonally adjusted % (monthly)
    Unemployment(SeriesArgs),
    /// Economic inactivity rate, aged 16-64, seasonally adjusted % (monthly)
    Inactivity(SeriesArgs),
}

#[derive(Args)]
struct SeriesArgs {
    /// Only show observations from this year onwards
    #[arg(long, value_name = "YEAR")]
    since: Option<i32>,

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Table)]
    format: Format,

    /// Show another series alongside, aligned on months
    /// (daily data is collapsed to its month-end value)
    #[arg(long, value_enum, value_name = "SERIES")]
    compare_to: Option<SeriesKind>,
}

#[derive(Copy, Clone, ValueEnum)]
enum Format {
    Table,
    Csv,
}

#[derive(Copy, Clone, PartialEq, ValueEnum)]
enum SeriesKind {
    Gdp,
    Inflation,
    Rate,
    Wages,
    Unemployment,
    Inactivity,
}

impl SeriesKind {
    fn name(self) -> &'static str {
        match self {
            SeriesKind::Gdp => "gdp",
            SeriesKind::Inflation => "inflation",
            SeriesKind::Rate => "rate",
            SeriesKind::Wages => "wages",
            SeriesKind::Unemployment => "unemployment",
            SeriesKind::Inactivity => "inactivity",
        }
    }
}

/// One observation, normalized across the three sources.
struct Row {
    period: String,
    year: Option<i32>,
    /// 1-12 when the observation can be aligned to a calendar month
    month: Option<u32>,
    value: String,
    /// Set for numeric sources (DBnomics, BoE) so displayed rows can be
    /// re-formatted to uniform precision after filtering; None for ONS,
    /// whose values arrive as already-formatted strings.
    num: Option<f64>,
}

const GDP_URL: &str =
    "https://www.ons.gov.uk/economy/grossdomesticproductgdp/timeseries/abmi/ukea/data";
const CPIH_URL: &str =
    "https://www.ons.gov.uk/economy/inflationandpriceindices/timeseries/l55o/mm23/data";
const WAGES_URL: &str =
    "https://api.db.nomics.world/v22/series/ONS/LMS/KAB9.M?observations=1";
// MGSX: the series the original project brief mislabeled as GDP
const UNEMPLOYMENT_URL: &str =
    "https://api.db.nomics.world/v22/series/ONS/LMS/MGSX.M?observations=1";
const INACTIVITY_URL: &str =
    "https://api.db.nomics.world/v22/series/ONS/LMS/LF2S.M?observations=1";
const RATE_URL: &str = "https://www.bankofengland.co.uk/boeapps/iadb/fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1975&Dateto=now&SeriesCodes=IUDBEDR&CSVF=TN&UsingCodes=Y&VPD=Y&VFD=N";

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let (args, kind) = match &cli.command {
        Command::Gdp(args) => (args, SeriesKind::Gdp),
        Command::Inflation(args) => (args, SeriesKind::Inflation),
        Command::Rate(args) => (args, SeriesKind::Rate),
        Command::Wages(args) => (args, SeriesKind::Wages),
        Command::Unemployment(args) => (args, SeriesKind::Unemployment),
        Command::Inactivity(args) => (args, SeriesKind::Inactivity),
    };

    if let Some(target) = args.compare_to {
        return run_compare(args, kind, target);
    }

    let (title, mut rows) = fetch_series(kind)?;
    if let Some(since) = args.since {
        rows.retain(|r| r.year.is_some_and(|y| y >= since));
    }
    finalize_values(&mut rows);

    match args.format {
        Format::Table => print_table(&title, &rows),
        Format::Csv => write_csv(&rows)?,
    }
    Ok(())
}

fn run_compare(args: &SeriesArgs, base: SeriesKind, target: SeriesKind) -> Result<(), Box<dyn Error>> {
    if base == target {
        return Err("cannot compare a series with itself".into());
    }
    if base == SeriesKind::Gdp || target == SeriesKind::Gdp {
        return Err("gdp is quarterly; monthly comparison is not supported yet".into());
    }

    let (_, mut base_rows) = fetch_series(base)?;
    let (_, mut target_rows) = fetch_series(target)?;
    // filter before aligning so uniform precision reflects displayed rows only
    if let Some(since) = args.since {
        base_rows.retain(|r| r.year.is_some_and(|y| y >= since));
        target_rows.retain(|r| r.year.is_some_and(|y| y >= since));
    }
    finalize_values(&mut base_rows);
    finalize_values(&mut target_rows);
    let rows = align_monthly(&base_rows, &target_rows);

    match args.format {
        Format::Table => print_compare_table(base.name(), target.name(), &rows),
        Format::Csv => write_compare_csv(base.name(), target.name(), &rows)?,
    }
    Ok(())
}

fn fetch_series(kind: SeriesKind) -> Result<(String, Vec<Row>), Box<dyn Error>> {
    match kind {
        SeriesKind::Gdp => fetch_ons("abmi", GDP_URL),
        SeriesKind::Inflation => fetch_ons("l55o", CPIH_URL),
        SeriesKind::Wages => fetch_dbnomics("kab9", WAGES_URL),
        SeriesKind::Unemployment => fetch_dbnomics("mgsx", UNEMPLOYMENT_URL),
        SeriesKind::Inactivity => fetch_dbnomics("lf2s", INACTIVITY_URL),
        SeriesKind::Rate => fetch_rate(),
    }
}

/// Returns the response body for `url`, from the local cache when fresh.
fn fetch_cached(cache_key: &str, ext: &str, url: &str) -> Result<String, Box<dyn Error>> {
    if let Some(cached) = cache::read_fresh(cache_key, ext) {
        return Ok(cached);
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; econ-cli/0.1)")
        .build()?;
    let body = client.get(url).send()?.error_for_status()?.text()?;
    cache::write(cache_key, ext, &body)?;
    Ok(body)
}

fn fetch_ons(cache_key: &str, url: &str) -> Result<(String, Vec<Row>), Box<dyn Error>> {
    let body = fetch_cached(cache_key, "json", url)?;
    let series: ons::SeriesResponse = serde_json::from_str(&body)?;
    let desc = &series.description;
    let unit = format!("{}{}", desc.pre_unit, desc.unit);
    let title = if unit.is_empty() {
        desc.title.clone()
    } else {
        format!("{} ({unit})", desc.title)
    };
    let rows = series
        .observations()
        .iter()
        .map(|o| Row {
            period: o.date.clone(),
            year: o.year(),
            month: month_number(&o.month),
            value: o.value.clone(),
            num: None,
        })
        .collect();
    Ok((title, rows))
}

fn fetch_dbnomics(cache_key: &str, url: &str) -> Result<(String, Vec<Row>), Box<dyn Error>> {
    let body = fetch_cached(cache_key, "json", url)?;
    let resp: dbnomics::DbnomicsResponse = serde_json::from_str(&body)?;
    let doc = resp
        .series
        .docs
        .first()
        .ok_or("DBnomics returned no series")?;
    let rows = doc
        .period
        .iter()
        .zip(&doc.value)
        .filter_map(|(period, value)| {
            let value = value.as_f64()?; // skip "NA" observations
            Some(Row {
                period: period.clone(),
                year: period.get(..4).and_then(|y| y.parse().ok()),
                month: period.get(5..7).and_then(|m| m.parse().ok()),
                value: value.to_string(),
                num: Some(value),
            })
        })
        .collect();
    Ok((doc.series_name.clone(), rows))
}

fn fetch_rate() -> Result<(String, Vec<Row>), Box<dyn Error>> {
    let body = fetch_cached("iudbedr", "csv", RATE_URL)?;
    let rows = boe::parse(&body)?
        .into_iter()
        .map(|r| {
            let date = r.date();
            Row {
                year: date.map(|d| d.year()),
                month: date.map(|d| d.month()),
                period: r.date,
                value: r.value.to_string(),
                num: Some(r.value),
            }
        })
        .collect();
    Ok(("Bank of England Bank Rate (%)".to_string(), rows))
}

/// Rewrites numeric rows to uniform decimal precision: the most decimal
/// places any displayed value needs (4.9 and 5 -> "4.9"/"5.0", but
/// all-integer series stay integer). ONS string rows pass through as-is.
fn finalize_values(rows: &mut [Row]) {
    let decimals = max_decimals(rows.iter().filter_map(|r| r.num));
    for row in rows {
        if let Some(n) = row.num {
            row.value = format!("{n:.decimals$}");
        }
    }
}

fn max_decimals(values: impl Iterator<Item = f64>) -> usize {
    values
        .map(|v| {
            let s = v.to_string();
            s.find('.').map_or(0, |dot| s.len() - dot - 1)
        })
        .max()
        .unwrap_or(0)
        .min(4)
}

/// "January" / "May" -> 1 / 5. Empty or unrecognized -> None.
fn month_number(name: &str) -> Option<u32> {
    chrono::NaiveDate::parse_from_str(&format!("01 {name} 2000"), "%d %B %Y")
        .ok()
        .map(|d| d.month())
}

/// One month present in both series.
struct ComparedRow {
    year: i32,
    month: u32,
    base: String,
    target: String,
}

impl ComparedRow {
    fn period(&self) -> String {
        format!("{}-{:02}", self.year, self.month)
    }
}

/// Joins two series on calendar month, keeping months present in both.
/// Multiple observations in one month (daily data) collapse to the last one.
fn align_monthly(base: &[Row], target: &[Row]) -> Vec<ComparedRow> {
    use std::collections::{BTreeMap, HashMap};

    let mut target_by_month = HashMap::new();
    for r in target {
        if let (Some(y), Some(m)) = (r.year, r.month) {
            target_by_month.insert((y, m), r.value.clone());
        }
    }
    let mut base_by_month = BTreeMap::new();
    for r in base {
        if let (Some(y), Some(m)) = (r.year, r.month) {
            base_by_month.insert((y, m), r.value.clone());
        }
    }
    base_by_month
        .into_iter()
        .filter_map(|((year, month), base)| {
            let target = target_by_month.get(&(year, month))?.clone();
            Some(ComparedRow { year, month, base, target })
        })
        .collect()
}

fn print_compare_table(base_name: &str, target_name: &str, rows: &[ComparedRow]) {
    println!("{base_name} vs {target_name}");
    println!();
    let base_header = base_name.to_uppercase();
    let base_width = rows
        .iter()
        .map(|r| r.base.len())
        .max()
        .unwrap_or(0)
        .max(base_header.len());
    println!("{:<7}  {:<base_width$}  {}", "PERIOD", base_header, target_name.to_uppercase());
    for row in rows {
        println!("{:<7}  {:<base_width$}  {}", row.period(), row.base, row.target);
    }
}

fn write_compare_csv(
    base_name: &str,
    target_name: &str,
    rows: &[ComparedRow],
) -> Result<(), Box<dyn Error>> {
    let mut writer = csv::Writer::from_writer(std::io::stdout());
    writer.write_record(["period", base_name, target_name])?;
    for row in rows {
        writer.write_record([&row.period(), &row.base, &row.target])?;
    }
    writer.flush()?;
    Ok(())
}

fn print_table(title: &str, rows: &[Row]) {
    println!("{title}");
    println!();
    let width = rows
        .iter()
        .map(|r| r.period.len())
        .max()
        .unwrap_or(0)
        .max("PERIOD".len());
    println!("{:<width$}  VALUE", "PERIOD");
    for row in rows {
        println!("{:<width$}  {}", row.period, row.value);
    }
}

fn write_csv(rows: &[Row]) -> Result<(), Box<dyn Error>> {
    let mut writer = csv::Writer::from_writer(std::io::stdout());
    writer.write_record(["period", "value"])?;
    for row in rows {
        writer.write_record([&row.period, &row.value])?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(year: i32, month: Option<u32>, value: &str) -> Row {
        Row {
            period: String::new(),
            year: Some(year),
            month,
            value: value.to_string(),
            num: value.parse().ok(),
        }
    }

    #[test]
    fn aligns_on_shared_months_and_collapses_dailies() {
        let base = vec![
            row(2026, Some(1), "740"),
            row(2026, Some(2), "747"),
            row(2026, Some(3), "754"),
        ];
        // daily-style target: two observations in Feb -> last one wins; no March
        let target = vec![
            row(2026, Some(2), "4.0"),
            row(2026, Some(2), "3.75"),
            row(2026, Some(1), "4.0"),
            row(2026, None, "9.9"), // not monthly-alignable -> ignored
        ];
        let joined = align_monthly(&base, &target);
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].period(), "2026-01");
        assert_eq!(joined[1].period(), "2026-02");
        assert_eq!(joined[1].base, "747");
        assert_eq!(joined[1].target, "3.75");
    }

    #[test]
    fn formats_series_to_uniform_precision() {
        // mixed precision -> pad to the widest
        assert_eq!(max_decimals([4.9, 5.0, 4.75].into_iter()), 2);
        // all integers -> stay integer
        assert_eq!(max_decimals([743.0, 754.0].into_iter()), 0);
        // empty -> 0
        assert_eq!(max_decimals(std::iter::empty()), 0);

        let mut rows = vec![row(2026, Some(1), "4.9"), row(2026, Some(2), "5")];
        finalize_values(&mut rows);
        assert_eq!(rows[1].value, "5.0");
    }

    #[test]
    fn parses_ons_month_names() {
        assert_eq!(month_number("January"), Some(1));
        assert_eq!(month_number("May"), Some(5));
        assert_eq!(month_number(""), None);
    }
}
