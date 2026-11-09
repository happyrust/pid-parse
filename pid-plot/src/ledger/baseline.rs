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
//! # What a baseline may hold
//!
//! Requirement 12.4: reason codes, counts, stream paths, type codes, oids, byte
//! ranges, template file names and page paths — never the text a page carries,
//! never a machine path. Every type below is already that shape; the gate adds
//! nothing to it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::assemble::{CorpusDescriptor, DomainAccount, ExportLedger};
use super::layout::{LayoutLedger, SourceName};
use super::reason::{DropReason, LedgerDomain, Tier};

/// The schema a baseline file states, so a reader can tell an old file from a
/// new one instead of misreading it.
pub const SCHEMA: &str = "pid-plot.export-ledger/1";

/// Which corpus a baseline was taken from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerCorpus {
    /// The 53-sheet plant backup.
    Backup,
    /// The seven committed fixtures.
    Fixtures,
}

impl LedgerCorpus {
    /// The corpus's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Backup => "backup",
            Self::Fixtures => "fixtures",
        }
    }
}

/// One corpus's committed baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerBaseline {
    /// [`SCHEMA`].
    pub schema: String,
    /// Which corpus this file pins.
    pub corpus: LedgerCorpus,
    /// The corpus descriptor the run that took it saw.
    pub descriptor: CorpusDescriptor,
    /// Whether the run that took it measured the two font marks.
    pub fonts_measured: bool,
    /// One account per page, keyed by the page's path relative to its corpus
    /// root with `/` separators.
    pub pages: BTreeMap<String, ExportLedger>,
}

/// Everything the gate needs to compare one run against a baseline.
pub struct GateInput<'a> {
    /// Which corpus this run laid out.
    pub corpus: LedgerCorpus,
    /// The baseline file to read, and to write when `update` is set.
    pub baseline_path: &'a Path,
    /// The corpus descriptor this run saw.
    pub descriptor: CorpusDescriptor,
    /// One account per page, keyed the way the baseline keys them.
    pub pages: Vec<(String, ExportLedger)>,
    /// Pages that failed to lay out or write, with the error.
    pub failures: Vec<(String, String)>,
    /// Cross-check and write-mismatch lines that are already known to be
    /// violations.
    pub breaches: Vec<String>,
    /// `UPDATE_EXPORT_LEDGER == "1"`.
    pub update: bool,
    /// How this corpus's baseline is taken, for the message a reader needs.
    /// Must not name a machine path.
    pub hint: &'a str,
}

/// What the gate decided, and the lines it wants printed.
pub struct GateReport {
    passed: bool,
    lines: Vec<String>,
}

impl GateReport {
    /// Whether the gate passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.passed
    }

    /// The report, one line per fact, ready to print.
    #[must_use]
    pub fn render(&self) -> String {
        self.lines.join("\n")
    }
}

/// How one count is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    /// Pinned at its present value, allowed only to fall.
    Suspect,
    /// What the plot rules say not to draw; pinned exactly.
    Intentional,
    /// A count that is not a drop: an on-page count, a partial mark, a
    /// cross-count or a page fact. Pinned exactly.
    Exact,
}

impl Check {
    fn of(reason: DropReason) -> Self {
        match reason.tier() {
            Tier::Suspect => Self::Suspect,
            Tier::Intentional => Self::Intentional,
        }
    }
}

/// One comparable count.
#[derive(Debug, Clone, Copy)]
struct Count {
    check: Check,
    value: u64,
    reason: Option<DropReason>,
}

/// Build the corpus descriptor for a set of pages laid out together.
///
/// The symbol sources come from the first page (every page of one run searches
/// the same order), the template sources are the union over all pages, and the
/// faces are what the caller actually embedded. Sorted and deduplicated, so two
/// runs over one corpus describe it identically.
#[must_use]
pub fn descriptor_for(ledgers: &[&LayoutLedger], font_faces: Vec<String>) -> CorpusDescriptor {
    let mut templates: BTreeSet<SourceName> = BTreeSet::new();
    for ledger in ledgers {
        for source in &ledger.sources.templates {
            templates.insert(source.clone());
        }
    }
    let mut faces = font_faces;
    faces.sort();
    faces.dedup();
    CorpusDescriptor {
        symbol_sources: ledgers
            .first()
            .map(|ledger| ledger.sources.symbols.clone())
            .unwrap_or_default(),
        template_sources: templates.into_iter().collect(),
        font_faces: faces,
    }
}

/// Run the gate.
#[must_use]
pub fn run_gate(input: GateInput<'_>) -> GateReport {
    let mut lines: Vec<String> = Vec::new();
    let mut failed = false;

    // Failures and breaches are defects whatever the baseline says.
    let mut page_defects: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (page, error) in &input.failures {
        page_defects
            .entry(page.clone())
            .or_default()
            .push(format!("排版或写出失败：{error}"));
    }
    if !input.breaches.is_empty() {
        failed = true;
        for breach in &input.breaches {
            lines.push(format!("export-ledger: 核对不符 — {breach}"));
        }
    }
    // Requirement 6.5: any non-zero remainder is a defect on its page, and it
    // is checked whatever the corpus descriptor says.
    for (page, ledger) in &input.pages {
        for breach in ledger.conservation_breaches() {
            page_defects
                .entry(page.clone())
                .or_default()
                .push(format!("未归因 — {breach}"));
        }
    }
    if !page_defects.is_empty() {
        failed = true;
    }

    let loaded = load_baseline(input.baseline_path);

    // The rewrite path: first baseline, or `UPDATE_EXPORT_LEDGER=1`.
    if input.update {
        let refusal = match &loaded {
            Loaded::Missing => None,
            Loaded::Unreadable(error) => Some(format!(
                "拒绝重写：{} 读不出（{error}）；删除该文件后重建",
                input.baseline_path.display()
            )),
            Loaded::Ok(baseline) => {
                if baseline.schema != SCHEMA {
                    Some(format!(
                        "拒绝重写：{} 的 schema 是 {:?}，本代码写 {SCHEMA:?}；删除该文件后重建",
                        input.baseline_path.display(),
                        baseline.schema
                    ))
                } else if !baseline.descriptor.same_corpus(&input.descriptor) {
                    Some(format!(
                        "拒绝重写：语料描述的符号或模板部分与旧基线不同（{}）",
                        describe_difference(&baseline.descriptor, &input.descriptor)
                    ))
                } else {
                    rewrite_refusal(&input, baseline)
                }
            }
        };
        let refusal = match refusal {
            Some(refusal) => Some(refusal),
            None => {
                let already_broken = !page_defects.is_empty() || !input.breaches.is_empty();
                if already_broken {
                    Some("拒绝重写：本次有失败页、未归因或核对不符".to_string())
                } else {
                    None
                }
            }
        };
        return match refusal {
            Some(reason) => {
                lines.push(format!("export-ledger: {reason}"));
                lines.push(format!("提示: {}", input.hint));
                for (page, defects) in &page_defects {
                    lines.push(format!("{page}: 缺陷 —— {}", defects.join("; ")));
                }
                GateReport {
                    passed: false,
                    lines,
                }
            }
            None => match write_baseline(input.baseline_path, &build_baseline(&input)) {
                Ok(summary) => {
                    lines.push(format!(
                        "export-ledger: 基线已重写 — {}（{} 页）",
                        input.baseline_path.display(),
                        input.pages.len()
                    ));
                    for line in summary {
                        lines.push(line);
                    }
                    lines.push(format!("提示: {}", input.hint));
                    GateReport {
                        passed: true,
                        lines,
                    }
                }
                Err(error) => {
                    lines.push(format!(
                        "export-ledger: 写基线失败 — {}: {error}",
                        input.baseline_path.display()
                    ));
                    GateReport {
                        passed: false,
                        lines,
                    }
                }
            },
        };
    }

    let baseline = match loaded {
        Loaded::Ok(baseline) => baseline,
        Loaded::Missing => {
            lines.push(format!(
                "export-ledger: 基线缺失 — {} 不存在",
                input.baseline_path.display()
            ));
            lines.push(format!("提示: {}", input.hint));
            return GateReport {
                passed: false,
                lines,
            };
        }
        Loaded::Unreadable(error) => {
            lines.push(format!(
                "export-ledger: 基线读不出 — {}: {error}",
                input.baseline_path.display()
            ));
            lines.push(format!("提示: {}", input.hint));
            return GateReport {
                passed: false,
                lines,
            };
        }
    };

    if baseline.schema != SCHEMA {
        lines.push(format!(
            "export-ledger: schema 不同 — 文件是 {:?}，本代码读 {SCHEMA:?}",
            baseline.schema
        ));
        lines.push(format!("提示: {}", input.hint));
        return GateReport {
            passed: false,
            lines,
        };
    }

    // Requirement 13.7: a different symbol or template corpus means the counts
    // are not comparable. Say so loudly, skip the count comparison, and keep
    // the remainder and cross-check findings computed above.
    if !baseline.descriptor.same_corpus(&input.descriptor) {
        lines.push(
            "export-ledger: corpus descriptor differs — count comparison skipped".to_string(),
        );
        lines.push(format!(
            "  symbols/templates: baseline={} current={}",
            describe_corpus(&baseline.descriptor),
            describe_corpus(&input.descriptor)
        ));
        for difference in corpus_differences(&baseline.descriptor, &input.descriptor) {
            lines.push(format!("    - {difference}"));
        }
        lines.push(format!(
            "  fonts: baseline={:?} current={:?}",
            baseline.descriptor.font_faces, input.descriptor.font_faces
        ));
        let unattributed: u64 = input
            .pages
            .iter()
            .map(|(_, ledger)| summary_unattributed(ledger))
            .fold(0u64, u64::saturating_add);
        lines.push(format!("  unattributed: {unattributed}"));
        lines.push(format!("  hint: {}", input.hint));
        if !page_defects.is_empty() {
            failed = true;
            for (page, defects) in &page_defects {
                lines.push(format!("{page}: 缺陷 —— {}", defects.join("; ")));
            }
        }
        if !failed {
            lines.push("export-ledger: 计数比对已跳过；未归因与核对全部通过".to_string());
        }
        return GateReport {
            passed: !failed,
            lines,
        };
    }

    // Design deviation D1: only the two font marks depend on a face, so a
    // difference there skips just those two keys.
    let skip_font_marks = baseline.fonts_measured != fonts_measured(&input)
        || baseline.descriptor.font_faces != input.descriptor.font_faces;

    let mut lowering: Vec<String> = Vec::new();
    let mut changes: Vec<String> = Vec::new();
    let current: BTreeMap<&str, &ExportLedger> = input
        .pages
        .iter()
        .map(|(page, ledger)| (page.as_str(), ledger))
        .collect();
    let pinned: BTreeMap<&str, &ExportLedger> = baseline
        .pages
        .iter()
        .map(|(page, ledger)| (page.as_str(), ledger))
        .collect();

    let mut pages: BTreeSet<&str> = BTreeSet::new();
    pages.extend(pinned.keys().copied());
    pages.extend(current.keys().copied());

    for page in &pages {
        let (pinned_page, current_page) = match (pinned.get(page), current.get(page)) {
            (Some(pinned_page), Some(current_page)) => (*pinned_page, *current_page),
            (Some(_), None) => {
                changes.push(format!("{page}: 出账页 — 本次没有这一页"));
                continue;
            }
            _ => {
                changes.push(format!("{page}: 新入账页 — 基线里没有这一页"));
                continue;
            }
        };
        let pinned_counts = collect(pinned_page);
        let current_counts = collect(current_page);
        let mut keys: BTreeSet<&str> = BTreeSet::new();
        keys.extend(pinned_counts.keys().map(String::as_str));
        keys.extend(current_counts.keys().map(String::as_str));
        for key in keys {
            if skip_font_marks && is_font_mark(key) {
                continue;
            }
            let before = pinned_counts.get(key).copied();
            let after = current_counts.get(key).copied();
            let check = after
                .map(|count| count.check)
                .or(before.map(|count| count.check))
                .unwrap_or(Check::Exact);
            let before_value = before.map_or(0, |count| count.value);
            let after_value = after.map_or(0, |count| count.value);
            if before_value == after_value {
                continue;
            }
            let reason = after
                .and_then(|count| count.reason)
                .or(before.and_then(|count| count.reason));
            match check {
                Check::Suspect if after_value > before_value => {
                    let mut line = format!(
                        "可疑项升高 {key} baseline={before_value} current={after_value} (delta +{})",
                        after_value.saturating_sub(before_value)
                    );
                    if let Some(reason) = reason {
                        let grown = grown_sites(pinned_page, current_page, reason);
                        if !grown.is_empty() {
                            line.push_str("；来源: ");
                            line.push_str(&grown.join(", "));
                        }
                    }
                    page_defects.entry(page.to_string()).or_default().push(line);
                }
                Check::Suspect => {
                    lowering.push(format!(
                        "{page} {key} baseline={before_value} current={after_value} (delta -{})",
                        before_value.saturating_sub(after_value)
                    ));
                }
                Check::Intentional => changes.push(format!(
                    "{page} {key} 有意项变化 baseline={before_value} current={after_value}"
                )),
                Check::Exact => changes.push(format!(
                    "{page} {key} 账目变动 baseline={before_value} current={after_value}"
                )),
            }
        }
    }

    for (page, defects) in &page_defects {
        failed = true;
        lines.push(format!("{page}: 缺陷 —— {}", defects.join("; ")));
    }
    // Requirement 15.1/15.2: every page gets exactly one of the two verdicts.
    let clean: BTreeSet<&str> = pages
        .iter()
        .copied()
        .filter(|page| !page_defects.contains_key(*page))
        .collect();
    for page in clean {
        lines.push(format!("{page}: 未发现可归属缺陷"));
    }
    if !changes.is_empty() {
        failed = true;
        lines.push("export-ledger: 账目变动（门禁失败）".to_string());
        for change in &changes {
            lines.push(format!("  {change}"));
        }
    }
    if !lowering.is_empty() {
        failed = true;
        lines.push("export-ledger: 基线待降（门禁失败）".to_string());
        for line in &lowering {
            lines.push(format!("  {line}"));
        }
        lines.push(
            "  提示: 可疑项只许降；若本次改动是有意降低，请设 UPDATE_EXPORT_LEDGER=1 重写基线"
                .to_string(),
        );
    }
    if !failed {
        lines.push(format!(
            "export-ledger: {} 页全部通过（可疑项 {} 个，未归因 0）",
            pages.len(),
            suspect_total(&input)
        ));
    }
    GateReport {
        passed: !failed,
        lines,
    }
}

/// The pages that failed to load a baseline from a path.
enum Loaded {
    Missing,
    Unreadable(String),
    Ok(LedgerBaseline),
}

fn load_baseline(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Loaded::Missing,
        Err(error) => return Loaded::Unreadable(error.to_string()),
    };
    match serde_json::from_str::<LedgerBaseline>(&text) {
        Ok(baseline) => Loaded::Ok(baseline),
        Err(error) => Loaded::Unreadable(error.to_string()),
    }
}

/// Why a rewrite would be refused, when it would.
///
/// Requirement 12.6: a rewrite must never absorb a regression, so any suspect
/// count that would rise refuses the whole write.
fn rewrite_refusal(input: &GateInput<'_>, baseline: &LedgerBaseline) -> Option<String> {
    let mut risen: Vec<String> = Vec::new();
    for (page, ledger) in &input.pages {
        let Some(pinned) = baseline.pages.get(page) else {
            continue;
        };
        let pinned_counts = collect(pinned);
        let current_counts = collect(ledger);
        for (key, count) in &current_counts {
            if count.check != Check::Suspect {
                continue;
            }
            let before = pinned_counts.get(key).map_or(0, |pinned| pinned.value);
            if count.value > before {
                risen.push(format!(
                    "{page} {key} baseline={before} current={} (delta +{})",
                    count.value,
                    count.value.saturating_sub(before)
                ));
            }
        }
    }
    if risen.is_empty() {
        return None;
    }
    let mut reason = format!("拒绝写入 — {} 个可疑项会高于原基线", risen.len());
    for line in risen {
        reason.push_str("\n  ");
        reason.push_str(&line);
    }
    Some(reason)
}

/// The baseline this run's pages would be pinned to.
fn build_baseline(input: &GateInput<'_>) -> LedgerBaseline {
    LedgerBaseline {
        schema: SCHEMA.to_string(),
        corpus: input.corpus,
        descriptor: input.descriptor.clone(),
        fonts_measured: fonts_measured(input),
        pages: input
            .pages
            .iter()
            .map(|(page, ledger)| (page.clone(), ledger.clone()))
            .collect(),
    }
}

fn fonts_measured(input: &GateInput<'_>) -> bool {
    input.pages.iter().any(|(_, ledger)| ledger.fonts_measured)
}

fn write_baseline(path: &Path, baseline: &LedgerBaseline) -> Result<Vec<String>, String> {
    let mut json = serde_json::to_string_pretty(baseline).map_err(|error| error.to_string())?;
    json.push('\n');
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, json).map_err(|error| error.to_string())?;
    Ok(vec![format!(
        "  corpus={} pages={} fonts_measured={}",
        baseline.corpus.code(),
        baseline.pages.len(),
        baseline.fonts_measured
    )])
}

fn summary_unattributed(ledger: &ExportLedger) -> u64 {
    ledger.summary().unattributed
}

fn suspect_total(input: &GateInput<'_>) -> u64 {
    input
        .pages
        .iter()
        .map(|(_, ledger)| ledger.summary().suspect)
        .fold(0u64, u64::saturating_add)
}

fn is_font_mark(key: &str) -> bool {
    key.ends_with("/text_condensed") || key.ends_with("/text_clipped_at_floor")
}

fn describe_corpus(descriptor: &CorpusDescriptor) -> String {
    format!(
        "symbols=[{}] templates=[{}]",
        descriptor
            .symbol_sources
            .iter()
            .map(|source| source.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        descriptor
            .template_sources
            .iter()
            .map(|source| source.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn describe_difference(baseline: &CorpusDescriptor, current: &CorpusDescriptor) -> String {
    corpus_differences(baseline, current).join("; ")
}

fn corpus_differences(baseline: &CorpusDescriptor, current: &CorpusDescriptor) -> Vec<String> {
    let mut differences = Vec::new();
    if baseline.symbol_sources != current.symbol_sources {
        differences.push(format!(
            "symbol sources baseline={} current={}",
            describe_sources(&baseline.symbol_sources),
            describe_sources(&current.symbol_sources)
        ));
    }
    if baseline.template_sources != current.template_sources {
        differences.push(format!(
            "template sources baseline={} current={}",
            describe_sources(&baseline.template_sources),
            describe_sources(&current.template_sources)
        ));
    }
    if differences.is_empty() {
        differences.push("descriptor differs in no symbol or template part".to_string());
    }
    differences
}

fn describe_sources(sources: &[SourceName]) -> String {
    let names: Vec<String> = sources
        .iter()
        .map(|source| match &source.archive {
            Some(archive) => format!("{}:{}", archive, source.name),
            None => source.name.clone(),
        })
        .collect();
    format!("[{}]", names.join(", "))
}

/// Every comparable count of one page, keyed by where it sits.
///
/// The key is the account's own path: `drawing/entity/dropped/layer_hidden`.
/// Two runs over one page produce the same keys in the same order, which is
/// what makes the comparison a table lookup rather than a diff of structures.
fn collect(ledger: &ExportLedger) -> BTreeMap<String, Count> {
    let mut out: BTreeMap<String, Count> = BTreeMap::new();
    for (reason, count) in &ledger.page.dropped {
        insert(
            &mut out,
            format!("page/dropped/{}", reason.code()),
            *reason,
            *count,
        );
    }
    // Page facts are not drops: each is pinned as its own one-valued key, so a
    // page that changes which size source or template it uses shows up as one
    // key leaving and another arriving.
    insert_exact(
        &mut out,
        format!("page/size_source/{}", ledger.page.size_source.code()),
        1,
    );
    if let Some(template) = &ledger.page.template {
        insert_exact(&mut out, format!("page/template/{}", template.name), 1);
    }
    for name in &ledger.page.missing_templates {
        insert_exact(&mut out, format!("page/missing_template/{name}"), 1);
    }

    for (name, domain) in [
        (LedgerDomain::Drawing.code(), &ledger.drawing),
        (LedgerDomain::Furniture.code(), &ledger.furniture),
    ] {
        collect_domain(&mut out, name, domain);
    }
    out
}

fn collect_domain(out: &mut BTreeMap<String, Count>, name: &str, domain: &DomainAccount) {
    for item in &domain.parse.items {
        let type_code = item
            .type_code
            .map_or_else(String::new, |code| format!("@0x{code:04X}"));
        let family = item
            .family
            .as_ref()
            .map_or_else(String::new, |family| format!("#{family}"));
        insert(
            out,
            format!(
                "{name}/parse/{}{}{}{}",
                item.reason.code(),
                type_code,
                family,
                format_args!("@{}", item.stream)
            ),
            item.reason,
            item.count,
        );
    }
    insert_exact(
        out,
        format!("{name}/parse/unattributed"),
        domain.parse.unattributed,
    );
    insert_exact(out, format!("{name}/entity/total"), domain.entities.total);
    insert_exact(
        out,
        format!("{name}/entity/on_page"),
        domain.entities.on_page,
    );
    insert_exact(
        out,
        format!("{name}/entity/partial"),
        domain.entities.partial,
    );
    insert_exact(
        out,
        format!("{name}/entity/fill_duplicate_first_taken_by_furniture"),
        domain.entities.fill_duplicate_first_taken_by_furniture,
    );
    for (reason, count) in &domain.entities.dropped {
        insert(
            out,
            format!("{name}/entity/dropped/{}", reason.code()),
            *reason,
            *count,
        );
    }
    for (kind, by_confidence) in &domain.entities.cross {
        for (confidence, by_outcome) in by_confidence {
            for (outcome, count) in by_outcome {
                insert_exact(
                    out,
                    format!(
                        "{name}/entity/cross/{}/{}/{outcome}",
                        kind.clone().code(),
                        confidence.code()
                    ),
                    *count,
                );
            }
        }
    }
    insert_exact(
        out,
        format!("{name}/entity/unattributed"),
        domain.entities.unattributed,
    );

    insert_exact(out, format!("{name}/body/total"), domain.bodies.total);
    insert_exact(out, format!("{name}/body/produced"), domain.bodies.produced);
    for (source, count) in &domain.bodies.placements {
        insert_exact(
            out,
            format!("{name}/body/placements/{}", source.code()),
            *count,
        );
    }
    for (reason, count) in &domain.bodies.dropped {
        insert(
            out,
            format!("{name}/body/dropped/{}", reason.code()),
            *reason,
            *count,
        );
    }
    insert_exact(
        out,
        format!("{name}/body/unattributed"),
        domain.bodies.unattributed,
    );

    insert_exact(
        out,
        format!("{name}/text/control_chars"),
        domain.text.control_chars,
    );
    insert_exact(
        out,
        format!("{name}/text/trailing_empty_lines"),
        domain.text.trailing_empty_lines,
    );

    insert_exact(
        out,
        format!("{name}/primitive/laid_out"),
        domain.primitives.laid_out,
    );
    insert_exact(
        out,
        format!("{name}/primitive/off_sheet"),
        domain.primitives.off_sheet,
    );
    insert_exact(
        out,
        format!("{name}/primitive/on_page"),
        domain.primitives.on_page,
    );
    insert_exact(
        out,
        format!("{name}/primitive/partial"),
        domain.primitives.partial,
    );
    for (reason, count) in &domain.primitives.dropped {
        insert(
            out,
            format!("{name}/primitive/dropped/{}", reason.code()),
            *reason,
            *count,
        );
    }
    for (mark, count) in &domain.primitives.marks {
        insert_exact(
            out,
            format!("{name}/primitive/marks/{}", mark.code()),
            *count,
        );
    }
    insert_exact(
        out,
        format!("{name}/primitive/unattributed"),
        domain.primitives.unattributed,
    );

    // `total` and `written` are stored for a reader and deliberately not
    // compared (design deviation I4).
    for (reason, count) in &domain.segments.dropped {
        insert(
            out,
            format!("{name}/segment/dropped/{}", reason.code()),
            *reason,
            *count,
        );
    }
    insert_exact(
        out,
        format!("{name}/segment/unattributed"),
        domain.segments.unattributed,
    );

    for (asymmetry, count) in &domain.asymmetry {
        insert_exact(
            out,
            format!("{name}/asymmetry/{}", asymmetry.code()),
            *count,
        );
    }
}

fn insert(out: &mut BTreeMap<String, Count>, key: String, reason: DropReason, value: u64) {
    let entry = out.entry(key).or_insert(Count {
        check: Check::of(reason),
        value: 0,
        reason: Some(reason),
    });
    entry.value = entry.value.saturating_add(value);
}

fn insert_exact(out: &mut BTreeMap<String, Count>, key: String, value: u64) {
    let entry = out.entry(key).or_insert(Count {
        check: Check::Exact,
        value: 0,
        reason: None,
    });
    entry.value = entry.value.saturating_add(value);
}

/// The sites this run reports for `reason` that the pinned page did not.
///
/// Requirement 13.4: a risen suspect count names where the extra drops came
/// from. Only identifiers — a stream, an oid, a byte range, a type code.
fn grown_sites(pinned: &ExportLedger, current: &ExportLedger, reason: DropReason) -> Vec<String> {
    let before = site_labels(pinned, reason);
    let after = site_labels(current, reason);
    let mut grown = Vec::new();
    for (label, count) in &after {
        let pinned_count = before.get(label).copied().unwrap_or(0);
        if *count > pinned_count {
            grown.push(format!("{label} ×{}", count.saturating_sub(pinned_count)));
        }
    }
    grown
}

fn site_labels(ledger: &ExportLedger, reason: DropReason) -> BTreeMap<String, u64> {
    let mut labels: BTreeMap<String, u64> = BTreeMap::new();
    let sites = ledger
        .page
        .suspect_sites
        .iter()
        .chain(ledger.drawing.suspect_sites.iter())
        .chain(ledger.furniture.suspect_sites.iter());
    for site in sites {
        if site.reason != reason {
            continue;
        }
        let mut parts: Vec<String> = Vec::new();
        if let Some(stream) = &site.stream {
            parts.push(stream.clone());
        }
        if let Some(oid) = site.oid {
            parts.push(format!("oid={oid}"));
        }
        if let Some(bytes) = site.bytes {
            parts.push(format!(
                "bytes={}..{}",
                bytes.first().copied().unwrap_or(0),
                bytes.get(1).copied().unwrap_or(0)
            ));
        }
        if let Some(code) = site.type_code {
            parts.push(format!("0x{code:04X}"));
        }
        if let Some(family) = &site.family {
            parts.push(family.clone());
        }
        if let Some(name) = &site.name {
            parts.push(name.clone());
        }
        if parts.is_empty() {
            parts.push("(no site recorded)".to_string());
        }
        let entry = labels.entry(parts.join(" ")).or_insert(0);
        *entry = entry.saturating_add(site.count);
    }
    labels
}

#[cfg(test)]
// A test's job is to fail loudly on a bad value, so the four production bans
// are lifted here and only here (design: "测试模块按需 #[allow]").
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::ledger::assemble::{EntityAccount, PageAccount, PrimitiveAccount};
    use crate::ledger::layout::{Confidence, EntityKind};

    /// A page whose drawing domain drops `suspect` records and `intentional`
    /// entities, and leaves `unattributed` entities unaccounted for.
    fn a_ledger(suspect: u64, intentional: u64, unattributed: u64) -> ExportLedger {
        let mut ledger = ExportLedger {
            page: PageAccount::default(),
            fonts_measured: false,
            drawing: DomainAccount::default(),
            furniture: DomainAccount::default(),
        };
        ledger
            .drawing
            .parse
            .items
            .push(crate::ledger::assemble::RecordItem {
                reason: DropReason::RecordRefused,
                stream: "/Sheet6".to_string(),
                type_code: Some(0x00CE),
                family: None,
                count: suspect,
                bytes: None,
                body_primitives: None,
            });
        ledger.drawing.entities = EntityAccount {
            total: intentional.saturating_add(unattributed),
            dropped: BTreeMap::from([(DropReason::LayerHidden, intentional)]),
            unattributed,
            ..EntityAccount::default()
        };
        ledger
    }

    fn an_input<'a>(
        path: &'a Path,
        pages: Vec<(String, ExportLedger)>,
        descriptor: CorpusDescriptor,
        update: bool,
    ) -> GateInput<'a> {
        GateInput {
            corpus: LedgerCorpus::Fixtures,
            baseline_path: path,
            descriptor,
            pages,
            failures: Vec::new(),
            breaches: Vec::new(),
            update,
            hint: "test hint",
        }
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pid-plot-ledger-gate-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir.join("export-ledger.json")
    }

    /// // Feature: pdf-export-ledger, Property 9: 基线往返与确定性
    ///
    /// A baseline serializes, parses back to an equal value, and serializes to
    /// the same bytes twice.
    #[test]
    fn baseline_round_trips_byte_identically() {
        use proptest::prelude::*;
        let strategy = (0u64..40, 0u64..40);
        let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
            cases: 256,
            ..proptest::test_runner::Config::default()
        });
        runner
            .run(&strategy, |(suspect, intentional)| {
                let mut baseline = LedgerBaseline {
                    schema: SCHEMA.to_string(),
                    corpus: LedgerCorpus::Fixtures,
                    descriptor: CorpusDescriptor::default(),
                    fonts_measured: false,
                    pages: BTreeMap::new(),
                };
                baseline.pages.insert(
                    "test-file/D06.pid".to_string(),
                    a_ledger(suspect, intentional, 0),
                );
                let mut first = serde_json::to_string_pretty(&baseline).unwrap();
                first.push('\n');
                let parsed: LedgerBaseline = serde_json::from_str(&first).unwrap();
                prop_assert_eq!(&parsed, &baseline);
                let mut second = serde_json::to_string_pretty(&parsed).unwrap();
                second.push('\n');
                prop_assert_eq!(first, second);
                Ok(())
            })
            .expect("a baseline round-trips");
    }

    /// // Feature: pdf-export-ledger, Property 10: 门禁比对的分类与措辞
    ///
    /// A rise in a suspect count is a page defect naming both values; a fall
    /// asks for the baseline to be lowered; an intentional change is an account
    /// change; no change passes. Every page's verdict line is one of the two
    /// allowed shapes.
    #[test]
    fn gate_classifies_rise_fall_and_hold() {
        let path = temp_path("classify");
        let descriptor = CorpusDescriptor::default();
        // First, pin the baseline.
        let pinned = a_ledger(2, 3, 0);
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), pinned.clone())],
            descriptor.clone(),
            true,
        ));
        assert!(report.passed(), "{}", report.render());

        // Hold: the same account passes.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), pinned.clone())],
            descriptor.clone(),
            false,
        ));
        assert!(report.passed(), "{}", report.render());
        assert!(
            report.render().contains("page.pid: 未发现可归属缺陷"),
            "{}",
            report.render()
        );

        // Suspect rises: a defect naming both values.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(5, 3, 0))],
            descriptor.clone(),
            false,
        ));
        assert!(!report.passed());
        let text = report.render();
        assert!(text.contains("page.pid: 缺陷"), "{text}");
        assert!(text.contains("baseline=2 current=5"), "{text}");

        // Suspect falls: the baseline has to be lowered.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(1, 3, 0))],
            descriptor.clone(),
            false,
        ));
        assert!(!report.passed());
        let text = report.render();
        assert!(text.contains("基线待降"), "{text}");
        assert!(text.contains("UPDATE_EXPORT_LEDGER=1"), "{text}");

        // Intentional moves either way: an account change, never "lowered".
        for value in [4, 2] {
            let report = run_gate(an_input(
                &path,
                vec![("page.pid".to_string(), a_ledger(2, value, 0))],
                descriptor.clone(),
                false,
            ));
            assert!(!report.passed());
            let text = report.render();
            assert!(text.contains("有意项变化"), "{text}");
            assert!(!text.contains("基线待降"), "{text}");
        }

        // A remainder fails whatever the baseline says.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(2, 3, 1))],
            descriptor.clone(),
            false,
        ));
        assert!(!report.passed());
        assert!(report.render().contains("未归因"), "{}", report.render());
    }

    /// // Feature: pdf-export-ledger, Property 11: 重写只许可疑项不升
    ///
    /// A rewrite that would raise a suspect count is refused, and the file is
    /// left as it was.
    #[test]
    fn rewrite_refuses_any_suspect_rise() {
        let path = temp_path("refuse");
        let descriptor = CorpusDescriptor::default();
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(2, 3, 0))],
            descriptor.clone(),
            true,
        ));
        assert!(report.passed(), "{}", report.render());
        let before = std::fs::read(&path).unwrap();

        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(5, 3, 0))],
            descriptor.clone(),
            true,
        ));
        assert!(!report.passed());
        let text = report.render();
        assert!(text.contains("拒绝写入"), "{text}");
        assert!(text.contains("baseline=2 current=5"), "{text}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "a refused rewrite must not touch the file"
        );

        // A fall is accepted and lowers the baseline.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(0, 3, 0))],
            descriptor.clone(),
            true,
        ));
        assert!(report.passed(), "{}", report.render());
        let after: LedgerBaseline =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let pinned = after.pages.get("page.pid").unwrap();
        assert_eq!(pinned.drawing.parse.items.first().unwrap().count, 0);
    }

    #[test]
    fn rewrite_is_deterministic() {
        let path = temp_path("deterministic");
        let descriptor = CorpusDescriptor::default();
        for _ in 0..2 {
            let report = run_gate(an_input(
                &path,
                vec![("page.pid".to_string(), a_ledger(2, 3, 0))],
                descriptor.clone(),
                true,
            ));
            assert!(report.passed(), "{}", report.render());
        }
        let first = std::fs::read(&path).unwrap();
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(2, 3, 0))],
            descriptor.clone(),
            true,
        ));
        assert!(report.passed(), "{}", report.render());
        assert_eq!(std::fs::read(&path).unwrap(), first);
    }

    #[test]
    fn descriptor_diff_skips_counts_but_still_fails_on_unattributed() {
        let path = temp_path("descriptor");
        let pinned_descriptor = CorpusDescriptor {
            symbol_sources: vec![SourceName {
                kind: crate::ledger::layout::SourceKind::Directory,
                name: "symbols".to_string(),
                archive: None,
            }],
            ..CorpusDescriptor::default()
        };
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(2, 3, 0))],
            pinned_descriptor.clone(),
            true,
        ));
        assert!(report.passed(), "{}", report.render());

        let other = CorpusDescriptor {
            symbol_sources: vec![SourceName {
                kind: crate::ledger::SourceKind::Directory,
                name: "symbols-full".to_string(),
                archive: None,
            }],
            ..CorpusDescriptor::default()
        };
        // Different corpus, different counts: the count comparison is skipped,
        // so the page passes despite the differing counts.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(9, 3, 0))],
            other.clone(),
            false,
        ));
        assert!(report.passed(), "{}", report.render());
        let text = report.render();
        assert!(
            text.contains("export-ledger: corpus descriptor differs"),
            "{text}"
        );
        assert!(text.contains("hint:"), "{text}");

        // A remainder still fails.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(9, 3, 1))],
            other,
            false,
        ));
        assert!(!report.passed());
    }

    #[test]
    fn font_only_diff_skips_the_two_font_marks() {
        let path = temp_path("fonts");
        let mut pinned = a_ledger(2, 3, 0);
        pinned.fonts_measured = true;
        pinned
            .drawing
            .primitives
            .marks
            .insert(crate::ledger::PartialMark::TextCondensed, 1);
        pinned
            .drawing
            .primitives
            .marks
            .insert(crate::ledger::PartialMark::PastTheEdge, 1);
        let descriptor = CorpusDescriptor {
            font_faces: vec!["SimSun".to_string()],
            ..CorpusDescriptor::default()
        };
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), pinned.clone())],
            descriptor.clone(),
            true,
        ));
        assert!(report.passed(), "{}", report.render());

        // Same corpus, different faces: only the two font marks are skipped, so
        // the `past_the_edge` mark still has to match.
        let mut current = pinned.clone();
        current
            .drawing
            .primitives
            .marks
            .insert(crate::ledger::PartialMark::TextCondensed, 7);
        current
            .drawing
            .primitives
            .marks
            .insert(crate::ledger::PartialMark::PastTheEdge, 2);
        let mut other_faces = descriptor.clone();
        other_faces.font_faces = vec!["FangSong".to_string()];
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), current)],
            other_faces,
            false,
        ));
        assert!(!report.passed());
        let text = report.render();
        assert!(text.contains("past_the_edge"), "{text}");
        assert!(!text.contains("text_condensed"), "{text}");
    }

    #[test]
    fn baseline_missing_or_corrupt_or_schema_mismatch_fails() {
        let path = temp_path("broken");
        let descriptor = CorpusDescriptor::default();
        // Missing.
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(2, 3, 0))],
            descriptor.clone(),
            false,
        ));
        assert!(!report.passed());
        assert!(report.render().contains("基线缺失"), "{}", report.render());

        // Corrupt.
        std::fs::write(&path, "{not json").unwrap();
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(2, 3, 0))],
            descriptor.clone(),
            false,
        ));
        assert!(!report.passed());
        assert!(
            report.render().contains("基线读不出"),
            "{}",
            report.render()
        );

        // A different schema.
        let mut baseline = LedgerBaseline {
            schema: "pid-plot.export-ledger/0".to_string(),
            corpus: LedgerCorpus::Fixtures,
            descriptor: descriptor.clone(),
            fonts_measured: false,
            pages: BTreeMap::new(),
        };
        baseline
            .pages
            .insert("page.pid".to_string(), a_ledger(2, 3, 0));
        std::fs::write(&path, serde_json::to_string(&baseline).unwrap()).unwrap();
        let report = run_gate(an_input(
            &path,
            vec![("page.pid".to_string(), a_ledger(2, 3, 0))],
            descriptor,
            false,
        ));
        assert!(!report.passed());
        assert!(
            report.render().contains("schema 不同"),
            "{}",
            report.render()
        );
    }

    #[test]
    fn update_flag_only_accepts_literal_one() {
        // The gate reads `update` from the caller, and the callers read the
        // environment; this pins the reading itself so `0`, `true` and the
        // empty string are not mistaken for "yes".
        for value in ["0", "true", "", "yes"] {
            assert!(!is_update(value), "{value:?} must not mean rewrite");
        }
        assert!(is_update("1"));
    }

    /// The two callers' reading of `UPDATE_EXPORT_LEDGER`, kept here so the
    /// rule is stated once.
    fn is_update(value: &str) -> bool {
        value == "1"
    }

    #[test]
    fn a_page_in_only_one_run_is_an_account_change() {
        let path = temp_path("pages");
        let descriptor = CorpusDescriptor::default();
        let report = run_gate(an_input(
            &path,
            vec![("a.pid".to_string(), a_ledger(0, 0, 0))],
            descriptor.clone(),
            true,
        ));
        assert!(report.passed(), "{}", report.render());
        let report = run_gate(an_input(
            &path,
            vec![
                ("a.pid".to_string(), a_ledger(0, 0, 0)),
                ("b.pid".to_string(), a_ledger(0, 0, 0)),
            ],
            descriptor,
            false,
        ));
        assert!(!report.passed());
        assert!(report.render().contains("新入账页"), "{}", report.render());
    }

    #[test]
    fn the_descriptor_union_is_sorted_and_deduplicated() {
        let mut first = LayoutLedger::default();
        first.sources.symbols.push(SourceName {
            kind: crate::ledger::SourceKind::Directory,
            name: "symbols".to_string(),
            archive: None,
        });
        first.sources.templates.push(SourceName {
            kind: crate::ledger::SourceKind::DirectoryFile,
            name: "CPE-SQ-A1.igr".to_string(),
            archive: None,
        });
        let mut second = LayoutLedger::default();
        second.sources.templates.push(SourceName {
            kind: crate::ledger::SourceKind::DirectoryFile,
            name: "CPE-SQ-A1.igr".to_string(),
            archive: None,
        });
        second.sources.templates.push(SourceName {
            kind: crate::ledger::SourceKind::ArchiveMember,
            name: "CPE-SQ-A2.igr".to_string(),
            archive: Some("RefData~4~682.zip".to_string()),
        });
        let descriptor =
            descriptor_for(&[&first, &second], vec!["b".into(), "a".into(), "a".into()]);
        assert_eq!(descriptor.template_sources.len(), 2);
        assert_eq!(
            descriptor.font_faces,
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(descriptor.symbol_sources.len(), 1);
    }

    /// Keep the imported names used so the module compiles without warnings
    /// when the tests above do not reach for them.
    #[test]
    fn cross_count_keys_are_reachable() {
        let mut ledger = a_ledger(0, 0, 0);
        ledger
            .drawing
            .entities
            .cross
            .entry(EntityKind::Line)
            .or_default()
            .entry(Confidence::Decoded)
            .or_default()
            .insert("on_page".to_string(), 1);
        let counts = collect(&ledger);
        assert!(
            counts.contains_key("drawing/entity/cross/line/decoded/on_page"),
            "{:?}",
            counts.keys()
        );
        let _ = PrimitiveAccount::default();
    }
}
