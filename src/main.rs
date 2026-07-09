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
}

#[derive(Args)]
struct SeriesArgs {
    /// Only show observations from this year onwards
    #[arg(long, value_name = "YEAR")]
    since: Option<i32>,

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Table)]
    format: Format,
}

#[derive(Copy, Clone, ValueEnum)]
enum Format {
    Table,
    Csv,
}

/// One observation, normalized across the three sources.
struct Row {
    period: String,
    year: Option<i32>,
    value: String,
}

const GDP_URL: &str =
    "https://www.ons.gov.uk/economy/grossdomesticproductgdp/timeseries/abmi/ukea/data";
const CPIH_URL: &str =
    "https://www.ons.gov.uk/economy/inflationandpriceindices/timeseries/l55o/mm23/data";
const WAGES_URL: &str =
    "https://api.db.nomics.world/v22/series/ONS/LMS/KAB9.M?observations=1";
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
    let (args, title, mut rows) = match &cli.command {
        Command::Gdp(args) => {
            let (title, rows) = fetch_ons("abmi", GDP_URL)?;
            (args, title, rows)
        }
        Command::Inflation(args) => {
            let (title, rows) = fetch_ons("l55o", CPIH_URL)?;
            (args, title, rows)
        }
        Command::Wages(args) => {
            let (title, rows) = fetch_wages()?;
            (args, title, rows)
        }
        Command::Rate(args) => {
            let (title, rows) = fetch_rate()?;
            (args, title, rows)
        }
    };

    if let Some(since) = args.since {
        rows.retain(|r| r.year.is_some_and(|y| y >= since));
    }

    match args.format {
        Format::Table => print_table(&title, &rows),
        Format::Csv => write_csv(&rows)?,
    }
    Ok(())
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
            value: o.value.clone(),
        })
        .collect();
    Ok((title, rows))
}

fn fetch_wages() -> Result<(String, Vec<Row>), Box<dyn Error>> {
    let body = fetch_cached("kab9", "json", WAGES_URL)?;
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
            let year = period.get(..4).and_then(|y| y.parse().ok());
            Some(Row {
                period: period.clone(),
                year,
                value: value.to_string(),
            })
        })
        .collect();
    Ok((doc.series_name.clone(), rows))
}

fn fetch_rate() -> Result<(String, Vec<Row>), Box<dyn Error>> {
    let body = fetch_cached("iudbedr", "csv", RATE_URL)?;
    let rows = boe::parse(&body)?
        .into_iter()
        .map(|r| Row {
            year: r.date().map(|d| d.year()),
            period: r.date,
            value: r.value.to_string(),
        })
        .collect();
    Ok(("Bank of England Bank Rate (%)".to_string(), rows))
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
