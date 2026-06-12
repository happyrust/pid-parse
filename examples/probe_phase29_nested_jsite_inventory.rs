//! Phase 29-D probe: inventory nested `JSite*` packages across local PID
//! fixtures.
//!
//! Read-only investigation output. It does not register nested streams or merge
//! nested Sheet geometry into top-level drawing geometry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pid_parse::PidParser;

const FIXTURES: &[(&str, &str)] = &[
    ("d06", "test-file/D06.pid"),
    ("nonascii-process-1", "test-file/工艺管道及仪表流程-1.pid"),
    ("dwg0201", "test-file/DWG-0201GP06-01.pid"),
    ("dwg0202", "test-file/DWG-0202GP06-01.pid"),
    (
        "publish-a01",
        "test-file/export-test/publish-data/A01/A01.pid",
    ),
    (
        "publish-dwg0202",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ),
];

#[derive(Debug, Default)]
struct NestedSite {
    fixture: &'static str,
    path: String,
    symbol_path: Option<String>,
    local_symbol_path: Option<String>,
    guid_count: usize,
    child_count: usize,
    total_child_bytes: u64,
    families: BTreeSet<String>,
    sheet_children: Vec<String>,
    child_streams: Vec<(String, u64)>,
}

fn child_family(child_path: &str) -> String {
    let parts: Vec<_> = child_path.trim_matches('/').split('/').collect();
    let Some(local) = parts.get(1) else {
        return "(root)".to_string();
    };
    if local.starts_with("Sheet") {
        "Sheet*".to_string()
    } else if local.starts_with("JSite") {
        "nested JSite*".to_string()
    } else if *local == "PSMspacemap" {
        "PSMspacemap".to_string()
    } else {
        (*local).to_string()
    }
}

fn classify(site: &NestedSite) -> &'static str {
    let has_psm = site.families.contains("PSMcluster0")
        || site.families.contains("PSMroots")
        || site.families.contains("PSMclustertable");
    let has_sheet = site.families.contains("Sheet*");
    let has_style = site.families.contains("StyleCluster");

    if has_psm && has_sheet {
        "NeedsOwnership"
    } else if has_psm || has_style {
        "CanTraceHeaderOnly"
    } else {
        "IgnoreUntilConsumerNeeds"
    }
}

fn collect_sites() -> Vec<NestedSite> {
    let parser = PidParser::new();
    let mut out = Vec::new();

    for (fixture_id, fixture_path) in FIXTURES {
        if !Path::new(fixture_path).exists() {
            eprintln!("skip: missing fixture {fixture_path}");
            continue;
        }
        let Ok(pkg) = parser.parse_package(fixture_path) else {
            eprintln!("skip: failed to parse fixture {fixture_path}");
            continue;
        };

        let jsite_by_path: BTreeMap<_, _> = pkg
            .parsed
            .jsites
            .iter()
            .map(|site| (site.path.clone(), site))
            .collect();
        let mut child_map: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();

        for (path, stream) in &pkg.streams {
            let mut parts = path.trim_matches('/').split('/');
            let Some(first) = parts.next() else {
                continue;
            };
            if !first.starts_with("JSite") || parts.next().is_none() {
                continue;
            }
            child_map
                .entry(format!("/{first}"))
                .or_default()
                .push((path.clone(), stream.data.len() as u64));
        }

        for (jsite_path, mut children) in child_map {
            children.sort_by(|a, b| a.0.cmp(&b.0));
            let site = jsite_by_path.get(&jsite_path).copied();
            let mut nested = NestedSite {
                fixture: fixture_id,
                path: jsite_path.clone(),
                symbol_path: site.and_then(|site| site.symbol_path.clone()),
                local_symbol_path: site.and_then(|site| site.local_symbol_path.clone()),
                guid_count: site
                    .map(|site| site.properties.guids.len())
                    .unwrap_or_default(),
                child_count: children.len(),
                total_child_bytes: children.iter().map(|(_, size)| *size).sum(),
                ..NestedSite::default()
            };
            for (child_path, size) in children {
                let family = child_family(&child_path);
                if family == "Sheet*" {
                    nested.sheet_children.push(child_path.clone());
                }
                nested.families.insert(family);
                nested.child_streams.push((child_path, size));
            }
            out.push(nested);
        }
    }

    out
}

fn joined_set(items: &BTreeSet<String>) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

fn joined_sheets(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items
            .iter()
            .map(|path| path.rsplit('/').next().unwrap_or(path))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn main() {
    let mut sites = collect_sites();
    sites.sort_by_key(|site| {
        (
            std::cmp::Reverse(site.child_count),
            std::cmp::Reverse(site.total_child_bytes),
            site.fixture,
            site.path.clone(),
        )
    });

    println!("# Phase 29-D Nested JSite Package Inventory");
    println!();
    println!("> Generated by `cargo run --example probe_phase29_nested_jsite_inventory`.");
    println!("> Nested JSite packages are not top-level drawing geometry.");
    println!();
    println!("## Summary");
    println!();
    println!(
        "| Fixture | JSite | Class | Child streams | Total child bytes | Families | Sheet children | Symbol path | Local symbol path | GUIDs |"
    );
    println!("|---|---|---|---:|---:|---|---|---|---|---:|");
    for site in &sites {
        println!(
            "| `{}` | `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} |",
            site.fixture,
            site.path,
            classify(site),
            site.child_count,
            site.total_child_bytes,
            joined_set(&site.families),
            joined_sheets(&site.sheet_children),
            site.symbol_path.as_deref().unwrap_or("-"),
            site.local_symbol_path.as_deref().unwrap_or("-"),
            site.guid_count
        );
    }

    println!();
    println!("## Top Child Streams");
    println!();
    println!("| Fixture | JSite | Child stream | Bytes |");
    println!("|---|---|---|---:|");
    for site in sites.iter().take(12) {
        for (path, size) in site.child_streams.iter().take(12) {
            println!(
                "| `{}` | `{}` | `{}` | {} |",
                site.fixture, site.path, path, size
            );
        }
    }

    println!();
    println!("## Decision");
    println!();
    println!("- `NeedsOwnership`: nested package mirrors top-level PSM/Sheet families and needs an owner model before recursive parsing.");
    println!("- `CanTraceHeaderOnly`: cluster-like child streams may be traceable for byte-audit accounting, but should not affect geometry.");
    println!("- `IgnoreUntilConsumerNeeds`: small OLE/JProperties-only groups stay demand-gated.");
}
