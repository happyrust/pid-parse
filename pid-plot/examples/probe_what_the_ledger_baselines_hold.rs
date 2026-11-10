//! What the two committed export-ledger baselines hold, class by class.
//!
//! `docs/baselines/export-ledger-backup.json` (53 backup sheets) and
//! `docs/baselines/export-ledger-fixtures.json` (7 committed fixtures) keep
//! one account per page. A gate compares a run against them page by page —
//! `export_plant_backup` and `export_ledger_fixtures` — so while those two
//! pass, the baselines *are* the run's readings, and summing them answers
//! "how many of each kind of thing did not reach a page" without laying a
//! single page out. This probe does that sum; it is the source of the tables
//! in
//! `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`.
//!
//! For each baseline it prints:
//!
//! - every suspect item, page by page: domain, layer, reason, count, and the
//!   stream and type code the account names for it;
//! - the totals per tier, layer, domain and reason, with the number of pages
//!   that carry each;
//! - the parse layer by reason and record family (or type code);
//! - the entity cross table — kind × confidence × outcome — which is what
//!   tells a dropped *record* from a dropped *guess*;
//! - the partial marks;
//! - the page facts: where each page's size came from, which template each
//!   wears and how many wear none.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_what_the_ledger_baselines_hold
//! cargo run -q -p pid-plot --example probe_what_the_ledger_baselines_hold -- <baseline.json> [...]
//! ```
//!
//! Reads committed files only: no corpus, no typeface, no environment
//! variable.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_plot::ledger::{DropReason, Tier};
use serde_json::Value;

/// `count` and the pages that carry a non-zero share of it.
#[derive(Default)]
struct Tally {
    count: u64,
    pages: BTreeSet<String>,
}

impl Tally {
    fn add(&mut self, count: u64, page: &str) {
        self.count += count;
        if count > 0 {
            self.pages.insert(page.to_string());
        }
    }
}

/// The tier a reason code belongs to, from the catalog itself.
fn tier_of(code: &str) -> &'static str {
    DropReason::all()
        .iter()
        .find(|reason| reason.code() == code)
        .map_or("unknown", |reason| match reason.tier() {
            Tier::Suspect => "suspect",
            Tier::Intentional => "intentional",
        })
}

fn as_u64(value: &Value) -> u64 {
    value.as_u64().unwrap_or(0)
}

fn counts(node: Option<&Value>) -> Vec<(String, u64)> {
    node.and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), as_u64(value)))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Default)]
struct Summary {
    /// `(tier, layer, domain, reason)`.
    reasons: BTreeMap<(String, String, String, String), Tally>,
    /// `(reason, family or type code, domain)`.
    parse: BTreeMap<(String, String, String), Tally>,
    /// `(domain, kind, confidence, outcome)`.
    cross: BTreeMap<(String, String, String, String), Tally>,
    /// `(domain, mark)`.
    marks: BTreeMap<(String, String), Tally>,
    /// Page size source.
    size_sources: BTreeMap<String, Tally>,
    /// `(template name, source kind)`; an empty name is a bare page.
    templates: BTreeMap<(String, String), Tally>,
    /// Placement body sources, `(domain, source)`.
    placements: BTreeMap<(String, String), Tally>,
    /// Per page: the suspect lines to print.
    suspects: BTreeMap<String, Vec<String>>,
    entities: u64,
    on_page: u64,
    unattributed: u64,
    primitives_laid_out: u64,
    primitives_on_page: u64,
    segments_total: u64,
    segments_written: u64,
}

impl Summary {
    fn reason(&mut self, layer: &str, domain: &str, code: &str, count: u64, page: &str) -> bool {
        let tier = tier_of(code);
        self.reasons
            .entry((
                tier.to_string(),
                layer.to_string(),
                domain.to_string(),
                code.to_string(),
            ))
            .or_default()
            .add(count, page);
        tier == "suspect" && count > 0
    }

    fn page(&mut self, name: &str, page: &Value) {
        let facts = &page["page"];
        self.size_sources
            .entry(facts["size_source"].as_str().unwrap_or("?").to_string())
            .or_default()
            .add(1, name);
        let template = &facts["template"];
        self.templates
            .entry((
                template["name"].as_str().unwrap_or("").to_string(),
                template["source"].as_str().unwrap_or("").to_string(),
            ))
            .or_default()
            .add(1, name);
        for (code, count) in counts(facts.get("dropped")) {
            if self.reason("page", "page", &code, count, name) {
                self.suspects
                    .entry(name.to_string())
                    .or_default()
                    .push(format!("page      page      {code:<32} {count:>5}"));
            }
        }
        for domain in ["drawing", "furniture"] {
            let account = &page[domain];
            let parse = &account["parse"];
            self.unattributed += as_u64(&parse["unattributed"]);
            for item in parse["items"].as_array().into_iter().flatten() {
                let code = item["reason"].as_str().unwrap_or("?");
                let count = as_u64(&item["count"]);
                let key = item["family"]
                    .as_str()
                    .or_else(|| item["type_code"].as_str())
                    .unwrap_or("-");
                self.parse
                    .entry((code.to_string(), key.to_string(), domain.to_string()))
                    .or_default()
                    .add(count, name);
                if self.reason("parse", domain, code, count, name) {
                    let mut site = format!(
                        "{domain:<9} parse     {code:<32} {count:>5}  {}",
                        item["stream"].as_str().unwrap_or("?")
                    );
                    if let Some(type_code) = item["type_code"].as_str() {
                        site.push_str(&format!(" type {type_code}"));
                    }
                    if let Some(bytes) = item["bytes"].as_u64() {
                        site.push_str(&format!(" ({bytes} bytes)"));
                    }
                    self.suspects
                        .entry(name.to_string())
                        .or_default()
                        .push(site);
                }
            }
            let entities = &account["entities"];
            self.entities += as_u64(&entities["total"]);
            self.on_page += as_u64(&entities["on_page"]);
            self.unattributed += as_u64(&entities["unattributed"]);
            for (code, count) in counts(entities.get("dropped")) {
                if self.reason("entity", domain, &code, count, name) {
                    self.suspects
                        .entry(name.to_string())
                        .or_default()
                        .push(format!("{domain:<9} entity    {code:<32} {count:>5}"));
                }
            }
            for (kind, by_confidence) in entities["cross"].as_object().into_iter().flatten() {
                for (confidence, outcomes) in by_confidence.as_object().into_iter().flatten() {
                    for (outcome, count) in counts(Some(outcomes)) {
                        self.cross
                            .entry((
                                domain.to_string(),
                                kind.clone(),
                                confidence.clone(),
                                outcome,
                            ))
                            .or_default()
                            .add(count, name);
                    }
                }
            }
            let bodies = &account["bodies"];
            self.unattributed += as_u64(&bodies["unattributed"]);
            for (code, count) in counts(bodies.get("dropped")) {
                if self.reason("body", domain, &code, count, name) {
                    self.suspects
                        .entry(name.to_string())
                        .or_default()
                        .push(format!("{domain:<9} body      {code:<32} {count:>5}"));
                }
            }
            for (source, count) in counts(bodies.get("placements")) {
                self.placements
                    .entry((domain.to_string(), source))
                    .or_default()
                    .add(count, name);
            }
            let primitives = &account["primitives"];
            self.unattributed += as_u64(&primitives["unattributed"]);
            self.primitives_laid_out += as_u64(&primitives["laid_out"]);
            self.primitives_on_page += as_u64(&primitives["on_page"]);
            let off_sheet = as_u64(&primitives["off_sheet"]);
            if self.reason("primitive", domain, "off_sheet", off_sheet, name) {
                self.suspects
                    .entry(name.to_string())
                    .or_default()
                    .push(format!(
                        "{domain:<9} primitive {:<32} {off_sheet:>5}",
                        "off_sheet"
                    ));
            }
            for (code, count) in counts(primitives.get("dropped")) {
                if self.reason("primitive", domain, &code, count, name) {
                    self.suspects
                        .entry(name.to_string())
                        .or_default()
                        .push(format!("{domain:<9} primitive {code:<32} {count:>5}"));
                }
            }
            for (mark, count) in counts(primitives.get("marks")) {
                self.marks
                    .entry((domain.to_string(), mark))
                    .or_default()
                    .add(count, name);
            }
            let segments = &account["segments"];
            self.unattributed += as_u64(&segments["unattributed"]);
            self.segments_total += as_u64(&segments["total"]);
            self.segments_written += as_u64(&segments["written"]);
            for (code, count) in counts(segments.get("dropped")) {
                if self.reason("segment", domain, &code, count, name) {
                    self.suspects
                        .entry(name.to_string())
                        .or_default()
                        .push(format!("{domain:<9} segment   {code:<32} {count:>5}"));
                }
            }
            let text = &account["text"];
            self.reason(
                "text",
                domain,
                "text_control_char",
                as_u64(&text["control_chars"]),
                name,
            );
            self.reason(
                "text",
                domain,
                "text_trailing_empty_line",
                as_u64(&text["trailing_empty_lines"]),
                name,
            );
        }
    }

    fn print(&self, pages: usize) {
        println!(
            "  {pages} pages; entities {} of which on the page {}; primitives laid out {} of which on the page {}; segments {} of which written {}; unattributed {}",
            self.entities,
            self.on_page,
            self.primitives_laid_out,
            self.primitives_on_page,
            self.segments_total,
            self.segments_written,
            self.unattributed
        );

        println!("\n  -- suspect items, page by page (domain / layer / reason / count / site)");
        if self.suspects.is_empty() {
            println!("     none");
        }
        for (page, lines) in &self.suspects {
            println!("     {page}");
            for line in lines {
                println!("        {line}");
            }
        }

        println!("\n  -- totals (tier / layer / domain / reason / count / pages)");
        let mut tier_totals: BTreeMap<&str, u64> = BTreeMap::new();
        for ((tier, layer, domain, code), tally) in &self.reasons {
            if tally.count == 0 {
                continue;
            }
            println!(
                "     {tier:<11} {layer:<9} {domain:<9} {code:<32} {:>8} {:>3}",
                tally.count,
                tally.pages.len()
            );
            // The gate's own total (`ExportLedger::summary`) adds every
            // layer's `dropped` map and the parse items, so an entity whose
            // one primitive was dropped counts at both layers. The two text
            // normalizer counts and `off_sheet` are not in those maps.
            let counted = !(layer == "text" || (layer == "primitive" && code == "off_sheet"));
            if counted {
                *tier_totals.entry(tier.as_str()).or_default() += tally.count;
            }
        }
        for (tier, total) in &tier_totals {
            println!("     {tier} total as the gate sums it (every layer's dropped map): {total}");
        }

        println!("\n  -- parse layer (reason / family or type code / domain / count / pages)");
        for ((code, key, domain), tally) in &self.parse {
            println!(
                "     {code:<30} {key:<20} {domain:<9} {:>8} {:>3}",
                tally.count,
                tally.pages.len()
            );
        }

        println!("\n  -- entities (domain / kind / confidence / outcome / count / pages)");
        let mut by_confidence: BTreeMap<(String, String, &str), u64> = BTreeMap::new();
        for ((domain, kind, confidence, outcome), tally) in &self.cross {
            println!(
                "     {domain:<9} {kind:<16} {confidence:<10} {outcome:<28} {:>8} {:>3}",
                tally.count,
                tally.pages.len()
            );
            let fate = if outcome == "on_page" {
                "on_page"
            } else {
                "dropped"
            };
            *by_confidence
                .entry((domain.clone(), confidence.clone(), fate))
                .or_default() += tally.count;
        }
        println!("     by confidence:");
        for ((domain, confidence, fate), count) in &by_confidence {
            println!("       {domain:<9} {confidence:<10} {fate:<8} {count:>8}");
        }

        println!("\n  -- placements by body source (domain / source / count / pages)");
        for ((domain, source), tally) in &self.placements {
            println!(
                "     {domain:<9} {source:<12} {:>8} {:>3}",
                tally.count,
                tally.pages.len()
            );
        }

        println!("\n  -- partial marks (domain / mark / primitives / pages)");
        if self.marks.is_empty() {
            println!("     none");
        }
        for ((domain, mark), tally) in &self.marks {
            println!(
                "     {domain:<9} {mark:<24} {:>6} {:>3}",
                tally.count,
                tally.pages.len()
            );
        }

        println!("\n  -- page facts");
        for (source, tally) in &self.size_sources {
            println!("     page size {source}: {} page(s)", tally.count);
            if source != "declared" {
                for page in &tally.pages {
                    println!("        {page}");
                }
            }
        }
        for ((name, source), tally) in &self.templates {
            if name.is_empty() {
                println!("     no template: {} page(s)", tally.count);
                for page in &tally.pages {
                    println!("        {page}");
                }
            } else {
                println!("     {name} ({source}): {} page(s)", tally.count);
            }
        }
    }
}

fn summarise(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("read: {error}"))?;
    let root: Value = serde_json::from_str(&text).map_err(|error| format!("parse: {error}"))?;
    let pages = root["pages"]
        .as_object()
        .ok_or_else(|| "no `pages` object".to_string())?;
    println!(
        "\n=== {} (schema {}, corpus {}) ===",
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        root["schema"].as_str().unwrap_or("?"),
        root["corpus"].as_str().unwrap_or("?"),
    );
    let mut summary = Summary::default();
    for (name, page) in pages {
        summary.page(name, page);
    }
    summary.print(pages.len());
    Ok(())
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let baselines = if args.is_empty() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/baselines");
        vec![
            dir.join("export-ledger-backup.json"),
            dir.join("export-ledger-fixtures.json"),
        ]
    } else {
        args
    };
    let mut failed = false;
    for path in &baselines {
        if let Err(error) = summarise(path) {
            eprintln!("{}: {error}", path.display());
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
}
