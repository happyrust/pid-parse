//! S1 evidence: how many item tags (and classes) can the `.pid` alone give
//! back, without the `_Data.xml` published beside it?
//!
//! OCS spec `pid-import-next-round`, design D6 / requirement 9 (decision
//! P-D7: evidence only). Today `label` / `class` come only from the
//! `_Data.xml` through the `GraphicOID` two-hop join
//! (`docs/analysis/2026-08-07-graphic-oid-is-the-semantic-join.md`). This
//! probe measures three routes that read links inside the `.pid` only; the
//! `_Data.xml` is used for scoring and nothing else.
//!
//! Scoring rules (fixed here before any score was looked at):
//!
//! 1. Answer key: parse the publish copy with the default (full) parser,
//!    load `<stem>_Data.xml` with `PidSemanticIndex::load_beside`, and resolve
//!    every in-file graphic-family record (`GLine2d`, `igLine2d`,
//!    `igLineString2d`, `igPoint2d`, `igTextBox`, `igSymbol2d`) with
//!    `PidSemanticIndex::resolve` (the two-hop rule). An item is one
//!    published object: `(GraphicOID, label, class)`, where the label is
//!    `PidSemanticObject::label()` -- `ItemTag`, else `Name`, the field OCS
//!    shows as `label=` (DWG-flavour exports carry no `ItemTag`; the strict
//!    `ItemTag` count is printed beside it).
//! 2. Denominator: items with a non-empty label whose two-hop join lands on
//!    at least one in-file graphic record. Labelled items that land nowhere
//!    are counted as "two-hop broken" and kept out of the denominator.
//!    An item's anchors are its landed records plus the `DependencyObject`
//!    its `GraphicOID` names, when it names one.
//! 3. Route a (`igTextBox.parent_ref`): a text box whose `parent_ref` is one
//!    of the item's anchors is a candidate. Exactly one distinct text
//!    predicts the label; more than one is "multiple candidates".
//! 4. Route b (`DependencyObject` groups): a group's children are its tail
//!    references (aligned 4-byte windows from payload `+18`, kept when they
//!    name an oid of the same sheet's decoded pool -- the reading
//!    `PidSemanticIndex` uses) plus every record whose `parent_ref` names the
//!    group; a child that is itself a group contributes its children too
//!    (one level). A group whose expanded children hold text boxes gives
//!    those texts to every item one of whose anchors is the group or a
//!    non-text child. Exactly one distinct text over all such groups
//!    predicts the label; more than one is "multiple candidates".
//! 5. Route c (symbol path -> class): the item's `igSymbol2d` placements
//!    (direct landings first, else those reached through the dependency
//!    hop), `jsite_ref` -> `JSite<id>` -> `symbol_path` (else
//!    `local_symbol_path`), mapped by the fixed `CLASS_RULES` table (first
//!    match wins; printed in the report). Exactly one distinct class
//!    predicts it.
//! 6. Combining: label = route a, else route b; class = route c.
//! 7. Normalising: trim, collapse inner whitespace runs to one space,
//!    case-sensitive compare.
//! 8. Failure classes (one per failing item, in this order): label -- no box
//!    points at it, tag split across boxes, multiple candidates, text is
//!    not a tag (size / spec attribute text), other wrong text; class -- no
//!    symbol path, class mapping missing, several classes, wrong class;
//!    outside the denominator -- two-hop broken.
//!
//! Two diagnostics sit beside the metrics and change no score: whether the
//! answer is among routes a and b's candidates at all, and whether it is
//! drawn in any text box of the file, linked or not.
//!
//! The 80 % threshold of P-D7 reads the combined total's tag recovery rate.
//!
//! ```powershell
//! cargo run --example probe_item_tags_without_data_xml
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pid_parse::{PidDocument, PidParser, PidSemanticHit, PidSemanticIndex, SheetGeometry};

/// The answer-key drawings: publish copies with their `_Data.xml` beside.
const ANSWER_KEYS: &[(&str, &str)] = &[
    (
        "DWG-0202GP06-01",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ),
    ("A01", "test-file/export-test/publish-data/A01/A01.pid"),
];

/// P-D7: an implementation ticket is proposed only at or above this rate.
const THRESHOLD: f64 = 0.80;

/// Decoded record families of one sheet's oid pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Family {
    PrimitiveLine,
    IgLine2d,
    IgLineString2d,
    IgPoint2d,
    IgTextBox,
    IgSymbol2d,
    IgBoundary2d,
    IgSmartFrame2d,
    DependencyObject,
    JStyleOverride,
}

impl Family {
    fn name(self) -> &'static str {
        match self {
            Self::PrimitiveLine => "GLine2d",
            Self::IgLine2d => "igLine2d",
            Self::IgLineString2d => "igLineString2d",
            Self::IgPoint2d => "igPoint2d",
            Self::IgTextBox => "igTextBox",
            Self::IgSymbol2d => "igSymbol2d",
            Self::IgBoundary2d => "igBoundary2d",
            Self::IgSmartFrame2d => "igSmartFrame2d",
            Self::DependencyObject => "DependencyObject",
            Self::JStyleOverride => "JStyleOverride",
        }
    }

    /// The graphic family of the two-hop rule (S1, 2026-08-07 §1).
    fn is_graphic(self) -> bool {
        matches!(
            self,
            Self::PrimitiveLine
                | Self::IgLine2d
                | Self::IgLineString2d
                | Self::IgPoint2d
                | Self::IgTextBox
                | Self::IgSymbol2d
        )
    }
}

/// A condition on a symbol library path, compared case-insensitively.
#[derive(Clone, Copy)]
enum Cond {
    /// Some directory component below `Symbols` equals this.
    Dir(&'static str),
    /// The first directory component below `Symbols` equals this.
    FirstDir(&'static str),
    /// The file name (without `.sym`) contains this.
    FileHas(&'static str),
    /// The file name (without `.sym`) equals this.
    FileIs(&'static str),
}

impl Cond {
    fn describe(self) -> String {
        match self {
            Self::Dir(d) => format!("a directory is `{d}`"),
            Self::FirstDir(d) => format!("first directory is `{d}`"),
            Self::FileHas(f) => format!("file name contains `{f}`"),
            Self::FileIs(f) => format!("file name is `{f}`"),
        }
    }
}

/// Route c's fixed mapping, written from the catalog's directory names and
/// the answer key's class vocabulary before any score was read. First match
/// wins; `None` means "this path names no item class".
const CLASS_RULES: &[(Cond, Option<&str>)] = &[
    (Cond::Dir("nozzles"), Some("PIDNozzle")),
    (Cond::Dir("vessels"), Some("PIDProcessVessel")),
    (Cond::FirstDir("equipment"), Some("PIDEquipment")),
    (
        Cond::Dir("system functions"),
        Some("PIDControlSystemFunction"),
    ),
    (Cond::FirstDir("instrumentation"), Some("PIDInstrument")),
    (Cond::Dir("piping opc's"), Some("PIDOPC")),
    (Cond::Dir("valves"), Some("PIDPipingComponent")),
    (Cond::Dir("fittings"), Some("PIDPipingComponent")),
    (Cond::Dir("piping components"), Some("PIDPipingComponent")),
    (Cond::FileHas("note"), Some("PIDNote")),
    (Cond::Dir("labels"), None),
    (Cond::Dir("annotation"), None),
    (Cond::FileIs("drawing description"), Some("PIDDrawing")),
];

/// `(directories below Symbols, file stem)`, lower-cased.
fn split_symbol_path(path: &str) -> (Vec<String>, String) {
    let parts: Vec<String> = path
        .split(['\\', '/'])
        .filter(|p| !p.is_empty())
        .map(str::to_lowercase)
        .collect();
    let below = parts
        .iter()
        .rposition(|p| p == "symbols")
        .map_or(&parts[..], |at| &parts[at + 1..]);
    match below.split_last() {
        Some((file, dirs)) => (dirs.to_vec(), file.trim_end_matches(".sym").to_string()),
        None => (Vec::new(), String::new()),
    }
}

/// Apply `CLASS_RULES`: `(rule number, class)` of the first match.
fn class_of_path(path: &str) -> Option<(usize, Option<&'static str>)> {
    let (dirs, file) = split_symbol_path(path);
    CLASS_RULES
        .iter()
        .enumerate()
        .find_map(|(at, (cond, class))| {
            let hit = match cond {
                Cond::Dir(d) => dirs.iter().any(|x| x == d),
                Cond::FirstDir(d) => dirs.first().is_some_and(|x| x == d),
                Cond::FileHas(f) => file.contains(*f),
                Cond::FileIs(f) => file == *f,
            };
            hit.then_some((at + 1, *class))
        })
}

/// Rule 7: trim and collapse inner whitespace; case stays.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Size / rating / dimension text rather than a tag: numbers only, a
/// `DN` / `PN` / `NPS` size, a number with a unit, or a diameter sign.
fn is_attribute_text(text: &str) -> bool {
    let upper = text.to_uppercase();
    let numeric_only = text.chars().any(|c| c.is_ascii_digit())
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || " .,/-xX×".contains(c));
    let size_prefix = ["DN", "PN", "NPS"].iter().any(|p| {
        upper.match_indices(p).any(|(at, _)| {
            upper[at + p.len()..]
                .trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        })
    });
    let unit_after_number = ["MM", "\"", "INCH"].iter().any(|u| {
        upper.match_indices(u).any(|(at, _)| {
            upper[..at]
                .trim_end()
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_digit())
        })
    });
    let diameter = text.contains(['Ø', 'φ', 'Φ']);
    numeric_only || size_prefix || unit_after_number || diameter
}

/// Does an ordered pick of two or more candidates, optionally separated by
/// `-` or `/`, spell the answer (whitespace ignored)?
fn is_split_tag(candidates: &BTreeSet<String>, answer: &str) -> bool {
    fn squash(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }
    fn spell(rest: &str, pieces: &[String], used: &mut [bool], count: usize) -> bool {
        if rest.is_empty() {
            return count >= 2;
        }
        let rest = if count > 0 {
            rest.strip_prefix(['-', '/']).unwrap_or(rest)
        } else {
            rest
        };
        for (at, piece) in pieces.iter().enumerate() {
            if !used[at] {
                if let Some(tail) = rest.strip_prefix(piece.as_str()) {
                    used[at] = true;
                    if spell(tail, pieces, used, count + 1) {
                        return true;
                    }
                    used[at] = false;
                }
            }
        }
        false
    }
    let target = squash(answer);
    let pieces: Vec<String> = candidates
        .iter()
        .map(|c| squash(c))
        .filter(|p| !p.is_empty() && p.len() < target.len() && target.contains(p.as_str()))
        .take(8)
        .collect();
    let mut used = vec![false; pieces.len()];
    spell(&target, &pieces, &mut used, 0)
}

/// oid -> families, over the same families `PidSemanticIndex` pools.
fn families_of(g: &SheetGeometry) -> BTreeMap<u32, BTreeSet<Family>> {
    let mut out: BTreeMap<u32, BTreeSet<Family>> = BTreeMap::new();
    let mut add = |oid: u32, family: Family| {
        out.entry(oid).or_default().insert(family);
    };
    g.decoded_primitive_lines
        .iter()
        .for_each(|r| add(r.oid, Family::PrimitiveLine));
    g.decoded_iglines
        .iter()
        .for_each(|r| add(r.oid, Family::IgLine2d));
    g.decoded_iglinestrings
        .iter()
        .for_each(|r| add(r.oid, Family::IgLineString2d));
    g.decoded_igpoints
        .iter()
        .for_each(|r| add(r.oid, Family::IgPoint2d));
    g.decoded_igtextboxes
        .iter()
        .for_each(|r| add(r.oid, Family::IgTextBox));
    g.decoded_igsymbols
        .iter()
        .for_each(|r| add(r.oid, Family::IgSymbol2d));
    g.decoded_igboundaries
        .iter()
        .for_each(|r| add(r.oid, Family::IgBoundary2d));
    g.decoded_igsmartframes
        .iter()
        .for_each(|r| add(r.oid, Family::IgSmartFrame2d));
    g.decoded_dependency_objects
        .iter()
        .for_each(|r| add(r.oid, Family::DependencyObject));
    g.decoded_jstyle_overrides
        .iter()
        .for_each(|r| add(r.oid, Family::JStyleOverride));
    out
}

/// A group's tail references: aligned 4-byte windows naming a pool oid.
fn tail_references(tail: &[u8], own: u32, pool: &BTreeMap<u32, BTreeSet<Family>>) -> Vec<u32> {
    (0..tail.len().saturating_sub(3))
        .step_by(4)
        .map(|at| u32::from_le_bytes([tail[at], tail[at + 1], tail[at + 2], tail[at + 3]]))
        .filter(|v| *v != 0 && *v != own && pool.contains_key(v))
        .collect()
}

/// `JSite<id>` -> symbol library path, as the geometry emitter reads it.
fn jsite_symbol_paths(doc: &PidDocument) -> BTreeMap<u32, String> {
    doc.jsites
        .iter()
        .filter_map(|site| {
            let id: u32 = site.name.strip_prefix("JSite")?.parse().ok()?;
            let path = site
                .symbol_path
                .as_deref()
                .or(site.local_symbol_path.as_deref())?;
            Some((id, path.to_string()))
        })
        .collect()
}

/// One sheet's route inputs, all read from the `.pid`.
struct SheetInputs {
    path: String,
    families: BTreeMap<u32, BTreeSet<Family>>,
    /// Text box oid -> normalized non-empty texts (an oid can repeat).
    texts: BTreeMap<u32, Vec<String>>,
    /// Every text box: `(oid, parent_ref, normalized text)`.
    text_boxes: Vec<(u32, u32, String)>,
    /// Group oid -> children (tail references and `parent_ref` children).
    groups: BTreeMap<u32, BTreeSet<u32>>,
    /// Symbol placement oid -> library path, when its `JSite` names one.
    symbols: BTreeMap<u32, Option<String>>,
}

impl SheetInputs {
    fn read(path: &str, g: &SheetGeometry, jsite_paths: &BTreeMap<u32, String>) -> Self {
        let families = families_of(g);
        let mut texts: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        let mut text_boxes = Vec::new();
        for t in &g.decoded_igtextboxes {
            let text = normalize(&t.text);
            if !text.is_empty() {
                texts.entry(t.oid).or_default().push(text.clone());
            }
            text_boxes.push((t.oid, t.parent_ref, text));
        }
        let mut groups: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for d in &g.decoded_dependency_objects {
            groups.entry(d.oid).or_default().extend(tail_references(
                &d.raw_reference_payload,
                d.oid,
                &families,
            ));
        }
        let parented = g
            .decoded_iglines
            .iter()
            .map(|r| (r.oid, r.parent_ref))
            .chain(
                g.decoded_iglinestrings
                    .iter()
                    .map(|r| (r.oid, r.parent_ref)),
            )
            .chain(g.decoded_igpoints.iter().map(|r| (r.oid, r.parent_ref)))
            .chain(g.decoded_igtextboxes.iter().map(|r| (r.oid, r.parent_ref)))
            .chain(g.decoded_igsymbols.iter().map(|r| (r.oid, r.parent_ref)));
        for (child, parent) in parented {
            if parent != 0 && child != parent {
                if let Some(children) = groups.get_mut(&parent) {
                    children.insert(child);
                }
            }
        }
        let symbols = g
            .decoded_igsymbols
            .iter()
            .map(|s| (s.oid, jsite_paths.get(&s.jsite_ref).cloned()))
            .collect();
        Self {
            path: path.to_string(),
            families,
            texts,
            text_boxes,
            groups,
            symbols,
        }
    }

    fn is_text(&self, oid: u32) -> bool {
        self.families
            .get(&oid)
            .is_some_and(|f| f.contains(&Family::IgTextBox))
    }

    fn family_label(&self, oid: u32) -> String {
        self.families.get(&oid).map_or_else(
            || "(not in pool)".to_string(),
            |set| set.iter().map(|f| f.name()).collect::<Vec<_>>().join("+"),
        )
    }

    /// Route b: a group's children plus its child groups' children.
    fn expanded(&self, group: u32) -> BTreeSet<u32> {
        let mut out = self.groups.get(&group).cloned().unwrap_or_default();
        let nested: Vec<u32> = out
            .iter()
            .copied()
            .filter(|c| self.groups.contains_key(c))
            .collect();
        for child in nested {
            out.extend(self.groups[&child].iter().copied());
        }
        out.remove(&group);
        out
    }
}

/// `(sheet index, oid)`.
type Key = (usize, u32);

/// One answer-key item and what each route said about it.
struct Item {
    graphic_oid: u32,
    class: String,
    label: String,
    has_item_tag: bool,
    direct: BTreeSet<Key>,
    via: BTreeSet<Key>,
    aggregate: BTreeSet<Key>,
    a_texts: BTreeSet<String>,
    b_texts: BTreeSet<String>,
    b_groups: BTreeSet<Key>,
    symbol_paths: Vec<String>,
    classes: BTreeSet<&'static str>,
}

impl Item {
    fn lands(&self) -> BTreeSet<Key> {
        self.direct.union(&self.via).copied().collect()
    }

    fn anchors(&self) -> BTreeSet<Key> {
        let mut out = self.lands();
        out.extend(self.aggregate.iter().copied());
        out
    }

    fn only(set: &BTreeSet<String>) -> Option<&str> {
        if set.len() == 1 {
            set.iter().next().map(String::as_str)
        } else {
            None
        }
    }

    fn route_a(&self) -> Option<&str> {
        Self::only(&self.a_texts)
    }

    fn route_b(&self) -> Option<&str> {
        Self::only(&self.b_texts)
    }

    fn label_guess(&self) -> Option<&str> {
        self.route_a().or(self.route_b())
    }

    fn class_guess(&self) -> Option<&'static str> {
        if self.classes.len() == 1 {
            self.classes.iter().next().copied()
        } else {
            None
        }
    }

    fn candidates(&self) -> BTreeSet<String> {
        self.a_texts.union(&self.b_texts).cloned().collect()
    }

    fn label_failure(&self) -> Option<&'static str> {
        let guess = self.label_guess();
        if guess == Some(self.label.as_str()) {
            return None;
        }
        let candidates = self.candidates();
        Some(if candidates.is_empty() {
            "label: no box points at it"
        } else if is_split_tag(&candidates, &self.label) {
            "label: tag split across boxes"
        } else if guess.is_none() {
            "label: multiple candidates"
        } else if guess.is_some_and(is_attribute_text) {
            "label: text is not a tag (size/spec)"
        } else {
            "label: other wrong text"
        })
    }

    fn class_failure(&self) -> Option<&'static str> {
        if self.class_guess() == Some(self.class.as_str()) {
            return None;
        }
        Some(if self.symbol_paths.is_empty() {
            "class: no symbol path"
        } else if self.classes.is_empty() {
            "class: class mapping missing"
        } else if self.classes.len() > 1 {
            "class: several classes"
        } else {
            "class: wrong class"
        })
    }

    fn describe(&self) -> String {
        let show = |set: &BTreeSet<String>| {
            let mut v: Vec<String> = set.iter().take(4).map(|s| format!("{s:?}")).collect();
            if set.len() > 4 {
                v.push(format!("…+{}", set.len() - 4));
            }
            format!("{{{}}}", v.join(", "))
        };
        format!(
            "g={} {} key={:?} a={} b={} c={:?}",
            self.graphic_oid,
            self.class,
            self.label,
            show(&self.a_texts),
            show(&self.b_texts),
            self.class_guess().unwrap_or("-"),
        )
    }
}

/// Counters for one row of the report (a drawing, a class, or the total).
#[derive(Default, Clone)]
struct Tally {
    denom: usize,
    a_pred: usize,
    a_ok: usize,
    a_multi: usize,
    b_pred: usize,
    b_ok: usize,
    b_multi: usize,
    tag_ok: usize,
    c_pred: usize,
    c_ok: usize,
    oracle: usize,
}

impl Tally {
    fn count(&mut self, item: &Item) {
        self.denom += 1;
        if let Some(a) = item.route_a() {
            self.a_pred += 1;
            self.a_ok += usize::from(a == item.label);
        } else if item.a_texts.len() > 1 {
            self.a_multi += 1;
        }
        if let Some(b) = item.route_b() {
            self.b_pred += 1;
            self.b_ok += usize::from(b == item.label);
        } else if item.b_texts.len() > 1 {
            self.b_multi += 1;
        }
        self.tag_ok += usize::from(item.label_guess() == Some(item.label.as_str()));
        if let Some(c) = item.class_guess() {
            self.c_pred += 1;
            self.c_ok += usize::from(c == item.class);
        }
        self.oracle += usize::from(item.candidates().contains(&item.label));
    }

    fn add(&mut self, other: &Tally) {
        self.denom += other.denom;
        self.a_pred += other.a_pred;
        self.a_ok += other.a_ok;
        self.a_multi += other.a_multi;
        self.b_pred += other.b_pred;
        self.b_ok += other.b_ok;
        self.b_multi += other.b_multi;
        self.tag_ok += other.tag_ok;
        self.c_pred += other.c_pred;
        self.c_ok += other.c_ok;
        self.oracle += other.oracle;
    }
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 {
        "n/a".to_string()
    } else {
        format!("{:.1} %", 100.0 * n as f64 / d as f64)
    }
}

fn frac(n: usize, d: usize) -> String {
    format!("{n}/{d}")
}

fn ratio(n: usize, d: usize) -> String {
    if d == 0 {
        "n/a".to_string()
    } else {
        format!("{:.4}", n as f64 / d as f64)
    }
}

/// Everything one drawing contributes to the combined total.
#[derive(Default)]
struct Report {
    tally: Tally,
    per_class: BTreeMap<String, Tally>,
    failures: BTreeMap<&'static str, (usize, String)>,
    published: usize,
    labelled: usize,
    strict_item_tag: usize,
    strict_item_tag_denom: usize,
    /// Diagnostic only: the answer is some text box's whole text.
    drawn_exact: usize,
    /// Diagnostic only: some text box's text is a part of the answer.
    drawn_fragment: usize,
    drawn_example: Option<String>,
}

impl Report {
    fn fail(&mut self, class: &'static str, example: String) {
        let entry = self.failures.entry(class).or_insert((0, example));
        entry.0 += 1;
    }

    fn absorb(&mut self, other: &Report) {
        self.tally.add(&other.tally);
        for (class, tally) in &other.per_class {
            self.per_class.entry(class.clone()).or_default().add(tally);
        }
        for (class, (n, example)) in &other.failures {
            let entry = self.failures.entry(*class).or_insert((0, example.clone()));
            entry.0 += n;
        }
        self.published += other.published;
        self.labelled += other.labelled;
        self.strict_item_tag += other.strict_item_tag;
        self.strict_item_tag_denom += other.strict_item_tag_denom;
        self.drawn_exact += other.drawn_exact;
        self.drawn_fragment += other.drawn_fragment;
        if self.drawn_example.is_none() {
            self.drawn_example.clone_from(&other.drawn_example);
        }
    }
}

/// Failure classes in report order.
const FAILURE_ORDER: &[&str] = &[
    "label: no box points at it",
    "label: tag split across boxes",
    "label: multiple candidates",
    "label: text is not a tag (size/spec)",
    "label: other wrong text",
    "class: no symbol path",
    "class: class mapping missing",
    "class: several classes",
    "class: wrong class",
    "two-hop broken (outside denominator)",
];

fn print_rules() {
    println!("route c class-mapping rules (fixed before scoring; first match wins):");
    for (at, (cond, class)) in CLASS_RULES.iter().enumerate() {
        println!(
            "  #{:<2} {:<42} -> {}",
            at + 1,
            cond.describe(),
            class.unwrap_or("(no item class)")
        );
    }
    println!("  (no rule matches)                           -> (no item class)");
    println!("  directories are the path components below `Symbols`; case-insensitive");
    println!();
}

fn print_metrics(name: &str, report: &Report) {
    let t = &report.tally;
    println!("metrics ({name}):");
    println!(
        "  denominator                 {} (labelled items that land in the file)",
        t.denom
    );
    println!(
        "  route a parent_ref          coverage {}/{} = {}   accuracy {}/{} = {}   multiple candidates {}",
        t.a_pred,
        t.denom,
        pct(t.a_pred, t.denom),
        t.a_ok,
        t.a_pred,
        pct(t.a_ok, t.a_pred),
        t.a_multi
    );
    println!(
        "  route b DependencyObject    coverage {}/{} = {}   accuracy {}/{} = {}   multiple candidates {}",
        t.b_pred,
        t.denom,
        pct(t.b_pred, t.denom),
        t.b_ok,
        t.b_pred,
        pct(t.b_ok, t.b_pred),
        t.b_multi
    );
    println!(
        "  route c symbol path->class  coverage {}/{} = {}   accuracy {}/{} = {}",
        t.c_pred,
        t.denom,
        pct(t.c_pred, t.denom),
        t.c_ok,
        t.c_pred,
        pct(t.c_ok, t.c_pred)
    );
    println!(
        "  TAG RECOVERY RATE (a, else b)   {}/{} = {}",
        t.tag_ok,
        t.denom,
        pct(t.tag_ok, t.denom)
    );
    println!(
        "  CLASS RECOVERY RATE (c)         {}/{} = {}",
        t.c_ok,
        t.denom,
        pct(t.c_ok, t.denom)
    );
    println!(
        "  diagnostic: answer among a∪b candidates {}/{} = {} (a ceiling for a better pick, not a route)",
        t.oracle,
        t.denom,
        pct(t.oracle, t.denom)
    );
    println!(
        "  diagnostic: answer drawn anywhere in the file (no link needed): whole text of a box {}/{}, only as box fragments {}/{}{}",
        report.drawn_exact,
        t.denom,
        report.drawn_fragment,
        t.denom,
        report
            .drawn_example
            .as_deref()
            .map_or_else(String::new, |e| format!("   e.g. {e}"))
    );
    println!("  per answer-key class:");
    println!(
        "    {:<26} {:>5} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "class", "denom", "a cov", "a acc", "b cov", "b acc", "c cov", "c acc", "tag", "class"
    );
    for (class, c) in &report.per_class {
        println!(
            "    {:<26} {:>5} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
            class,
            c.denom,
            frac(c.a_pred, c.denom),
            frac(c.a_ok, c.a_pred),
            frac(c.b_pred, c.denom),
            frac(c.b_ok, c.b_pred),
            frac(c.c_pred, c.denom),
            frac(c.c_ok, c.c_pred),
            frac(c.tag_ok, c.denom),
            frac(c.c_ok, c.denom),
        );
    }
    println!("  failure classes (count, one example):");
    for class in FAILURE_ORDER {
        match report.failures.get(class) {
            Some((n, example)) => println!("    {class:<38} {n:>3}   e.g. {example}"),
            None => println!("    {class:<38} {:>3}", 0),
        }
    }
}

fn summary(name: &str, report: &Report) {
    let t = &report.tally;
    println!(
        "SUMMARY file={name} published={} labelled={} strict_itemtag_denom={} denom={} two_hop_broken={} a_cov={} a_ok={} b_cov={} b_ok={} tag_ok={} tag_rate={} class_ok={} class_rate={}",
        report.published,
        report.labelled,
        report.strict_item_tag_denom,
        t.denom,
        report
            .failures
            .get("two-hop broken (outside denominator)")
            .map_or(0, |f| f.0),
        t.a_pred,
        t.a_ok,
        t.b_pred,
        t.b_ok,
        t.tag_ok,
        ratio(t.tag_ok, t.denom),
        t.c_ok,
        ratio(t.c_ok, t.denom),
    );
}

/// Score one answer-key drawing; `None` when it has no `_Data.xml`.
fn score(name: &str, file: &str) -> Option<Report> {
    let path = Path::new(file);
    println!("==== {name}  ({file})");
    if !path.exists() {
        println!("  skip: not on this machine");
        return None;
    }
    let doc = PidParser::new().parse_file(path).expect("fixture parses");
    let Some(index) = PidSemanticIndex::load_beside(path, &doc) else {
        println!("  skip: no readable _Data.xml beside it");
        return None;
    };
    let jsite_paths = jsite_symbol_paths(&doc);
    let sheets: Vec<SheetInputs> = doc
        .sheet_streams
        .iter()
        .filter_map(|s| {
            s.geometry
                .as_ref()
                .map(|g| SheetInputs::read(&s.path, g, &jsite_paths))
        })
        .collect();

    // --- route inputs (the .pid only) ---------------------------------
    println!("route inputs (read from the .pid only):");
    for sheet in &sheets {
        let mut targets: BTreeMap<String, usize> = BTreeMap::new();
        for (_, parent, _) in &sheet.text_boxes {
            let label = if *parent == 0 {
                "(zero)".to_string()
            } else {
                sheet.family_label(*parent)
            };
            *targets.entry(label).or_default() += 1;
        }
        let with_text = sheet
            .groups
            .keys()
            .filter(|g| sheet.expanded(**g).iter().any(|c| sheet.is_text(*c)))
            .count();
        let resolved = sheet.symbols.values().filter(|p| p.is_some()).count();
        println!(
            "  {}: pool {} oids, text boxes {} (parent_ref -> {:?}), groups {} ({} hold text after expansion), symbols {} ({} with a library path)",
            sheet.path,
            sheet.families.len(),
            sheet.text_boxes.len(),
            targets,
            sheet.groups.len(),
            with_text,
            sheet.symbols.len(),
            resolved
        );
    }

    // --- answer key (the _Data.xml, for scoring only) -----------------
    let mut items: BTreeMap<u32, Item> = index
        .objects()
        .map(|o| {
            let label = normalize(o.label().unwrap_or(""));
            let has_item_tag = o
                .item_tag
                .as_deref()
                .is_some_and(|t| !normalize(t).is_empty());
            (
                o.graphic_oid,
                Item {
                    graphic_oid: o.graphic_oid,
                    class: o.class.clone(),
                    label,
                    has_item_tag,
                    direct: BTreeSet::new(),
                    via: BTreeSet::new(),
                    aggregate: BTreeSet::new(),
                    a_texts: BTreeSet::new(),
                    b_texts: BTreeSet::new(),
                    b_groups: BTreeSet::new(),
                    symbol_paths: Vec::new(),
                    classes: BTreeSet::new(),
                },
            )
        })
        .collect();
    for (at, sheet) in sheets.iter().enumerate() {
        for (oid, families) in &sheet.families {
            if !families.iter().any(|f| f.is_graphic()) {
                continue;
            }
            match index.resolve(*oid) {
                Some(PidSemanticHit::Direct(o)) => {
                    if let Some(item) = items.get_mut(&o.graphic_oid) {
                        item.direct.insert((at, *oid));
                    }
                }
                Some(PidSemanticHit::ViaDependency { object, .. }) => {
                    if let Some(item) = items.get_mut(&object.graphic_oid) {
                        item.via.insert((at, *oid));
                    }
                }
                None => {}
            }
        }
        for item in items.values_mut() {
            if sheet.groups.contains_key(&item.graphic_oid) {
                item.aggregate.insert((at, item.graphic_oid));
            }
        }
    }

    // --- routes ---------------------------------------------------------
    for item in items.values_mut() {
        let anchors = item.anchors();
        // Route a: a text box's parent_ref is one of the item's anchors.
        for (at, sheet) in sheets.iter().enumerate() {
            for (_, parent, text) in &sheet.text_boxes {
                if *parent != 0 && !text.is_empty() && anchors.contains(&(at, *parent)) {
                    item.a_texts.insert(text.clone());
                }
            }
        }
        // Route b: groups whose expanded children hold text and link the item.
        for (at, sheet) in sheets.iter().enumerate() {
            for group in sheet.groups.keys() {
                let expanded = sheet.expanded(*group);
                let texts: BTreeSet<String> = expanded
                    .iter()
                    .filter(|c| sheet.is_text(**c))
                    .flat_map(|c| sheet.texts.get(c).into_iter().flatten().cloned())
                    .collect();
                if texts.is_empty() {
                    continue;
                }
                let links = std::iter::once(*group)
                    .chain(expanded.iter().copied().filter(|c| !sheet.is_text(*c)))
                    .any(|c| anchors.contains(&(at, c)));
                if links {
                    item.b_texts.extend(texts);
                    item.b_groups.insert((at, *group));
                }
            }
        }
        // Route c: the item's own placements first, else the hop's.
        let symbols_of = |keys: &BTreeSet<Key>| -> Vec<String> {
            keys.iter()
                .filter_map(|(at, oid)| sheets[*at].symbols.get(oid).cloned().flatten())
                .collect()
        };
        let mut paths = symbols_of(&item.direct);
        if paths.is_empty() {
            paths = symbols_of(&item.via);
        }
        item.classes = paths
            .iter()
            .filter_map(|p| class_of_path(p).and_then(|(_, class)| class))
            .collect();
        item.symbol_paths = paths;
    }

    // --- score ----------------------------------------------------------
    let mut report = Report {
        published: items.len(),
        ..Report::default()
    };
    let mut hop_counts: BTreeMap<String, usize> = BTreeMap::new();
    let all_texts: BTreeSet<String> = sheets
        .iter()
        .flat_map(|s| s.texts.values().flatten().cloned())
        .collect();
    println!("items (answer key vs. routes):");
    for item in items.values() {
        if item.label.is_empty() {
            continue;
        }
        report.labelled += 1;
        report.strict_item_tag += usize::from(item.has_item_tag);
        let lands = item.lands();
        if lands.is_empty() {
            report.fail(
                "two-hop broken (outside denominator)",
                format!(
                    "g={} {} key={:?} lands on no in-file graphic record",
                    item.graphic_oid, item.class, item.label
                ),
            );
            println!("  BROKEN  {}", item.describe());
            continue;
        }
        report.strict_item_tag_denom += usize::from(item.has_item_tag);
        // Diagnostic only (no route, no score): is the answer drawn at all?
        if all_texts.contains(&item.label) {
            report.drawn_exact += 1;
        } else {
            let fragments: Vec<&String> = all_texts
                .iter()
                .filter(|t| t.chars().count() >= 3 && item.label.contains(t.as_str()))
                .collect();
            if !fragments.is_empty() {
                report.drawn_fragment += 1;
                if report.drawn_example.is_none() {
                    report.drawn_example = Some(format!(
                        "g={} key={:?} fragments {:?}",
                        item.graphic_oid, item.label, fragments
                    ));
                }
            }
        }
        for (at, oid) in &item.direct {
            *hop_counts
                .entry(format!("direct {}", sheets[*at].family_label(*oid)))
                .or_default() += 1;
        }
        for (at, oid) in &item.via {
            *hop_counts
                .entry(format!("via dependency {}", sheets[*at].family_label(*oid)))
                .or_default() += 1;
        }
        report.tally.count(item);
        report
            .per_class
            .entry(item.class.clone())
            .or_default()
            .count(item);
        let label_failure = item.label_failure();
        let class_failure = item.class_failure();
        if let Some(f) = label_failure {
            report.fail(f, item.describe());
        }
        if let Some(f) = class_failure {
            let paths: Vec<String> = item
                .symbol_paths
                .iter()
                .map(|p| {
                    let (dirs, file) = split_symbol_path(p);
                    format!("{}\\{}", dirs.join("\\"), file)
                })
                .collect();
            report.fail(f, format!("{} paths={paths:?}", item.describe()));
        }
        println!(
            "  {:<5} {:<5} {}  groups={}",
            if label_failure.is_none() {
                "TAG"
            } else {
                "tag-"
            },
            if class_failure.is_none() {
                "CLASS"
            } else {
                "cls-"
            },
            item.describe(),
            item.b_groups.len()
        );
    }
    println!(
        "answer key: {} published objects, {} with a label ({} with a strict ItemTag), {} land in the file; landed records by hop: {:?}",
        report.published,
        report.labelled,
        report.strict_item_tag,
        report.tally.denom,
        hop_counts
    );
    print_metrics(name, &report);
    summary(name, &report);
    println!();
    Some(report)
}

fn main() {
    println!("S1 probe: item tags and classes from in-file links only (_Data.xml scores only)");
    println!();
    print_rules();
    let mut total = Report::default();
    let mut scored = Vec::new();
    for (name, file) in ANSWER_KEYS {
        if let Some(report) = score(name, file) {
            total.absorb(&report);
            scored.push(*name);
        }
    }
    println!("==== combined total ({})", scored.join(" + "));
    print_metrics("combined", &total);
    summary("TOTAL", &total);
    let rate = if total.tally.denom == 0 {
        0.0
    } else {
        total.tally.tag_ok as f64 / total.tally.denom as f64
    };
    println!(
        "threshold (P-D7): combined tag recovery rate {} is {} {:.0} %",
        pct(total.tally.tag_ok, total.tally.denom),
        if rate >= THRESHOLD {
            "at or above"
        } else {
            "below"
        },
        THRESHOLD * 100.0
    );
}
