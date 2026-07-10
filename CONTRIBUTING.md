# Contributing

Thanks for your interest. This is a small tool and contributions are welcome.

## Building and testing

```
cargo build
cargo test
```

Tests deserialize real captured API responses from `samples/`, so they run
offline. If you touch the fetch layer, please also run the affected
subcommand against the live API before opening a PR.

## What's welcome

- Bug fixes, always.
- New series. Anything on DBnomics is about ten lines: a `SeriesKind`
  variant, a URL constant, and a match arm in `fetch_series` (see how
  `unemployment` was added). New series get `--since`, `--format`, and
  `--compare-to` for free.
- New output formats (`--format json` would be a natural one).
- Quarterly alignment for `--compare-to gdp`, which is currently refused.

## Conventions

- Keep PRs small and single-purpose.
- Commit messages: short subject, a sentence or two of body at most,
  and mention any non-obvious trap you hit.
- No new dependencies without discussion first; the current set is
  deliberately minimal.
- The formatting and table output are hand-rolled on purpose. Please
  don't introduce a table crate.

For anything bigger than a bug fix, opening an issue first saves both of
us time.
