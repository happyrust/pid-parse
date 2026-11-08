//! The ledger baseline and its gate: what a page's account is pinned to, and
//! how this run's account is compared with it.
//!
//! A baseline is one JSON file per corpus under `docs/baselines/`. Two exist:
//! `export-ledger-backup.json` (the 53-sheet plant backup) and
//! `export-ledger-fixtures.json` (the seven committed fixtures). The gate
//! compares this run's ledger page by page:
//!
//! - a **suspect** count that rose fails, naming the page, the account, the
//!   reason, both values, and where the extra drops came from;
//! - a **suspect** count that fell also fails, asking for the baseline to be
//!   lowered — the same convention as the `missing_docs` ratchet;
//! - an **intentional** count, an on-page count, a partial count or a
//!   cross-count that moved fails and is listed as an account change;
//! - a non-zero `unattributed` remainder fails;
//! - a page that failed to lay out or write fails.
//!
//! `UPDATE_EXPORT_LEDGER=1` rewrites the baseline, but only when no suspect
//! count would rise: a rewrite must never absorb a regression.
//!
//! # What is not compared
//!
//! The segment totals and written counts are stored for a reader and skipped by
//! the comparison: how many segments an arc flattens to comes from `kurbo`'s
//! floating-point arithmetic, which can differ in the last place between
//! platforms (design deviation I4). The two font-dependent partial marks are
//! skipped when only the typefaces differ (design deviation D1).
//!
//! # Not implemented yet
//!
//! This module is declared here so the module tree is complete; the gate
//! itself lands with milestone 6's tasks 9 and 10. Until then
//! `run_gate` does not exist and nothing calls it.

// The gate's implementation is milestone 6 (D2 tasks 9–13). This file exists
// now so `ledger.rs` declares the whole module tree, and so a later worker
// finds the design's own summary of the rules above where the code will go.
