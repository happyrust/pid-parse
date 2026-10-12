//! Read T_CONNECTOR out of an Oracle conventional exp dump and see
//! whether I井场's six connectivity relationships can be assembled
//! from one row, and whether the definition follows from the sides the
//! symbol's item was seen on (`report_definition_rule`).
//!
//! The row decoder is copied from examples/probe_oracle_exp_rows.rs.
//! Type code 12 (DATE) and character form 1 are consumed so
//! T_REPRESENTATION and T_OPC stay aligned; those cells are not used
//! as UIDs. Form 2 is UTF-16BE. NULL is 0xFFFE. The table ends at
//! 0xFFFF. This probe does not run imp, does not connect to Oracle,
//! and does not write src/ or publish/.
//!
//! ```powershell
//! cargo run --example probe_oracle_exp_connectivity
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use quick_xml::events::BytesStart;
use quick_xml::Reader;

const DEFAULT_DUMP: &str = r"D:\work\cad\pid-test-data\Export.dmp";
const DEFAULT_REFERENCE: &str =
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图_Data.xml";
const IJINGCHANG_DOC: &str = "5633AF4121EE4F4D9FE78E90722CC050";

const DEFS: [&str; 6] = [
    "PipingTapOrFitting",
    "PipingEnd1Conn",
    "PipingEnd2Conn",
    "SignalEnd1Conn",
    "SignalEnd2Conn",
    "SignalTapOrFitting",
];
const EXPECTED: [usize; 6] = [369, 129, 143, 38, 46, 13];

fn main() {
    if let Err(err) = run() {
        eprintln!("probe_oracle_exp_connectivity: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    self_check_numbers();
    let mut args = std::env::args().skip(1);
    let dump_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_DUMP), PathBuf::from);
    let reference_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_REFERENCE), PathBuf::from);

    let dump =
        std::fs::read(&dump_path).map_err(|err| format!("read {}: {err}", dump_path.display()))?;
    println!(
        "dump {} bytes ({:.2} MB)",
        dump.len(),
        dump.len() as f64 / 1_048_576.0
    );

    let apps = read_table(&dump, "APPLICATIONS")?;
    pin_applications(&apps)?;
    let connector = read_table(&dump, "T_CONNECTOR")?;
    report_connector(&connector);
    let indexes = Indexes::load(&dump)?;
    let reference = read_xml(&reference_path)?;
    println!(
        "reference DocUID={} rels={}",
        reference.doc_uid,
        reference.rels.len()
    );
    if reference.doc_uid != IJINGCHANG_DOC {
        return Err(format!(
            "reference DocUID {} != {IJINGCHANG_DOC}",
            reference.doc_uid
        ));
    }
    for (index, def) in DEFS.iter().enumerate() {
        let count = reference
            .rels
            .iter()
            .filter(|rel| rel.def_index == index)
            .count();
        println!("reference {def} {count}");
        if count != EXPECTED[index] {
            return Err(format!("{def} count {count} != {}", EXPECTED[index]));
        }
    }

    let both_kinds = indexes
        .pipe
        .iter()
        .filter(|key| indexes.sig.contains(*key))
        .count();
    println!("point_number_in_both_tables={both_kinds}");

    let assembled = assemble(&connector, &indexes, &reference.doc_uid);
    println!(
        "drawing_connectors={} drawing_run_connectors={} skipped_no_rep={}",
        assembled.drawing_connectors, assembled.drawing_run_connectors, assembled.skipped_no_rep
    );
    for how in [OpcHow::Model, OpcHow::Min, OpcHow::Max] {
        let set = &assembled.by_how[how as usize];
        let extra = set
            .keys()
            .filter(|(symbol, run)| !reference.pairs.contains(&(symbol.clone(), run.clone())))
            .count();
        println!(
            "opc_{} produced={} extra={} duplicate_rows={}",
            how.label(),
            set.len(),
            extra,
            assembled.duplicate_rows[how as usize]
        );
    }

    let direct = direct_same_row(&connector, &reference);
    println!("direct_same_row_both_ends={direct}");

    report_defs(&assembled, &indexes, &reference);
    report_definition_rule(&assembled, &reference);
    Ok(())
}

/// The item a symbol end belongs to: a port's parent, a `.BPT` or an OPC
/// by itself.
fn item_of_symbol(symbol: &str) -> String {
    if symbol.ends_with(".BPT") {
        return symbol.to_string();
    }
    match split_point(symbol) {
        Some((parent, _)) => parent,
        None => symbol.to_string(),
    }
}

/// Decide the six definitions from the sides the symbol's *item* was seen
/// on for the same run: both sides means the run passes through the item
/// (`TapOrFitting`); the start side only is `End1Conn`, the end side only
/// `End2Conn`. Prints, per definition, how many reference rows the rule
/// reproduces, and the shapes of the item groups.
fn report_definition_rule(assembled: &Assembled, reference: &XmlDoc) {
    let produced = &assembled.by_how[OpcHow::Min as usize];
    let mut item_sides: HashMap<(String, String), (bool, bool, usize)> = HashMap::new();
    for ((symbol, run), hit) in produced {
        let sides = item_sides
            .entry((item_of_symbol(symbol), run.clone()))
            .or_default();
        sides.0 |= hit.start;
        sides.1 |= hit.end;
        sides.2 += 1;
    }
    println!("rule=item_both_sides_is_tap");
    let mut right_total = 0usize;
    for (index, def) in DEFS.iter().enumerate() {
        let mut right = 0usize;
        let mut wrong: BTreeMap<&str, usize> = BTreeMap::new();
        for rel in reference.rels.iter().filter(|rel| rel.def_index == index) {
            let Some(hit) = produced.get(&(rel.uid1.clone(), rel.uid2.clone())) else {
                *wrong.entry("missing").or_default() += 1;
                continue;
            };
            let (item_start, item_end, _) =
                item_sides[&(item_of_symbol(&rel.uid1), rel.uid2.clone())];
            let piping = hit.run_place == "T_PIPERUN.SP_ID";
            let predicted = match (piping, item_start && item_end, hit.start) {
                (true, true, _) => "PipingTapOrFitting",
                (false, true, _) => "SignalTapOrFitting",
                (true, false, true) => "PipingEnd1Conn",
                (true, false, false) => "PipingEnd2Conn",
                (false, false, true) => "SignalEnd1Conn",
                (false, false, false) => "SignalEnd2Conn",
            };
            if predicted == *def {
                right += 1;
            } else {
                *wrong.entry(predicted).or_default() += 1;
            }
        }
        right_total += right;
        println!(
            "definition {def} reference={} right={right} wrong={wrong:?}",
            EXPECTED[index]
        );
    }
    println!(
        "definition total right={right_total}/{}",
        reference.rels.len()
    );
    let mut shapes: BTreeMap<(bool, bool, usize), usize> = BTreeMap::new();
    for (start, end, points) in item_sides.values() {
        *shapes.entry((*start, *end, *points)).or_default() += 1;
    }
    for ((start, end, points), count) in shapes {
        println!("item_group start={start} end={end} points={points} groups={count}");
    }
}

struct Indexes {
    reps: HashMap<String, Rep>,
    pipe: HashSet<(String, i64)>,
    sig: HashSet<(String, i64)>,
    runs: HashSet<String>,
    sruns: HashSet<String>,
    opc: HashMap<String, Option<String>>,
}

struct Rep {
    model: String,
    drawing: String,
}

impl Indexes {
    fn load(dump: &[u8]) -> Result<Self, String> {
        let reps_table = read_table(dump, "T_REPRESENTATION")?;
        let points = read_table(dump, "T_PIPINGPOINT")?;
        let signals = read_table(dump, "T_SIGNALPOINT")?;
        let runs = read_table(dump, "T_PIPERUN")?;
        let signal_runs = read_table(dump, "T_SIGNALRUN")?;
        let opc = read_table(dump, "T_OPC")?;
        println!(
            "followed T_REPRESENTATION rows={} T_PIPINGPOINT rows={} T_SIGNALPOINT rows={} T_PIPERUN rows={} T_SIGNALRUN rows={} T_OPC rows={}",
            reps_table.rows.len(),
            points.rows.len(),
            signals.rows.len(),
            runs.rows.len(),
            signal_runs.rows.len(),
            opc.rows.len()
        );

        let mut indexes = Indexes {
            reps: HashMap::new(),
            pipe: HashSet::new(),
            sig: HashSet::new(),
            runs: HashSet::new(),
            sruns: HashSet::new(),
            opc: HashMap::new(),
        };
        let id = col(&reps_table, "SP_ID")?;
        let model = col(&reps_table, "SP_MODELITEMID")?;
        let drawing = col(&reps_table, "SP_DRAWINGID")?;
        for row in &reps_table.rows {
            let Some(rep_id) = text_of(row, id) else {
                continue;
            };
            indexes.reps.insert(
                norm_uid(rep_id),
                Rep {
                    model: text_of(row, model).map(norm_uid).unwrap_or_default(),
                    drawing: text_of(row, drawing).map(norm_uid).unwrap_or_default(),
                },
            );
        }
        insert_points(
            &mut indexes.pipe,
            &points,
            "SP_PLANTITEMID",
            "PIPINGPOINTNUMBER",
        )?;
        insert_points(
            &mut indexes.sig,
            &signals,
            "SP_PLANTITEMID",
            "SIGNALPOINTNUMBER",
        )?;
        insert_ids(&mut indexes.runs, &runs, "SP_ID")?;
        insert_ids(&mut indexes.sruns, &signal_runs, "SP_ID")?;
        let opc_id = col(&opc, "SP_ID")?;
        let paired = col(&opc, "SP_PAIREDWITHID")?;
        for row in &opc.rows {
            let Some(sp_id) = text_of(row, opc_id) else {
                continue;
            };
            let other = text_of(row, paired)
                .map(norm_uid)
                .filter(|text| !text.is_empty());
            indexes.opc.insert(norm_uid(sp_id), other);
        }
        Ok(indexes)
    }
}

fn insert_points(
    into: &mut HashSet<(String, i64)>,
    table: &Table,
    parent_name: &str,
    number_name: &str,
) -> Result<(), String> {
    let parent = col(table, parent_name)?;
    let number = col(table, number_name)?;
    for row in &table.rows {
        let Some(parent_id) = text_of(row, parent) else {
            continue;
        };
        let Some(point) = int_of(row, number) else {
            continue;
        };
        into.insert((norm_uid(parent_id), point));
    }
    Ok(())
}

fn insert_ids(into: &mut HashSet<String>, table: &Table, name: &str) -> Result<(), String> {
    let index = col(table, name)?;
    for row in &table.rows {
        if let Some(text) = text_of(row, index) {
            into.insert(norm_uid(text));
        }
    }
    Ok(())
}

struct XmlDoc {
    doc_uid: String,
    rels: Vec<Rel>,
    pairs: HashSet<(String, String)>,
}

struct Rel {
    def_index: usize,
    uid1: String,
    uid2: String,
}

#[derive(Clone, Copy)]
enum OpcHow {
    Model = 0,
    Min = 1,
    Max = 2,
}

impl OpcHow {
    fn label(self) -> &'static str {
        match self {
            OpcHow::Model => "model",
            OpcHow::Min => "min",
            OpcHow::Max => "max",
        }
    }
}

struct Assembly {
    start: bool,
    end: bool,
    symbol_place: &'static str,
    run_place: &'static str,
}

struct Assembled {
    drawing_connectors: usize,
    drawing_run_connectors: usize,
    skipped_no_rep: usize,
    by_how: [HashMap<(String, String), Assembly>; 3],
    duplicate_rows: [usize; 3],
}

fn assemble(connector: &Table, indexes: &Indexes, doc_uid: &str) -> Assembled {
    let sp_id = col(connector, "SP_ID").expect("SP_ID");
    let item1 = col(connector, "SP_CONNECTITEM1ID").expect("item1");
    let item2 = col(connector, "SP_CONNECTITEM2ID").expect("item2");
    let start_object = col(connector, "STARTOBJECT").expect("start");
    let end_object = col(connector, "ENDOBJECT").expect("end");
    let mut assembled = Assembled {
        drawing_connectors: 0,
        drawing_run_connectors: 0,
        skipped_no_rep: 0,
        by_how: [HashMap::new(), HashMap::new(), HashMap::new()],
        duplicate_rows: [0, 0, 0],
    };
    for row in &connector.rows {
        let Some(connector_id) = text_of(row, sp_id) else {
            continue;
        };
        let Some(rep) = indexes.reps.get(&norm_uid(connector_id)) else {
            assembled.skipped_no_rep += 1;
            continue;
        };
        if rep.drawing != doc_uid {
            continue;
        }
        assembled.drawing_connectors += 1;
        let (run_uid, run_place) = if indexes.runs.contains(&rep.model) {
            (rep.model.clone(), "T_PIPERUN.SP_ID")
        } else if indexes.sruns.contains(&rep.model) {
            (rep.model.clone(), "T_SIGNALRUN.SP_ID")
        } else {
            continue;
        };
        assembled.drawing_run_connectors += 1;
        let sides = [
            ("start", text_of(row, item1), int_of(row, start_object)),
            ("end", text_of(row, item2), int_of(row, end_object)),
        ];
        for (side, item, object_index) in sides {
            let Some(item_id) = item else {
                continue;
            };
            for how in [OpcHow::Model, OpcHow::Min, OpcHow::Max] {
                for (symbol, symbol_place) in
                    symbol_ends(indexes, &norm_uid(item_id), object_index, how)
                {
                    let key = (symbol, run_uid.clone());
                    let slot = &mut assembled.by_how[how as usize];
                    if let Some(existing) = slot.get_mut(&key) {
                        assembled.duplicate_rows[how as usize] += 1;
                        if side == "start" {
                            existing.start = true;
                        } else {
                            existing.end = true;
                        }
                        continue;
                    }
                    slot.insert(
                        key,
                        Assembly {
                            start: side == "start",
                            end: side == "end",
                            symbol_place,
                            run_place,
                        },
                    );
                }
            }
        }
    }
    assembled
}

fn symbol_ends(
    indexes: &Indexes,
    rep_id: &str,
    object_index: Option<i64>,
    how: OpcHow,
) -> Vec<(String, &'static str)> {
    let Some(rep) = indexes.reps.get(rep_id) else {
        return Vec::new();
    };
    if rep.model.is_empty() {
        return Vec::new();
    }
    if indexes.runs.contains(&rep.model) || indexes.sruns.contains(&rep.model) {
        return vec![(format!("{rep_id}.BPT"), "T_REPRESENTATION.SP_ID+.BPT")];
    }
    if let Some(paired) = indexes.opc.get(&rep.model) {
        return vec![(
            pick_opc(&rep.model, paired.as_deref(), how),
            match how {
                OpcHow::Model => "T_OPC.SP_ID",
                OpcHow::Min => "T_OPC.min(SP_ID,SP_PAIREDWITHID)",
                OpcHow::Max => "T_OPC.max(SP_ID,SP_PAIREDWITHID)",
            },
        )];
    }
    let Some(index) = object_index.filter(|value| *value > 0) else {
        return Vec::new();
    };
    let mut ends = Vec::new();
    if indexes.pipe.contains(&(rep.model.clone(), index)) {
        ends.push((
            format!("{}.{index}", rep.model),
            "T_PIPINGPOINT.SP_PLANTITEMID+PIPINGPOINTNUMBER",
        ));
    }
    if indexes.sig.contains(&(rep.model.clone(), index)) {
        ends.push((
            format!("{}.{index}", rep.model),
            "T_SIGNALPOINT.SP_PLANTITEMID+SIGNALPOINTNUMBER",
        ));
    }
    ends
}

fn pick_opc(model: &str, paired: Option<&str>, how: OpcHow) -> String {
    let Some(other) = paired.filter(|text| !text.is_empty()) else {
        return model.to_string();
    };
    match how {
        OpcHow::Model => model.to_string(),
        OpcHow::Min => {
            if model <= other {
                model.to_string()
            } else {
                other.to_string()
            }
        }
        OpcHow::Max => {
            if model >= other {
                model.to_string()
            } else {
                other.to_string()
            }
        }
    }
}

fn direct_same_row(connector: &Table, reference: &XmlDoc) -> usize {
    let mut texts_per_row = Vec::with_capacity(connector.rows.len());
    for row in &connector.rows {
        let mut texts = HashSet::new();
        for field in row {
            if let Field::Text(text) = field {
                let uid = norm_uid(text);
                if !uid.is_empty() {
                    texts.insert(uid);
                }
            }
        }
        texts_per_row.push(texts);
    }
    reference
        .rels
        .iter()
        .filter(|rel| {
            texts_per_row
                .iter()
                .any(|texts| texts.contains(&rel.uid1) && texts.contains(&rel.uid2))
        })
        .count()
}

fn report_defs(assembled: &Assembled, indexes: &Indexes, reference: &XmlDoc) {
    let produced = &assembled.by_how[OpcHow::Min as usize];
    println!("rule=opc_min same_row");
    let mut lands: BTreeMap<(usize, &str, &str), usize> = BTreeMap::new();
    for (index, def) in DEFS.iter().enumerate() {
        let mut ordered = 0usize;
        let mut swapped = 0usize;
        let mut addressable = 0usize;
        let mut start_only = 0usize;
        let mut end_only = 0usize;
        let mut both_sides = 0usize;
        let mut missing = 0usize;
        for rel in reference.rels.iter().filter(|rel| rel.def_index == index) {
            if locate(&rel.uid1, indexes).is_some() && locate(&rel.uid2, indexes).is_some() {
                addressable += 1;
            }
            if let Some(hit) = produced.get(&(rel.uid1.clone(), rel.uid2.clone())) {
                ordered += 1;
                match (hit.start, hit.end) {
                    (true, false) => start_only += 1,
                    (false, true) => end_only += 1,
                    (true, true) => both_sides += 1,
                    (false, false) => {}
                }
                *lands
                    .entry((index, hit.symbol_place, hit.run_place))
                    .or_default() += 1;
            } else if produced.contains_key(&(rel.uid2.clone(), rel.uid1.clone())) {
                swapped += 1;
            } else {
                missing += 1;
                if missing <= 3 {
                    println!("  miss {def} {} -> {}", rel.uid1, rel.uid2);
                }
            }
        }
        let reference_count = EXPECTED[index];
        println!(
            "def {def} reference={reference_count} ordered={ordered} swapped={swapped} addressable={addressable} start_only={start_only} end_only={end_only} both_sides={both_sides} missing={missing}"
        );
    }
    for ((index, symbol, run), count) in lands {
        println!("land {} {symbol} -> {run} {count}", DEFS[index]);
    }
}

fn locate(uid: &str, indexes: &Indexes) -> Option<&'static str> {
    if let Some(stem) = uid.strip_suffix(".BPT") {
        if indexes.reps.contains_key(stem) {
            return Some("T_REPRESENTATION.SP_ID");
        }
    }
    if let Some((parent, number)) = split_point(uid) {
        if indexes.pipe.contains(&(parent.clone(), number)) {
            return Some("T_PIPINGPOINT.SP_PLANTITEMID+PIPINGPOINTNUMBER");
        }
        if indexes.sig.contains(&(parent, number)) {
            return Some("T_SIGNALPOINT.SP_PLANTITEMID+SIGNALPOINTNUMBER");
        }
    }
    if indexes.runs.contains(uid) {
        return Some("T_PIPERUN.SP_ID");
    }
    if indexes.sruns.contains(uid) {
        return Some("T_SIGNALRUN.SP_ID");
    }
    if indexes.opc.contains_key(uid) {
        return Some("T_OPC.SP_ID");
    }
    None
}

fn split_point(uid: &str) -> Option<(String, i64)> {
    let (parent, number) = uid.rsplit_once('.')?;
    if parent.is_empty() {
        return None;
    }
    let number = number.parse::<i64>().ok()?;
    Some((parent.to_string(), number))
}

fn pin_applications(table: &Table) -> Result<(), String> {
    let row = table
        .rows
        .first()
        .ok_or("APPLICATIONS has no rows; row format is not pinned")?;
    let id = int_at(row, &table.columns, "ID");
    let name = text_at(row, &table.columns, "NAME");
    let short = text_at(row, &table.columns, "SHORTNAME");
    println!(
        "format_pin APPLICATIONS rows={} id={id:?} name={name:?} short={short:?}",
        table.rows.len()
    );
    if id != Some(1) || name != "SmartPlant PID" || short != "SPPID" {
        return Err(
            "APPLICATIONS row 0 is not (1, SmartPlant PID, SPPID); refusing to read the big tables"
                .into(),
        );
    }
    println!("format_pin ok tail={}", table.tail);
    Ok(())
}

fn report_connector(table: &Table) {
    let null_cols = table
        .columns
        .iter()
        .filter(|column| non_null(table, &column.name) == 0)
        .count();
    println!(
        "T_CONNECTOR rows={} cols={} null_cols={null_cols} tail={}",
        table.rows.len(),
        table.columns.len(),
        table.tail
    );
    for column in &table.columns {
        let count = non_null(table, &column.name);
        match column.kind {
            Kind::Char => println!(
                "  col {} char non_null={count} maxlen={} charset={:?} form={:?}",
                column.name, column.maxlen, column.charset, column.form
            ),
            Kind::Num => println!(
                "  col {} number non_null={count} maxlen={}",
                column.name, column.maxlen
            ),
            Kind::Date => println!(
                "  col {} date non_null={count} maxlen={}",
                column.name, column.maxlen
            ),
        }
    }
}

fn non_null(table: &Table, name: &str) -> usize {
    let index = col(table, name).expect("column");
    table
        .rows
        .iter()
        .filter(|row| !matches!(row.get(index), Some(Field::Null) | None))
        .count()
}

fn col(table: &Table, name: &str) -> Result<usize, String> {
    table
        .columns
        .iter()
        .position(|column| column.name == name)
        .ok_or_else(|| format!("{} has no column {name}", table.name))
}

fn text_of(row: &[Field], index: usize) -> Option<&str> {
    match row.get(index) {
        Some(Field::Text(text)) if !text.is_empty() => Some(text),
        _ => None,
    }
}

fn int_of(row: &[Field], index: usize) -> Option<i64> {
    match row.get(index) {
        Some(Field::Int(value)) => Some(*value),
        _ => None,
    }
}

fn text_at(row: &[Field], columns: &[Column], name: &str) -> String {
    let Some(index) = columns.iter().position(|column| column.name == name) else {
        return String::new();
    };
    match row.get(index) {
        Some(Field::Text(text)) => text.clone(),
        Some(Field::Int(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn int_at(row: &[Field], columns: &[Column], name: &str) -> Option<i64> {
    let index = columns.iter().position(|column| column.name == name)?;
    int_of(row, index)
}

fn norm_uid(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch == '{' || ch == '}' || ch == '-' || ch.is_whitespace() {
            continue;
        }
        out.extend(ch.to_uppercase());
    }
    out
}

struct Table {
    name: String,
    columns: Vec<Column>,
    rows: Vec<Vec<Field>>,
    tail: String,
}

struct Column {
    name: String,
    kind: Kind,
    maxlen: u16,
    charset: Option<u16>,
    form: Option<u16>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Char,
    Num,
    Date,
}

#[derive(Clone)]
enum Field {
    Null,
    Int(i64),
    Text(String),
    Unscaled,
}

fn read_table(dump: &[u8], name: &str) -> Result<Table, String> {
    let marker = format!("INSERT INTO \"{name}\"");
    let at = find(dump, marker.as_bytes()).ok_or_else(|| format!("{name}: INSERT not found"))?;
    let line_end = dump[at..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| at + offset)
        .ok_or_else(|| format!("{name}: INSERT line has no newline"))?;
    let line = std::str::from_utf8(&dump[at..line_end])
        .map_err(|_| format!("{name}: INSERT line is not ASCII"))?;
    let names = column_names(line).map_err(|err| format!("{name}: {err}"))?;
    let (columns, mut pos) =
        parse_header(dump, line_end + 1, &names).map_err(|err| format!("{name}: {err}"))?;
    let mut rows = Vec::new();
    loop {
        if pos + 2 > dump.len() {
            return Err(format!("{name}: truncated before end marker"));
        }
        let peek = u16(dump, pos);
        if peek == 0xFFFF {
            let tail = &dump[pos + 2..dump.len().min(pos + 10)];
            let tail = String::from_utf8_lossy(tail).replace('\n', "\\n");
            return Ok(Table {
                name: name.to_string(),
                columns,
                rows,
                tail,
            });
        }
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            let (field, next) = read_field(dump, pos, column)
                .map_err(|err| format!("{name}: row {} {err}", rows.len()))?;
            pos = next;
            row.push(field);
        }
        if dump.get(pos..pos + 2) != Some(&[0, 0]) {
            return Err(format!(
                "{name}: row {} separator is {:02x?} at {pos:#x}",
                rows.len(),
                dump.get(pos..pos + 2)
            ));
        }
        pos += 2;
        rows.push(row);
    }
}

fn column_names(line: &str) -> Result<Vec<String>, String> {
    let open = line.find('(').ok_or("INSERT has no column list")?;
    let close = line.find(") VALUES").ok_or("INSERT has no VALUES")?;
    let mut names = Vec::new();
    for piece in line[open + 1..close].split(',') {
        let name = piece.trim().trim_matches('"');
        if name.is_empty() {
            return Err("empty column name".into());
        }
        names.push(name.to_string());
    }
    Ok(names)
}

fn parse_header(
    dump: &[u8],
    mut pos: usize,
    names: &[String],
) -> Result<(Vec<Column>, usize), String> {
    let count = u16(dump, pos) as usize;
    pos += 2;
    if count != names.len() {
        return Err(format!(
            "header columns {count} != INSERT names {}",
            names.len()
        ));
    }
    let mut columns = Vec::with_capacity(count);
    for name in names {
        let kind_code = u16(dump, pos);
        pos += 2;
        let column = match kind_code {
            1 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                let charset = u16(dump, pos);
                pos += 2;
                let form = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Char,
                    maxlen,
                    charset: Some(charset),
                    form: Some(form),
                }
            }
            2 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Num,
                    maxlen,
                    charset: None,
                    form: None,
                }
            }
            12 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Date,
                    maxlen,
                    charset: None,
                    form: None,
                }
            }
            other => return Err(format!("column {name} type {other:#x} at {pos:#x}")),
        };
        columns.push(column);
    }
    if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
        return Err(format!("column header terminator at {pos:#x}"));
    }
    Ok((columns, pos + 4))
}

fn read_field(dump: &[u8], pos: usize, column: &Column) -> Result<(Field, usize), String> {
    if pos + 2 > dump.len() {
        return Err(format!("truncated length at {pos:#x}"));
    }
    let len = u16(dump, pos) as usize;
    let pos = pos + 2;
    if len == 0xFFFE {
        return Ok((Field::Null, pos));
    }
    if len > column.maxlen as usize + 8 || pos + len > dump.len() {
        return Err(format!("length {len} at {pos:#x}"));
    }
    let raw = &dump[pos..pos + len];
    let next = pos + len;
    let field = match column.kind {
        Kind::Char if column.form == Some(2) => {
            if !len.is_multiple_of(2) {
                return Err(format!("odd UTF-16 length {len}"));
            }
            Field::Text(decode_utf16_be(raw))
        }
        Kind::Char => Field::Text(String::from_utf8_lossy(raw).into_owned()),
        Kind::Num => match decode_i64(raw) {
            Some(value) => Field::Int(value),
            None => Field::Unscaled,
        },
        Kind::Date => Field::Unscaled,
    };
    Ok((field, next))
}

fn decode_utf16_be(raw: &[u8]) -> String {
    let units = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

fn decode_i64(raw: &[u8]) -> Option<i64> {
    if raw.is_empty() {
        return None;
    }
    if raw == [0x80] {
        return Some(0);
    }
    let negative = raw[0] < 0x80;
    let body = if negative && raw.last() == Some(&0x66) && raw.len() < 21 {
        &raw[..raw.len() - 1]
    } else {
        raw
    };
    if body.is_empty() {
        return None;
    }
    let exponent = if negative {
        62 - i32::from(body[0])
    } else {
        i32::from(body[0]) - 193
    };
    let mut total: i128 = 0;
    for (index, byte) in body[1..].iter().enumerate() {
        let digit = if negative {
            101 - i32::from(*byte)
        } else {
            i32::from(*byte) - 1
        };
        if !(0..100).contains(&digit) {
            return None;
        }
        let power = exponent - index as i32;
        if power < 0 {
            if digit != 0 {
                return None;
            }
            continue;
        }
        let mut place: i128 = i128::from(digit);
        for _ in 0..power {
            place = place.checked_mul(100)?;
        }
        total = total.checked_add(place)?;
    }
    let signed = if negative { -total } else { total };
    i64::try_from(signed).ok()
}

fn self_check_numbers() {
    assert_eq!(decode_i64(&[0x80]), Some(0));
    assert_eq!(decode_i64(&[0xc1, 0x02]), Some(1));
    assert_eq!(decode_i64(&[0xc1, 0x03]), Some(2));
    assert_eq!(decode_i64(&[0xc1, 0x06]), Some(5));
    assert_eq!(decode_i64(&[0xc2, 0x02]), Some(100));
    assert_eq!(decode_i64(&[0xc2, 0x1a, 0x34]), Some(2551));
    assert_eq!(decode_i64(&[0xc2, 0x10, 0x1f]), Some(1530));
    assert_eq!(decode_i64(&[0x3e, 0x64, 0x66]), Some(-1));
    assert_eq!(decode_i64(&[0x3e, 0x62, 0x66]), Some(-3));
}

fn u16(dump: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([dump[pos], dump[pos + 1]])
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

fn read_xml(path: &Path) -> Result<XmlDoc, String> {
    let xml =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut doc = XmlDoc {
        doc_uid: String::new(),
        rels: Vec::new(),
        pairs: HashSet::new(),
    };
    let mut depth = 0usize;
    let mut tag = String::new();
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(start)) => {
                if depth == 0 {
                    take_doc_uid(&start, &mut doc.doc_uid);
                } else if depth == 1 {
                    tag = local_name(&start);
                    values.clear();
                } else if depth == 2 {
                    take(&start, &mut values);
                }
                depth += 1;
            }
            Ok(quick_xml::events::Event::Empty(start)) => {
                if depth == 0 {
                    take_doc_uid(&start, &mut doc.doc_uid);
                } else if depth == 2 {
                    take(&start, &mut values);
                }
            }
            Ok(quick_xml::events::Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    finish(&tag, &values, &mut doc)?;
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(err) => return Err(format!("xml {}: {err}", path.display())),
            _ => {}
        }
    }
    Ok(doc)
}

fn take_doc_uid(start: &BytesStart<'_>, doc_uid: &mut String) {
    if local_name(start) != "Container" {
        return;
    }
    for (name, value) in attributes_of(start) {
        if name == "DocUID" {
            *doc_uid = norm_uid(&value);
        }
    }
}

fn take(start: &BytesStart<'_>, values: &mut BTreeMap<String, String>) {
    let interface = local_name(start);
    for (name, value) in attributes_of(start) {
        values.insert(format!("{interface}.{name}"), value);
    }
}

fn finish(tag: &str, values: &BTreeMap<String, String>, doc: &mut XmlDoc) -> Result<(), String> {
    if tag != "Rel" {
        return Ok(());
    }
    let Some(def) = values.get("IRel.DefUID").map(String::as_str) else {
        return Ok(());
    };
    let Some(def_index) = DEFS.iter().position(|name| *name == def) else {
        return Ok(());
    };
    let uid1 = norm_uid(values.get("IRel.UID1").map(String::as_str).unwrap_or(""));
    let uid2 = norm_uid(values.get("IRel.UID2").map(String::as_str).unwrap_or(""));
    if !doc.pairs.insert((uid1.clone(), uid2.clone())) {
        return Err(format!("duplicate reference pair {def} {uid1} {uid2}"));
    }
    doc.rels.push(Rel {
        def_index,
        uid1,
        uid2,
    });
    Ok(())
}

fn attributes_of(start: &BytesStart<'_>) -> Vec<(String, String)> {
    start
        .attributes()
        .flatten()
        .map(|attribute| {
            (
                String::from_utf8_lossy(attribute.key.as_ref()).into_owned(),
                attribute
                    .unescape_value()
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn local_name(start: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(start.name().as_ref()).into_owned()
}
