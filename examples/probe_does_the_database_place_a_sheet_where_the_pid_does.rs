//! Does the plant database place a sheet's symbols and labels where its `.pid`
//! does, and does it spell its off-page connectors and pipe runs the way the
//! page letters them?
//!
//! A `.pid` caches what the database held when the drawing was last saved:
//! `T_SYMBOL` and `T_LABELPERSIST` carry a point for every representation,
//! joined to the drawing's own records by `T_REPRESENTATION.GRAPHICOID`, and
//! `T_MODELITEM.DESCRIPTION` / `T_PLANTITEM.ITEMTAG` carry the strings a label
//! is driven from. When a reference plot shows an instrument elsewhere, or a
//! pipeline lettered differently, than the `.pid` on hand, there are two
//! readings: the `.pid` is stale against its own database, or the plot was
//! made from a later state of both. This asks the database half of that.
//!
//! For each drawing given, against one Oracle `exp` dump:
//!
//! - every `T_SYMBOL` row of the drawing against the `igSymbol2d` placement
//!   with the same graphic oid. A row is the symbol's own origin and a
//!   placement its insertion point, so the two differ by a constant for each
//!   symbol file whose origin is not its insertion (`Off-Unit SQ.sym`,
//!   20.674 mm; a label drawn on a letter sheet, 176.548 mm). What would show
//!   a symbol moved in one and not the other is a file whose placements
//!   differ from their rows by *different* amounts once each is divided by
//!   the row's `SCALEFACTOR` (`Skid Mounted Equipment.sym` is drawn to two
//!   sizes on the `J K` sheet: 504.112 mm and 464.561 mm, both 89.616 mm at
//!   unit scale), and that is what fails;
//! - every `T_LABELPERSIST` row against the sheet: whether a text box, or
//!   failing that a symbol placement, is inserted within a micrometre of the
//!   row's point (a count, not a verdict — a label's point is its own, and
//!   only some label kinds letter from it);
//! - each off-page connector's description, with whether the page letters it,
//!   and each tagged pipe run's item tag and nominal diameter, with how often
//!   the page letters the tag (a line label drops the unit: `50 mm-PC-…` is
//!   lettered `50-PC-…`);
//! - with `--tag <text>`, the rows of every plant item whose item tag carries
//!   `<text>`, side by side with the placement.
//!
//! ```text
//! cargo run --example probe_does_the_database_place_a_sheet_where_the_pid_does -- \
//!     <Export.dmp> [--tag <text>] <drawing.pid> [<drawing.pid> ...]
//! ```
//!
//! Exit code 1 when a symbol file's placements disagree with their rows by
//! different amounts, or a connector description is lettered nowhere on the
//! page. Read-only; the dump reader is this file's own (`INSERT INTO` line,
//! column header, length-prefixed cells, as `src/publish/oracle_exp.rs`
//! documents the layout) because the crate's reader exposes only the publish
//! tables.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::{build_normalized_geometry, PidGraphicKind, PidParser};

/// One cell of an `exp` row.
#[derive(Clone, Debug)]
enum Cell {
    Null,
    Text(String),
    Number(f64),
    Other,
}

impl Cell {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
    fn number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }
}

/// One table read out of the dump: column names and rows.
struct Table {
    columns: Vec<String>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    fn column(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|column| column == name)
    }
    fn text<'a>(&self, row: &'a [Cell], name: &str) -> Option<&'a str> {
        row.get(self.column(name)?)?.text()
    }
    fn number(&self, row: &[Cell], name: &str) -> Option<f64> {
        row.get(self.column(name)?)?.number()
    }
    /// Row index by `SP_ID`, braces and dashes dropped.
    fn by_id(&self) -> HashMap<String, usize> {
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| Some((key(self.text(row, "SP_ID")?), index)))
            .collect()
    }
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// An Oracle `NUMBER` as a float: sign and base-100 exponent in the first
/// byte, base-100 digits after it, a trailing `0x66` on a negative value.
fn oracle_number(raw: &[u8]) -> Option<f64> {
    let (&first, rest) = raw.split_first()?;
    if raw == [0x80] {
        return Some(0.0);
    }
    let negative = first < 0x80;
    let digits = if negative && rest.last() == Some(&0x66) && raw.len() < 21 {
        &rest[..rest.len() - 1]
    } else {
        rest
    };
    let exponent = if negative {
        62 - i32::from(first)
    } else {
        i32::from(first) - 193
    };
    let mut total = 0.0_f64;
    for (index, &byte) in digits.iter().enumerate() {
        let digit = if negative {
            101 - i32::from(byte)
        } else {
            i32::from(byte) - 1
        };
        if !(0..100).contains(&digit) {
            return None;
        }
        total += f64::from(digit) * 100f64.powi(exponent - i32::try_from(index).ok()?);
    }
    Some(if negative { -total } else { total })
}

/// Read the first `INSERT INTO "<name>"` of `dump`.
fn read_table(dump: &[u8], name: &str) -> Result<Table, String> {
    let marker = format!("INSERT INTO \"{name}\"");
    let at = dump
        .windows(marker.len())
        .position(|window| window == marker.as_bytes())
        .ok_or_else(|| format!("{name}: no INSERT in the dump"))?;
    let line_end = at
        + dump[at..]
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| format!("{name}: the INSERT line does not end"))?;
    let line = String::from_utf8_lossy(&dump[at..line_end]);
    let open = line
        .find('(')
        .ok_or_else(|| format!("{name}: no column list"))?;
    let close = line
        .find(") VALUES")
        .ok_or_else(|| format!("{name}: no VALUES"))?;
    let columns: Vec<String> = line[open + 1..close]
        .split(',')
        .map(|part| part.trim().trim_matches('"').to_string())
        .collect();
    let mut pos = line_end + 1;
    let short = || format!("{name}: the column header is cut short");
    let count = usize::from(u16_at(dump, pos).ok_or_else(short)?);
    if count != columns.len() {
        return Err(format!(
            "{name}: the header states {count} columns and the INSERT {}",
            columns.len()
        ));
    }
    pos += 2;
    // 1 = character (length, charset, form), 2 = NUMBER, 12 = DATE.
    let mut kinds: Vec<(u16, u16)> = Vec::with_capacity(count);
    for column in &columns {
        let kind = u16_at(dump, pos).ok_or_else(short)?;
        pos += 2;
        match kind {
            1 => {
                let charset = u16_at(dump, pos + 2).ok_or_else(short)?;
                pos += 6;
                kinds.push((kind, charset));
            }
            2 | 12 => {
                pos += 2;
                kinds.push((kind, 0));
            }
            other => return Err(format!("{name}: column {column} has type {other}")),
        }
    }
    if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
        return Err(format!(
            "{name}: the header does not close on four zero bytes"
        ));
    }
    pos += 4;
    let mut rows = Vec::new();
    loop {
        let peek = u16_at(dump, pos).ok_or_else(|| format!("{name}: the table does not end"))?;
        if peek == 0xFFFF {
            break;
        }
        let mut row = Vec::with_capacity(count);
        for (kind, charset) in &kinds {
            let length = u16_at(dump, pos).ok_or_else(|| format!("{name}: short row"))?;
            pos += 2;
            if length == 0xFFFE {
                row.push(Cell::Null);
                continue;
            }
            let end = pos + usize::from(length);
            let raw = dump
                .get(pos..end)
                .ok_or_else(|| format!("{name}: short cell"))?;
            pos = end;
            row.push(match kind {
                1 if *charset == 2000 => {
                    let units: Vec<u16> = raw
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| u16::from_be_bytes(*pair))
                        .collect();
                    Cell::Text(String::from_utf16_lossy(&units))
                }
                1 => Cell::Text(String::from_utf8_lossy(raw).into_owned()),
                2 => oracle_number(raw).map_or(Cell::Other, Cell::Number),
                _ => Cell::Other,
            });
        }
        if dump.get(pos..pos + 2) != Some(&[0, 0]) {
            return Err(format!(
                "{name}: row {} does not close on two zero bytes",
                rows.len()
            ));
        }
        pos += 2;
        rows.push(row);
    }
    Ok(Table { columns, rows })
}

/// A UID with braces and dashes dropped, upper case: the form the tables
/// join on.
fn key(uid: &str) -> String {
    uid.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

/// What the `.pid` says about one drawing: every symbol placement of its own
/// sheet by graphic oid, and every text box with where it is inserted.
struct Sheet {
    placements: HashMap<u32, ([f64; 2], String)>,
    texts: Vec<(u32, [f64; 2], String)>,
}

fn read_sheet(path: &Path) -> Result<Sheet, String> {
    let doc = PidParser::new()
        .parse_file(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let geometry = build_normalized_geometry(&doc);
    let mut placements = HashMap::new();
    let mut texts = Vec::new();
    for entity in &geometry.entities {
        // The drawing's own sheet only: a storage's sheets are symbol bodies.
        let top_level = entity
            .source
            .stream_path
            .as_deref()
            .is_some_and(|stream| stream.matches('/').count() == 1);
        if !top_level {
            continue;
        }
        let oid = entity.graphic_oid.unwrap_or_default();
        match &entity.kind {
            PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                ..
            } => {
                placements.insert(
                    oid,
                    (
                        [insertion.x, insertion.y],
                        symbol_path.clone().unwrap_or_default(),
                    ),
                );
            }
            PidGraphicKind::Text {
                value, insertion, ..
            } => texts.push((oid, [insertion.x, insertion.y], value.clone())),
            _ => {}
        }
    }
    Ok(Sheet { placements, texts })
}

/// A micrometre, in the tables' metres.
const MICROMETRE_M: f64 = 1.0e-6;

fn leaf(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Compare one drawing with the dump. Returns whether the two agree.
fn compare_drawing(path: &Path, tables: &BTreeMap<&str, Table>, tag: Option<&str>) -> bool {
    let drawing = &tables["T_DRAWING"];
    let representation = &tables["T_REPRESENTATION"];
    let symbol = &tables["T_SYMBOL"];
    let label = &tables["T_LABELPERSIST"];
    let model = &tables["T_MODELITEM"];
    let plant = &tables["T_PLANTITEM"];
    let run = &tables["T_PIPERUN"];
    let (symbol_at, label_at, model_at, plant_at, run_at) = (
        symbol.by_id(),
        label.by_id(),
        model.by_id(),
        plant.by_id(),
        run.by_id(),
    );
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(row) = drawing.rows.iter().find(|row| {
        drawing
            .text(row, "PATH")
            .is_some_and(|stored| leaf(stored) == file_name)
    }) else {
        eprintln!("{file_name}: no T_DRAWING row names this file");
        return false;
    };
    let drawing_id = key(drawing.text(row, "SP_ID").unwrap_or_default());
    let sheet = match read_sheet(path) {
        Ok(sheet) => sheet,
        Err(error) => {
            eprintln!("{error}");
            return false;
        }
    };
    println!(
        "== {} — T_DRAWING template {:?}; the .pid places {} symbols and {} text boxes",
        path.display(),
        drawing.text(row, "TEMPLATE").unwrap_or_default(),
        sheet.placements.len(),
        sheet.texts.len(),
    );

    // Symbol file -> how far each of its placements sits from its row, in
    // metres, with the scale factor the row draws it to.
    let mut by_file: BTreeMap<String, Vec<(u32, f64, f64)>> = BTreeMap::new();
    let (mut symbol_rows, mut symbol_found) = (0usize, 0usize);
    let (mut label_rows, mut label_on_a_box, mut label_next_oid) = (0usize, 0usize, 0usize);
    let mut label_on_a_symbol = 0usize;
    // Representation -> the model item it draws, for every drawing: a label's
    // row names the representation it labels.
    let rep_model: HashMap<String, String> = representation
        .rows
        .iter()
        .map(|rep| {
            (
                key(representation.text(rep, "SP_ID").unwrap_or_default()),
                key(representation
                    .text(rep, "SP_MODELITEMID")
                    .unwrap_or_default()),
            )
        })
        .collect();
    let mut tagged: Vec<String> = Vec::new();
    let mut connectors: Vec<(String, String)> = Vec::new();
    let mut runs: BTreeMap<String, (String, Option<f64>)> = BTreeMap::new();
    for rep in &representation.rows {
        if key(representation.text(rep, "SP_DRAWINGID").unwrap_or_default()) != drawing_id {
            continue;
        }
        let rep_id = key(representation.text(rep, "SP_ID").unwrap_or_default());
        let model_id = key(representation
            .text(rep, "SP_MODELITEMID")
            .unwrap_or_default());
        let oid = representation
            .number(rep, "GRAPHICOID")
            .map_or(0, |value| value as u32);
        let item_tag = plant_at
            .get(&model_id)
            .and_then(|index| plant.text(&plant.rows[*index], "ITEMTAG"))
            .unwrap_or_default();
        let wanted = tag.is_some_and(|needle| item_tag.contains(needle));
        if let Some(index) = symbol_at.get(&rep_id) {
            let stored = &symbol.rows[*index];
            if let (Some(x), Some(y)) = (
                symbol.number(stored, "XCOORDINATE"),
                symbol.number(stored, "YCOORDINATE"),
            ) {
                symbol_rows += 1;
                // A placement drawn larger carries its origin that much
                // further from its insertion point.
                let scale = symbol
                    .number(stored, "SCALEFACTOR")
                    .filter(|scale| *scale > 0.0)
                    .unwrap_or(1.0);
                match sheet.placements.get(&oid) {
                    Some((at, file)) => {
                        symbol_found += 1;
                        let distance = (at[0] - x).hypot(at[1] - y);
                        by_file
                            .entry(leaf(file).to_string())
                            .or_default()
                            .push((oid, distance, scale));
                        if wanted {
                            tagged.push(format!(
                                "{item_tag:<24} symbol oid {oid:>6} database ({x:.6}, {y:.6}) m | .pid \
                                 ({:.6}, {:.6}) m | {:.3} mm apart | {}",
                                at[0],
                                at[1],
                                distance * 1000.0,
                                leaf(file)
                            ));
                        }
                    }
                    None if wanted => tagged.push(format!(
                        "{item_tag:<24} symbol oid {oid:>6} database ({x:.6}, {y:.6}) m | no \
                         igSymbol2d of that oid (a run's own representation)"
                    )),
                    None => {}
                }
            }
        }
        if let Some(index) = label_at.get(&rep_id) {
            let stored = &label.rows[*index];
            if let (Some(x), Some(y)) = (
                label.number(stored, "XCOORDINATE"),
                label.number(stored, "YCOORDINATE"),
            ) {
                label_rows += 1;
                // A label is tagged by the item it labels: the representation
                // its row names, not its own.
                let labelled = key(label
                    .text(stored, "SP_REPRESENTATIONID")
                    .unwrap_or_default());
                let labelled_tag = rep_model
                    .get(&labelled)
                    .and_then(|model_id| plant_at.get(model_id))
                    .and_then(|index| plant.text(&plant.rows[*index], "ITEMTAG"))
                    .unwrap_or_default();
                let wanted = tag.is_some_and(|needle| labelled_tag.contains(needle));
                let here = |at: &[f64; 2]| (at[0] - x).hypot(at[1] - y) <= MICROMETRE_M;
                let on_box = sheet.texts.iter().find(|(_, at, _)| here(at));
                let on_symbol = sheet.placements.iter().find(|(_, (at, _))| here(at));
                let found = if let Some((text_oid, _, value)) = on_box {
                    label_on_a_box += 1;
                    if *text_oid == oid + 1 {
                        label_next_oid += 1;
                    }
                    format!("text box oid {text_oid} {value:?} inserted there")
                } else if let Some((symbol_oid, (_, file))) = on_symbol {
                    label_on_a_symbol += 1;
                    format!("igSymbol2d oid {symbol_oid} {} inserted there", leaf(file))
                } else {
                    "nothing of the sheet inserted there".to_string()
                };
                if wanted {
                    tagged.push(format!(
                        "{labelled_tag:<24} label  oid {oid:>6} database ({x:.6}, {y:.6}) m | {found}"
                    ));
                }
            }
        }
        if let Some(index) = model_at.get(&model_id) {
            let item = &model.rows[*index];
            let description = model
                .text(item, "DESCRIPTION")
                .filter(|_| model.text(item, "ITEMTYPENAME") == Some("OPC"));
            if let Some(description) = description {
                if !connectors.iter().any(|(id, _)| *id == model_id) {
                    connectors.push((model_id.clone(), description.to_string()));
                }
            }
        }
        if let Some(index) = run_at.get(&model_id).filter(|_| !item_tag.is_empty()) {
            runs.entry(model_id.clone()).or_insert((
                item_tag.to_string(),
                run.number(&run.rows[*index], "NOMINALDIAMETER"),
            ));
        }
    }

    let mut agree = true;
    let exact: usize = by_file
        .values()
        .flatten()
        .filter(|(_, distance, _)| *distance <= MICROMETRE_M)
        .count();
    println!(
        "   T_SYMBOL: {symbol_rows} rows of the drawing, {symbol_found} with an igSymbol2d of the same \
         graphic oid, {exact} of those within a micrometre of it"
    );
    for (file, distances) in &by_file {
        let span = |of: &dyn Fn(&(u32, f64, f64)) -> f64| {
            let low = distances.iter().map(of).fold(f64::MAX, f64::min);
            let high = distances.iter().map(of).fold(0.0, f64::max);
            (low, high)
        };
        let (low, high) = span(&|(_, distance, _)| *distance);
        // The same offset at unit scale: what a symbol's own origin leaves
        // when its placements are drawn to different sizes.
        let (unit_low, unit_high) = span(&|(_, distance, scale)| *distance / *scale);
        if high <= MICROMETRE_M {
            continue;
        }
        if high - low <= MICROMETRE_M {
            println!(
                "      {file:<44} ×{:<3} every row {:.3} mm from its placement: the symbol's own \
                 origin",
                distances.len(),
                high * 1000.0
            );
        } else if unit_high - unit_low <= MICROMETRE_M {
            println!(
                "      {file:<44} ×{:<3} every row {:.3} mm × its scale factor from its placement \
                 ({:.3}…{:.3} mm): the symbol's own origin",
                distances.len(),
                unit_high * 1000.0,
                low * 1000.0,
                high * 1000.0
            );
        } else {
            agree = false;
            println!(
                "      {file:<44} ×{:<3} rows {:.3}…{:.3} mm from their placements: NOT one offset",
                distances.len(),
                low * 1000.0,
                high * 1000.0
            );
            for (oid, distance, scale) in distances {
                println!(
                    "         oid {oid:>6} {:.3} mm at scale {scale:.6}",
                    distance * 1000.0
                );
            }
        }
    }
    println!(
        "   T_LABELPERSIST: {label_rows} rows of the drawing; within a micrometre of the row's point \
         {label_on_a_box} have a text box inserted ({label_next_oid} of them the box numbered one \
         after the label), {label_on_a_symbol} more a symbol placement, {} nothing of the sheet",
        label_rows - label_on_a_box - label_on_a_symbol
    );
    for line in &tagged {
        println!("   --tag {line}");
    }
    println!("   off-page connectors: {}", connectors.len());
    for (id, description) in &connectors {
        let lettered = sheet
            .texts
            .iter()
            .any(|(_, _, text)| text.trim() == description.trim());
        agree &= lettered;
        println!(
            "      {} DESCRIPTION {description:?} — {}",
            &id[..8.min(id.len())],
            if lettered {
                "lettered on the page"
            } else {
                "NOT lettered on the page"
            }
        );
    }
    let mut by_tag: BTreeMap<String, (usize, Option<f64>)> = BTreeMap::new();
    for (item_tag, diameter) in runs.values() {
        by_tag.entry(item_tag.clone()).or_insert((0, *diameter)).0 += 1;
    }
    println!(
        "   tagged pipe runs: {} ({} distinct tags); those the page letters:",
        runs.len(),
        by_tag.len()
    );
    for (item_tag, (count, diameter)) in &by_tag {
        let spelled = item_tag.replace(" mm-", "-");
        let lettered = sheet
            .texts
            .iter()
            .filter(|(_, _, text)| text.trim() == spelled)
            .count();
        if lettered == 0 {
            continue;
        }
        println!(
            "      ITEMTAG {item_tag:?} ×{count} NOMINALDIAMETER {} — lettered {spelled:?} {lettered} \
             time(s)",
            diameter.map_or_else(|| "NULL".to_string(), |value| format!("{value}")),
        );
    }
    agree
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(dump_path) = args.next() else {
        eprintln!(
            "usage: probe_does_the_database_place_a_sheet_where_the_pid_does <Export.dmp> \
             [--tag <text>] <drawing.pid> ..."
        );
        return ExitCode::from(2);
    };
    let mut tag: Option<String> = None;
    let mut drawings: Vec<PathBuf> = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--tag" {
            tag = args.next();
        } else {
            drawings.push(PathBuf::from(arg));
        }
    }
    let dump = match std::fs::read(&dump_path) {
        Ok(dump) => dump,
        Err(error) => {
            eprintln!("{dump_path}: {error}");
            return ExitCode::from(1);
        }
    };
    let mut tables: BTreeMap<&str, Table> = BTreeMap::new();
    for name in [
        "T_DRAWING",
        "T_REPRESENTATION",
        "T_SYMBOL",
        "T_LABELPERSIST",
        "T_MODELITEM",
        "T_PLANTITEM",
        "T_PIPERUN",
    ] {
        match read_table(&dump, name) {
            Ok(table) => {
                tables.insert(name, table);
            }
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(1);
            }
        }
    }
    println!(
        "{dump_path}: T_DRAWING {} rows, T_REPRESENTATION {}, T_SYMBOL {}, T_LABELPERSIST {}, \
         T_MODELITEM {}, T_PLANTITEM {}, T_PIPERUN {}",
        tables["T_DRAWING"].rows.len(),
        tables["T_REPRESENTATION"].rows.len(),
        tables["T_SYMBOL"].rows.len(),
        tables["T_LABELPERSIST"].rows.len(),
        tables["T_MODELITEM"].rows.len(),
        tables["T_PLANTITEM"].rows.len(),
        tables["T_PIPERUN"].rows.len(),
    );
    let mut agree = true;
    for path in &drawings {
        agree &= compare_drawing(path, &tables, tag.as_deref());
    }
    if agree {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
