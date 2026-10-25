//! Decode `CatalogIndex.xml` out of one or more `RefData~*.zip`
//! archives and print a `catalog-index-crosscheck/1` JSON report —
//! per-zip parse tallies plus the three-set cross-check against
//! `PUBLISH_ITEM_CATALOG`.
//!
//! Usage:
//!   cargo run --example probe_catalog_index \
//!     -- path/to/RefData~4~681.zip [more.zip ...]

use std::process::ExitCode;

use pid_parse::publish::catalog_index::{read_catalog_index_from_zip, CatalogIndexReading};
use pid_parse::publish::cross_check_catalog_index;
use serde_json::json;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: probe_catalog_index <RefData~*.zip> [<zip> ...]");
        return ExitCode::from(2);
    }
    let mut copies = Vec::new();
    for arg in &args {
        match read_catalog_index_from_zip(std::path::Path::new(arg)) {
            Ok(reading) => copies.push(copy_json(arg, &reading)),
            Err(error) => {
                eprintln!("error: {arg}: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let report = json!({
        "schema": "catalog-index-crosscheck/1",
        "copies": copies,
    });
    match serde_json::to_string_pretty(&report) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("error: json: {error}");
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

fn copy_json(zip: &str, reading: &CatalogIndexReading) -> serde_json::Value {
    let document = &reading.document;
    let check = cross_check_catalog_index(document);
    let items_by_item_type: serde_json::Map<String, serde_json::Value> = document
        .items_by_item_type()
        .iter()
        .map(|(name, count)| ((*name).to_owned(), json!(count)))
        .collect();
    let first_item = document.first_item().map(|item| {
        json!({
            "item_type": item.item_type,
            "class": item.class,
            "subclass": item.subclass,
            "type": item.type_code,
            "catalog_item": item.catalog_item,
        })
    });
    json!({
        "zip": zip,
        "entry": reading.entry_name,
        "uncompressed_bytes": reading.uncompressed_bytes,
        "encoding": document.encoding().as_str(),
        "blocks": document.blocks().len(),
        "block_item_types": document.item_types(),
        "item_bearing_blocks": document.item_bearing_item_types().len(),
        "item_bearing_item_types": document.item_bearing_item_types(),
        "empty_blocks": document.empty_blocks(),
        "items": document.items().len(),
        "items_by_item_type": items_by_item_type,
        "sym_references": document.sym_reference_count(),
        "discriminant_keys": document.discriminant_key_count(),
        "discriminant_keys_with_multiple_items": document.discriminant_keys_with_multiple_items(),
        "first_item": first_item,
        "cross_check": {
            "in_both": check.in_both,
            "only_in_catalog_index": check.only_in_catalog_index,
            "only_in_catalog": check.only_in_catalog,
            "matched_through_an_alias": check.matched_through_an_alias,
            "catalog_item_types": check.catalog_item_types,
            "catalog_aliases": check.catalog_aliases,
            "empty_difference_sets": check.empty_difference_sets,
        },
    })
}
