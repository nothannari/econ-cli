# econ-cli

A small Rust CLI that fetches, caches, and prints UK macroeconomic data.

```
$ econ-cli inflation --since 2026
CPIH ANNUAL RATE 00: ALL ITEMS 2015=100 (%)

PERIOD    VALUE
2026 JAN  3.2
2026 FEB  3.2
2026 MAR  3.4
2026 APR  3.0
2026 MAY  3.0
```

## Usage

```
econ-cli <COMMAND> [--since <YEAR>] [--format <table|csv>]
```

| Command     | Series                                              | Frequency |
|-------------|-----------------------------------------------------|-----------|
| `gdp`       | GDP, chained volume measures, seasonally adj. (£m)  | quarterly |
| `inflation` | CPIH annual rate, all items                         | monthly   |
| `wages`     | Average weekly earnings, whole economy total pay (£)| monthly   |
| `rate`      | Bank of England Bank Rate (%)                       | daily     |
| `unemployment` | Unemployment rate, aged 16+, seasonally adj. (%) | monthly   |

- `--since 2020` — only observations from 2020 onwards
- `--format csv` — machine-readable output instead of the default table
- `--compare-to <SERIES>` — show another series alongside, aligned on calendar
  months (daily data collapses to its month-end value). `gdp` is quarterly and
  can't take part yet.

```
$ econ-cli wages --compare-to inflation --since 2026
wages vs inflation

PERIOD   WAGES  INFLATION
2026-01  743    3.2
2026-02  747    3.2
2026-03  754    3.4
2026-04  753    3.0
```

```
$ econ-cli gdp --since 2025 --format csv
period,value
2025 Q1,703178
...
```

Tip: `rate` is daily data going back to 1975, so you probably want `--since`.

## Data sources

No API keys required for any of them.

| Source | Used for | Endpoint |
|--------|----------|----------|
| [ONS website](https://www.ons.gov.uk) | `gdp` (ABMI), `inflation` (L55O) | `www.ons.gov.uk/{path}/timeseries/{cdid}/{dataset}/data` |
| [DBnomics](https://db.nomics.world) | `wages` (KAB9), `unemployment` (MGSX) | `api.db.nomics.world/v22/series/ONS/LMS/{CDID}.M` |
| [Bank of England IADB](https://www.bankofengland.co.uk/boeapps/database) | `rate` (IUDBEDR) | CSV export endpoint |

> **Note:** the old ONS API (`api.ons.gov.uk`) was retired in November 2024.
> This tool uses the ONS website's JSON endpoints instead; the taxonomy path
> for any other CDID can be looked up via
> `api.beta.ons.gov.uk/v1/search?content_type=timeseries&cdids={CDID}`.

## Caching

Responses are cached in `~/.cache/econ-cli/` and re-fetched once they are
older than 24 hours. Delete the directory to force a refresh.

## Building

```
cargo build --release
cargo test          # deserialization tests run against real captured responses in samples/
```
