//! How much of a SmartPlant `_Data.xml` does one of ours state?
//!
//! `pid_publish_xml --diff-against` settles whether anything a run wrote is
//! wrong and whether anything it left out is unexcused. A run passes that
//! while leaving out what a `.pid` or a dump cannot state, class by class, so
//! it does not say how much of the reference the run states. This probe
//! does: it reads one generated `_Data.xml` and its reference and prints, for
//! the container's attributes, the objects, their interface elements, their
//! non-empty values, their empty attributes and the relations, how many the
//! reference carries and how many of those ours states — then what is missing
//! most, by class, by `(class, interface)` and by
//! `(class, interface, attribute)`.
//!
//! Every denominator is the reference's own count; nothing is excused. The
//! line `reference attributes` checks that: the attributes the shares account
//! for against the attributes counted off the reference's text.
//!
//! The counting is `pid_parse::publish::completeness::measure`, the same
//! function `tests/publish_completeness.rs` pins per drawing and mode.
//!
//! ```powershell
//! cargo run --example probe_how_much_of_a_reference_does_a_publish_state -- `
//!     <ours_Data.xml> <reference_Data.xml> [--top N]
//! ```
//!
//! `--top N` sets how many rows each breakdown prints (20 by default, `0` for
//! all of them). Exits 2 on a usage error and 1 when a file does not read or
//! is not well-formed XML; a measurement, whatever it says, exits 0.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::path::Path;
use std::process::ExitCode;

use pid_parse::publish::completeness::{measure, Completeness, Share};

const USAGE: &str = "usage: probe_how_much_of_a_reference_does_a_publish_state \
                     <ours_Data.xml> <reference_Data.xml> [--top N]";

fn main() -> ExitCode {
    let mut paths = Vec::new();
    let mut top = 20usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--top" {
            let Some(value) = args.next().and_then(|value| value.parse().ok()) else {
                eprintln!("--top takes a row count\n{USAGE}");
                return ExitCode::from(2);
            };
            top = value;
        } else {
            paths.push(arg);
        }
    }
    let [ours, reference] = paths.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let (Some(ours_xml), Some(reference_xml)) = (read(ours), read(reference)) else {
        return ExitCode::FAILURE;
    };
    let measured = match measure(&ours_xml, &reference_xml) {
        Ok(measured) => measured,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    println!("ours       {ours}");
    println!("reference  {reference}");
    println!();
    println!("{measured}");
    let in_file = attributes_in(&reference_xml);
    let in_shares = measured.reference_attributes();
    println!(
        "reference attributes: {in_file} in the file, {in_shares} in the shares{}",
        if in_file == in_shares {
            ""
        } else {
            " — the shares do not cover the whole reference"
        }
    );
    for difference in &measured.container_differences {
        println!(
            "container {}: ours {:?}, reference {:?}",
            difference.attribute, difference.generated, difference.reference
        );
    }

    print_breakdown(&measured, top);
    ExitCode::SUCCESS
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(Path::new(path))
        .inspect_err(|error| eprintln!("read {path}: {error}"))
        .ok()
}

/// The attributes a `_Data.xml` carries, counted off its text: every `="`
/// that opens a value, less the XML declaration's own. Independent of the
/// reader `measure` uses.
fn attributes_in(xml: &str) -> usize {
    let declaration = xml.find("?>").map_or(0, |end| end + 2);
    xml[declaration..].matches("=\"").count()
}

fn print_breakdown(measured: &Completeness, top: usize) {
    print_missing(
        "objects by class",
        measured
            .objects_by_class
            .iter()
            .map(|(key, share)| (key, *share)),
        top,
    );
    print_missing(
        "interface elements by (class, interface)",
        measured
            .interfaces_by_class
            .iter()
            .map(|((class, interface), share)| (format!("{class}.{interface}"), *share)),
        top,
    );
    print_missing(
        "non-empty values by (class, interface, attribute)",
        measured
            .values_by_attribute
            .iter()
            .map(|(key, share)| (dotted(key), *share)),
        top,
    );
    print_missing(
        "empty attributes by (class, interface, attribute)",
        measured
            .empty_attributes_by_attribute
            .iter()
            .map(|(key, share)| (dotted(key), *share)),
        top,
    );
    print_missing(
        "relations by DefUID",
        measured
            .relations_by_def_uid
            .iter()
            .map(|(key, share)| (key, *share)),
        top,
    );
    print_counts(
        "values stated differently, by (class, interface, attribute)",
        measured
            .values_different_by_attribute
            .iter()
            .map(|(key, count)| (dotted(key), *count)),
        top,
    );
    print_counts(
        "interface elements not in the reference, by (class, interface)",
        measured
            .interfaces_not_in_reference_by_class
            .iter()
            .map(|((class, interface), count)| (format!("{class}.{interface}"), *count)),
        top,
    );

    // The same missing values with the class folded away: what an attribute
    // costs across every class that carries it.
    let mut by_attribute: BTreeMap<String, Share> = BTreeMap::new();
    for ((_, interface, attribute), share) in &measured.values_by_attribute {
        let total = by_attribute
            .entry(format!("{interface}.{attribute}"))
            .or_default();
        total.stated += share.stated;
        total.reference += share.reference;
    }
    print_missing(
        "non-empty values by (interface, attribute), every class together",
        by_attribute.into_iter(),
        top,
    );
}

fn dotted((class, interface, attribute): &(String, String, String)) -> String {
    format!("{class}.{interface}.{attribute}")
}

/// The rows of one breakdown that miss something, most missing first, as
/// `missing / reference`.
fn print_missing<K: Display>(title: &str, rows: impl Iterator<Item = (K, Share)>, top: usize) {
    let mut rows: Vec<(String, Share)> = rows
        .filter(|(_, share)| share.missing() > 0)
        .map(|(key, share)| (key.to_string(), share))
        .collect();
    rows.sort_by(|a, b| b.1.missing().cmp(&a.1.missing()).then(a.0.cmp(&b.0)));
    let missing: usize = rows.iter().map(|(_, share)| share.missing()).sum();
    println!();
    println!(
        "missing {title}: {missing} in {} rows (missing / reference)",
        rows.len()
    );
    let shown = if top == 0 {
        rows.len()
    } else {
        top.min(rows.len())
    };
    for (key, share) in &rows[..shown] {
        println!("  {:>6} / {:<6} {key}", share.missing(), share.reference);
    }
    if shown < rows.len() {
        println!("  … {} more rows", rows.len() - shown);
    }
}

fn print_counts<K: Display>(title: &str, rows: impl Iterator<Item = (K, usize)>, top: usize) {
    let mut rows: Vec<(String, usize)> =
        rows.map(|(key, count)| (key.to_string(), count)).collect();
    if rows.is_empty() {
        return;
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total: usize = rows.iter().map(|(_, count)| count).sum();
    println!();
    println!("{title}: {total} in {} rows", rows.len());
    let shown = if top == 0 {
        rows.len()
    } else {
        top.min(rows.len())
    };
    for (key, count) in &rows[..shown] {
        println!("  {count:>6}  {key}");
    }
    if shown < rows.len() {
        println!("  … {} more rows", rows.len() - shown);
    }
}
