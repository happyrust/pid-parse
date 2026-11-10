//! Which primitives of a page leave ink, which only paint the paper's own
//! colour — and which of them does another build of this crate print the
//! other way?
//!
//! A page prints black on white paper: the writer inks every colour a pen
//! states in black and keeps exact white white, so a stroke whose model
//! colour is `[255, 255, 255]` is on the page and shows nowhere. Which
//! strokes those are is the layout's decision, and a change to that decision
//! moves no geometry: no golden's coordinates, no ledger count and no
//! colour-blind comparison sees it. This reads the seventy-one pages this
//! crate exports — the eleven sheets with a vendor plot, the fifty-three of
//! the plant backup and the seven committed fixtures, each laid out the way
//! its own gate lays it out — and says, primitive by primitive, what prints.
//!
//! A primitive is *ink* when the writer puts it on the page
//! (`write::write_ledger`) in any colour but white, *paper* when it is on the
//! page in white, and *off* when the writer leaves it off the page. A label
//! is always ink: lettering has no pen. Every primitive is filed under what
//! drew it — a border template's own rule, a symbol the template places, a
//! symbol the drawing places, or the drawing's own record.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_primitives_paint_the_paper -- \
//!     [--dump <file>] [--against <file>]
//! ```
//!
//! With no argument: one line a page (what is paper, by what drew it) and the
//! totals of each batch. `--dump` also writes one line per primitive to a
//! file. `--against` reads such a file written by another build and lists,
//! page by page, the primitives that print the other way now — how many, how
//! long, and what drew them — after checking that nothing else about the
//! page moved: the same primitives in the same order with the same shapes. It
//! ends with the kinds of change no page with a vendor plot carries, which
//! are the ones no reference can speak for.
//!
//! Exit code 1 when a drawing does not lay out, or when `--against` finds a
//! page whose primitives differ in more than what they print.
//!
//! The corpora are found as the gates find them (`PID_ACCEPTANCE_CORPUS`,
//! `PID_TEST_DATA_DIR`, `PID_PARSE_SELF_CHECK_DIR`); a batch whose corpus is
//! absent is named and left out. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;
#[path = "support/batches.rs"]
mod batches;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::process::ExitCode;

use pid_plot::model::{PaperModel, PaperPrimitive, PaperShape};
use pid_plot::write::write_ledger;

use acceptance::strokes_of;
use batches::{batches, leaf};

/// The one colour the writer does not ink.
const WHITE: [u8; 3] = [255, 255, 255];

/// What a primitive does on the written page.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prints {
    Ink,
    Paper,
    Off,
}

impl Prints {
    fn name(self) -> &'static str {
        match self {
            Self::Ink => "ink",
            Self::Paper => "paper",
            Self::Off => "off",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "ink" => Some(Self::Ink),
            "paper" => Some(Self::Paper),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/// One primitive as a dump line states it.
struct Row {
    kind: &'static str,
    prints: Prints,
    /// Millimetres of stroke or ring; characters of a label.
    measure: f64,
    /// What drew it.
    class: String,
    /// The colour the model states, `r,g,b`, or `-` for a label and for a
    /// stroke or fill that states none.
    stated: String,
    /// Where it is: enough of its shape to tell that two builds drew the
    /// same thing.
    locus: String,
}

fn kind_of(shape: &PaperShape) -> &'static str {
    match shape {
        PaperShape::Line { .. } => "line",
        PaperShape::Arc { .. } => "arc",
        PaperShape::Circle { .. } => "circle",
        PaperShape::Fill { .. } => "fill",
        PaperShape::Polyline { .. } => "polyline",
        PaperShape::Text { .. } => "text",
    }
}

/// What drew a primitive.
fn class_of(primitive: &PaperPrimitive, model: &PaperModel) -> String {
    let template = model
        .template
        .as_ref()
        .map_or("template", |template| leaf(&template.path));
    match (&primitive.symbol, primitive.furniture) {
        (Some(symbol), true) => format!("template symbol: {} in {template}", leaf(symbol)),
        (None, true) => format!("template rule: {template}"),
        (Some(symbol), false) => format!("symbol: {symbol}"),
        (None, false) => "sheet: the drawing's own record".to_string(),
    }
}

fn point(at: [f64; 2]) -> String {
    format!("{:.3},{:.3}", at[0], at[1])
}

fn row_of(primitive: &PaperPrimitive, model: &PaperModel, on_page: bool) -> Row {
    let kind = kind_of(&primitive.shape);
    let (colour, measure, locus) = match &primitive.shape {
        PaperShape::Text {
            origin_mm, value, ..
        } => (
            None,
            value.chars().filter(|ch| !ch.is_whitespace()).count() as f64,
            point(*origin_mm),
        ),
        PaperShape::Fill { chain_mm, rgb } => {
            let ring: f64 = strokes_of(&primitive.shape)
                .iter()
                .flat_map(|chain| chain.windows(2))
                .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
                .sum();
            (
                *rgb,
                ring,
                chain_mm
                    .first()
                    .map_or_else(String::new, |first| point(*first)),
            )
        }
        shape => {
            let chains = strokes_of(shape);
            let length: f64 = chains
                .iter()
                .flat_map(|chain| chain.windows(2))
                .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
                .sum();
            let first = chains
                .first()
                .and_then(|chain| chain.first())
                .map_or_else(String::new, |first| point(*first));
            (primitive.stroke.as_ref().map(|pen| pen.rgb), length, first)
        }
    };
    let prints = if !on_page {
        Prints::Off
    } else if colour == Some(WHITE) {
        Prints::Paper
    } else {
        Prints::Ink
    };
    Row {
        kind,
        prints,
        measure,
        class: class_of(primitive, model),
        stated: colour.map_or_else(
            || "-".to_string(),
            |rgb| format!("{},{},{}", rgb[0], rgb[1], rgb[2]),
        ),
        locus,
    }
}

/// A count and a length (or a count of characters, for labels).
#[derive(Clone, Copy, Default)]
struct Tally {
    primitives: usize,
    measure: f64,
}

impl Tally {
    fn add(&mut self, measure: f64) {
        self.primitives += 1;
        self.measure += measure;
    }
}

/// What a dump line of another build said about one primitive.
struct Before {
    kind: String,
    prints: Prints,
    measure: f64,
    locus: String,
}

type Key = (String, String, usize);

fn read_dump(path: &str) -> Result<HashMap<Key, Before>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let mut rows = HashMap::new();
    for (number, line) in text.lines().enumerate().skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [
            batch,
            page,
            index,
            kind,
            prints,
            measure,
            _class,
            _stated,
            locus,
        ] = fields[..]
        else {
            return Err(format!("{path}:{}: not a dump line", number + 1));
        };
        let (Ok(index), Some(prints), Ok(measure)) = (
            index.parse::<usize>(),
            Prints::parse(prints),
            measure.parse::<f64>(),
        ) else {
            return Err(format!("{path}:{}: not a dump line", number + 1));
        };
        rows.insert(
            (batch.to_string(), page.to_string(), index),
            Before {
                kind: kind.to_string(),
                prints,
                measure,
                locus: locus.to_string(),
            },
        );
    }
    Ok(rows)
}

fn main() -> ExitCode {
    let mut dump_to: Option<String> = None;
    let mut against: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dump" => dump_to = args.next(),
            "--against" => against = args.next(),
            other => {
                eprintln!("{other}: expected --dump <file> or --against <file>");
                return ExitCode::from(2);
            }
        }
    }
    let before = match against.as_deref().map(read_dump) {
        Some(Ok(rows)) => Some(rows),
        Some(Err(error)) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
        None => None,
    };
    let batches = match batches() {
        Ok(batches) => batches,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };

    let mut dump =
        String::from("batch\tpage\tindex\tkind\tprints\tmeasure\tclass\tstated\tlocus\n");
    let mut moved_pages = 0usize;
    // (direction, class) -> (batch -> tally); and the pages each occurs on.
    let mut changes: BTreeMap<(&'static str, String), BTreeMap<&'static str, Tally>> =
        BTreeMap::new();
    let mut change_pages: BTreeMap<(&'static str, String), BTreeSet<String>> = BTreeMap::new();
    for batch in &batches {
        println!(
            "== {} — {} page(s) against {}",
            batch.name,
            batch.sheets.len(),
            batch.corpus.display()
        );
        let mut batch_paper: BTreeMap<String, Tally> = BTreeMap::new();
        let mut batch_ink = [Tally::default(); 2];
        let mut batch_off = Tally::default();
        let mut batch_changed: BTreeMap<&'static str, Tally> = BTreeMap::new();
        for sheet in &batch.sheets {
            let fates = match write_ledger(&sheet.model) {
                Ok(ledger) => ledger.fates,
                Err(error) => {
                    eprintln!("{}: {error}", sheet.key);
                    return ExitCode::from(1);
                }
            };
            let mut paper: BTreeMap<String, Tally> = BTreeMap::new();
            let mut ink = [Tally::default(); 2];
            let mut off = Tally::default();
            let mut page_changes: BTreeMap<(&'static str, String), Tally> = BTreeMap::new();
            let mut moved: Vec<String> = Vec::new();
            for (index, primitive) in sheet.model.primitives.iter().enumerate() {
                let on_page = fates.get(index).is_some_and(|fate| fate.is_on_page());
                let row = row_of(primitive, &sheet.model, on_page);
                let _ = writeln!(
                    dump,
                    "{}\t{}\t{index}\t{}\t{}\t{:.3}\t{}\t{}\t{}",
                    batch.name,
                    sheet.key,
                    row.kind,
                    row.prints.name(),
                    row.measure,
                    row.class,
                    row.stated,
                    row.locus
                );
                match row.prints {
                    Prints::Ink => ink[usize::from(row.kind == "text")].add(row.measure),
                    Prints::Paper => paper.entry(row.class.clone()).or_default().add(row.measure),
                    Prints::Off => off.add(row.measure),
                }
                let Some(before) = &before else {
                    continue;
                };
                let key = (batch.name.to_string(), sheet.key.clone(), index);
                match before.get(&key) {
                    None => moved.push(format!("primitive {index} is new")),
                    Some(was)
                        if was.kind != row.kind
                            || was.locus != row.locus
                            || (was.measure - row.measure).abs() > 5e-4 =>
                    {
                        moved.push(format!(
                            "primitive {index} was a {} of {:.3} at {}, is a {} of {:.3} at {}",
                            was.kind, was.measure, was.locus, row.kind, row.measure, row.locus
                        ));
                    }
                    Some(was) if was.prints != row.prints => {
                        let direction = match (was.prints, row.prints) {
                            (Prints::Paper, Prints::Ink) => "paper → ink",
                            (Prints::Ink, Prints::Paper) => "ink → paper",
                            (Prints::Off, _) => "off → on the page",
                            (_, Prints::Off) => "on the page → off",
                            _ => "?",
                        };
                        page_changes
                            .entry((direction, row.class.clone()))
                            .or_default()
                            .add(row.measure);
                    }
                    Some(_) => {}
                }
            }
            if let Some(before) = &before {
                let had = before
                    .keys()
                    .filter(|(name, page, _)| name == batch.name && *page == sheet.key)
                    .count();
                if had != sheet.model.primitives.len() {
                    moved.push(format!(
                        "{had} primitive(s) before, {} now",
                        sheet.model.primitives.len()
                    ));
                }
            }
            let paper_total: Tally = paper.values().fold(Tally::default(), |mut sum, tally| {
                sum.primitives += tally.primitives;
                sum.measure += tally.measure;
                sum
            });
            println!(
                "   {} — {} primitive(s): ink {} stroke(s)/fill(s) {:.3} mm and {} label(s) {} \
                 character(s); paper {} / {:.3} mm; off the page {}",
                sheet.key,
                sheet.model.primitives.len(),
                ink[0].primitives,
                ink[0].measure,
                ink[1].primitives,
                ink[1].measure,
                paper_total.primitives,
                paper_total.measure,
                off.primitives
            );
            for (class, tally) in &paper {
                println!(
                    "         paper: {:>5} / {:>10.3} mm  {class}",
                    tally.primitives, tally.measure
                );
                let total = batch_paper.entry(class.clone()).or_default();
                total.primitives += tally.primitives;
                total.measure += tally.measure;
            }
            for (slot, tally) in ink.iter().enumerate() {
                batch_ink[slot].primitives += tally.primitives;
                batch_ink[slot].measure += tally.measure;
            }
            batch_off.primitives += off.primitives;
            for ((direction, class), tally) in &page_changes {
                println!(
                    "         {direction}: {:>5} / {:>10.3} mm  {class}",
                    tally.primitives, tally.measure
                );
                let total = changes
                    .entry((direction, class.clone()))
                    .or_default()
                    .entry(batch.name)
                    .or_default();
                total.primitives += tally.primitives;
                total.measure += tally.measure;
                change_pages
                    .entry((direction, class.clone()))
                    .or_default()
                    .insert(format!("{}: {}", batch.name, sheet.key));
                let total = batch_changed.entry(direction).or_default();
                total.primitives += tally.primitives;
                total.measure += tally.measure;
            }
            if !moved.is_empty() {
                moved_pages += 1;
                println!(
                    "         MOVED: {} difference(s) beyond what prints; the first: {}",
                    moved.len(),
                    moved[0]
                );
            }
        }
        println!(
            "   {} in all: ink {} stroke(s)/fill(s) {:.3} mm and {} label(s) {} character(s); off \
             the page {}",
            batch.name,
            batch_ink[0].primitives,
            batch_ink[0].measure,
            batch_ink[1].primitives,
            batch_ink[1].measure,
            batch_off.primitives
        );
        for (class, tally) in &batch_paper {
            println!(
                "      paper: {:>5} / {:>10.3} mm  {class}",
                tally.primitives, tally.measure
            );
        }
        for (direction, tally) in &batch_changed {
            println!(
                "      {direction}: {:>5} / {:>10.3} mm",
                tally.primitives, tally.measure
            );
        }
    }

    if let Some(path) = &dump_to {
        if let Err(error) = std::fs::write(path, dump.as_bytes()) {
            eprintln!("{path}: {error}");
            return ExitCode::from(1);
        }
        println!("wrote {} line(s) to {path}", dump.lines().count() - 1);
    }
    if before.is_none() {
        return ExitCode::SUCCESS;
    }

    println!("== what prints the other way than in the build the dump came from");
    for ((direction, class), by_batch) in &changes {
        let cells: Vec<String> = ["reference", "backup", "fixtures"]
            .iter()
            .map(|name| {
                by_batch.get(name).map_or_else(
                    || format!("{name} —"),
                    |tally| format!("{name} {} / {:.3} mm", tally.primitives, tally.measure),
                )
            })
            .collect();
        println!("   {direction}  {class}: {}", cells.join("; "));
    }
    println!("== changes no page with a vendor plot carries");
    let mut unwitnessed = 0usize;
    for ((direction, class), by_batch) in &changes {
        if by_batch.contains_key("reference") {
            continue;
        }
        unwitnessed += 1;
        let pages = &change_pages[&(*direction, class.clone())];
        let total = by_batch.values().fold(Tally::default(), |mut sum, tally| {
            sum.primitives += tally.primitives;
            sum.measure += tally.measure;
            sum
        });
        println!(
            "   {direction}  {class}: {} / {:.3} mm on {} page(s)",
            total.primitives,
            total.measure,
            pages.len()
        );
        for page in pages.iter().take(12) {
            println!("         {page}");
        }
        if pages.len() > 12 {
            println!("         … and {} more", pages.len() - 12);
        }
    }
    if unwitnessed == 0 {
        println!("   none");
    }
    println!(
        "{} page(s) differ from the dump in more than what prints",
        moved_pages
    );
    if moved_pages == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
