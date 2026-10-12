//! Read I井场 nozzle, note, and OPC rows out of an Oracle conventional
//! `exp` dump by column, then compare them with the reference publish XML.
//!
//! Row format matches `probe_oracle_exp_rows`: length-prefixed columns,
//! UTF-16BE character payloads, NULL `0xFFFE`, end-of-table `0xFFFF`.
//! This probe does not run `imp`, does not connect to Oracle, and does
//! not write `src/publish` or the published XML.
//!
//! ```powershell
//! cargo run --example probe_oracle_exp_small_classes
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::BytesStart;

const DEFAULT_DUMP: &str = r"D:\work\cad\pid-test-data\Export.dmp";
const DEFAULT_REFERENCE: &str =
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图_Data.xml";
const DEFAULT_PUBLISH: &str =
    r"D:\work\cad\PID自研软件验证\publish\I井场 注采阀组工艺及自控流程图_Data.xml";

const NOZZLE_TABLES: &[&str] = &["T_NOZZLE"];
const NOTE_TABLES: &[&str] = &["T_ITEMNOTE", "T_NOTE"];
const OPC_TABLES: &[&str] = &["T_OPC"];

fn main() {
    if let Err(err) = run() {
        eprintln!("probe_oracle_exp_small_classes: {err}");
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
    let publish_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_PUBLISH), PathBuf::from);

    let dump =
        std::fs::read(&dump_path).map_err(|err| format!("read {}: {err}", dump_path.display()))?;
    println!(
        "dump {} bytes ({:.2} MB)",
        dump.len(),
        dump.len() as f64 / 1_048_576.0
    );

    let apps = read_table(&dump, "APPLICATIONS")?;
    pin_applications(&apps)?;
    let mut tables = BTreeMap::new();
    for name in NOZZLE_TABLES
        .iter()
        .chain(NOTE_TABLES.iter())
        .chain(OPC_TABLES.iter())
    {
        let table = read_table(&dump, name)?;
        report_table(&table);
        tables.insert((*name).to_string(), table);
    }

    let reference = read_xml(&reference_path)?;
    let publish = read_xml(&publish_path)?;
    println!(
        "reference DocUID={} objects={} rels={}",
        reference.doc_uid,
        reference.objects.len(),
        reference.rels.len()
    );
    println!(
        "publish_output objects={} rels={}",
        publish.objects.len(),
        publish.rels.len()
    );

    let nozzle_rels = rels_of(&reference, "EquipmentComponentComposition");
    print_rel_classes("EquipmentComponentComposition", &nozzle_rels, &reference);
    score_rels(
        "nozzle",
        &nozzle_rels,
        &reference,
        tables.get("T_NOZZLE").expect("T_NOZZLE"),
        "SP_ID",
        "SP_EQUIPMENTID",
    );

    let note_rels = rels_of(&reference, "PBSItemNotes");
    print_rel_classes("PBSItemNotes", &note_rels, &reference);
    score_rels(
        "itemnote",
        &note_rels,
        &reference,
        tables.get("T_ITEMNOTE").expect("T_ITEMNOTE"),
        "SP_ID",
        "SP_ID",
    );
    score_rels(
        "note",
        &note_rels,
        &reference,
        tables.get("T_NOTE").expect("T_NOTE"),
        "SP_ITEMNOTEID",
        "SP_MODELITEMID",
    );
    report_drawing_filter(tables.get("T_NOTE").expect("T_NOTE"), &reference.doc_uid);

    let branch_points = objects_of(&reference, "PIDBranchPoint");
    let published_branch_points = objects_of(&publish, "PIDBranchPoint");
    println!(
        "PIDBranchPoint reference={} publish={}",
        branch_points.len(),
        published_branch_points.len()
    );
    score_uids(
        "opc_branch",
        &branch_points,
        &reference,
        tables.get("T_OPC").expect("T_OPC"),
    );
    for (uid, _) in &branch_points {
        print_partner(
            tables.get("T_OPC").expect("T_OPC"),
            uid,
            "SP_ID",
            "SP_PAIREDWITHID",
        );
    }

    let drawing_ref = rels_of(&reference, "DrawingItems");
    let drawing_pub = rels_of(&publish, "DrawingItems");
    let published_pairs: HashSet<(String, String)> = drawing_pub
        .iter()
        .map(|rel| (rel.uid1.clone(), rel.uid2.clone()))
        .collect();
    let missing: Vec<&Rel> = drawing_ref
        .iter()
        .copied()
        .filter(|rel| !published_pairs.contains(&(rel.uid1.clone(), rel.uid2.clone())))
        .collect();
    println!(
        "DrawingItems reference={} publish={} missing_pairs={}",
        drawing_ref.len(),
        drawing_pub.len(),
        missing.len()
    );
    let mut missing_uids = Vec::new();
    for rel in &missing {
        let left = class_of(&reference, &rel.uid1);
        let right = class_of(&reference, &rel.uid2);
        println!(
            "  missing_drawing {left} {} -> {right} {}",
            rel.uid1, rel.uid2
        );
        missing_uids.push((rel.uid1.clone(), left));
        missing_uids.push((rel.uid2.clone(), right));
    }
    score_labeled_uids(
        "opc_drawing_gap",
        &missing_uids,
        tables.get("T_OPC").expect("T_OPC"),
    );
    let missing_owned = missing.into_iter().cloned().collect::<Vec<_>>();
    let missing_refs = missing_owned.iter().collect::<Vec<_>>();
    score_rels(
        "opc_drawing_rel",
        &missing_refs,
        &reference,
        tables.get("T_OPC").expect("T_OPC"),
        "SP_ID",
        "SP_PAIREDWITHID",
    );
    Ok(())
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
    /// Oracle `DATE`. The header carries a 7-byte maximum, like a number.
    Date,
}

#[derive(Clone)]
enum Field {
    Null,
    Int(i64),
    Text(String),
    Unscaled,
}

struct XmlDoc {
    doc_uid: String,
    objects: HashMap<String, String>,
    rels: Vec<Rel>,
}

#[derive(Clone)]
struct Rel {
    def: String,
    uid1: String,
    uid2: String,
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

fn report_table(table: &Table) {
    println!(
        "{} rows={} cols={} tail={}",
        table.name,
        table.rows.len(),
        table.columns.len(),
        table.tail
    );
    for column in &table.columns {
        let non_null = table
            .rows
            .iter()
            .filter(|row| {
                let index = column_index(table, &column.name).expect("column");
                !matches!(row.get(index), Some(Field::Null) | None)
            })
            .count();
        match column.kind {
            Kind::Char => println!(
                "  col {} char maxlen={} charset={:?} form={:?} non_null={non_null}",
                column.name, column.maxlen, column.charset, column.form
            ),
            Kind::Num => println!(
                "  col {} number maxlen={} non_null={non_null}",
                column.name, column.maxlen
            ),
            Kind::Date => println!(
                "  col {} date maxlen={} non_null={non_null}",
                column.name, column.maxlen
            ),
        }
    }
}

fn report_drawing_filter(table: &Table, doc_uid: &str) {
    if column_index(table, "SP_DRAWINGID").is_none() {
        println!("{} has no SP_DRAWINGID", table.name);
        return;
    }
    let mut drawings: BTreeMap<String, usize> = BTreeMap::new();
    let mut empty = 0usize;
    for row in &table.rows {
        let drawing = text_at(row, &table.columns, "SP_DRAWINGID");
        if drawing.is_empty() {
            empty += 1;
            continue;
        }
        *drawings.entry(drawing).or_default() += 1;
    }
    println!(
        "{} distinct_SP_DRAWINGID={} empty_SP_DRAWINGID={empty} doc_rows={} doc_uid_present={}",
        table.name,
        drawings.len(),
        drawings.get(doc_uid).copied().unwrap_or(0),
        drawings.contains_key(doc_uid)
    );
}

fn print_rel_classes(def: &str, rels: &[&Rel], doc: &XmlDoc) {
    let mut classes: BTreeMap<(String, String), usize> = BTreeMap::new();
    for rel in rels {
        *classes
            .entry((class_of(doc, &rel.uid1), class_of(doc, &rel.uid2)))
            .or_default() += 1;
    }
    println!("category {def} reference={}", rels.len());
    for ((left, right), count) in classes {
        println!("  ref_ends {left} -> {right} {count}");
    }
}

fn score_rels(
    label: &str,
    rels: &[&Rel],
    doc: &XmlDoc,
    table: &Table,
    left_col: &str,
    right_col: &str,
) {
    let index = IdIndex::build(table);
    let ordered = rels
        .iter()
        .filter(|rel| index.same_row(&rel.uid1, left_col, &rel.uid2, right_col))
        .count();
    let swapped = if left_col == right_col {
        0
    } else {
        rels.iter()
            .filter(|rel| index.same_row(&rel.uid2, left_col, &rel.uid1, right_col))
            .count()
    };
    let both_anywhere = rels
        .iter()
        .filter(|rel| index.contains(&rel.uid1) && index.contains(&rel.uid2))
        .count();
    let table_name = &table.name;
    let rel_count = rels.len();
    println!(
        "score {label} table={table_name} rels={rel_count} pair={left_col}->{right_col} same_row={ordered} swapped={swapped} both_anywhere={both_anywhere}"
    );
    let best = index.best_pair(rels);
    if let Some((left, right, count, swapped_count)) = best {
        println!("  best_pair {left}->{right} same_row={count} swapped={swapped_count}");
    } else {
        println!("  best_pair none");
    }
    for rel in rels {
        println!(
            "  rel {} {} [{}] -> {} {} [{}]",
            class_of(doc, &rel.uid1),
            rel.uid1,
            index.columns_of(&rel.uid1),
            class_of(doc, &rel.uid2),
            rel.uid2,
            index.columns_of(&rel.uid2)
        );
    }
}

fn score_uids(label: &str, uids: &[(String, String)], doc: &XmlDoc, table: &Table) {
    let labeled = uids
        .iter()
        .map(|(uid, class)| (uid.clone(), class.clone()))
        .collect::<Vec<_>>();
    let _ = doc;
    score_labeled_uids(label, &labeled, table);
}

fn score_labeled_uids(label: &str, uids: &[(String, String)], table: &Table) {
    let index = IdIndex::build(table);
    let hits = uids.iter().filter(|(uid, _)| index.contains(uid)).count();
    let table_name = &table.name;
    let uid_count = uids.len();
    println!("score {label} table={table_name} uids={uid_count} id_cell_hits={hits}");
    for (uid, class) in uids {
        println!("  uid {class} {uid} [{}]", index.columns_of(uid));
    }
}

fn print_partner(table: &Table, uid: &str, id_col: &str, partner_col: &str) {
    let mut found = 0usize;
    for row in &table.rows {
        let id = text_at(row, &table.columns, id_col);
        if !id.eq_ignore_ascii_case(uid) {
            continue;
        }
        found += 1;
        let partner = text_at(row, &table.columns, partner_col);
        println!("  partner {id_col}={uid} {partner_col}={partner}");
    }
    if found == 0 {
        println!("  partner {id_col}={uid} rows=0");
    }
}

fn objects_of(doc: &XmlDoc, class: &str) -> Vec<(String, String)> {
    let mut found = doc
        .objects
        .iter()
        .filter(|(_, kind)| kind.as_str() == class)
        .map(|(uid, kind)| (uid.clone(), kind.clone()))
        .collect::<Vec<_>>();
    found.sort();
    found
}

fn rels_of<'a>(doc: &'a XmlDoc, def: &str) -> Vec<&'a Rel> {
    doc.rels.iter().filter(|rel| rel.def == def).collect()
}

fn class_of(doc: &XmlDoc, uid: &str) -> String {
    doc.objects.get(uid).cloned().unwrap_or_else(|| "?".into())
}

struct IdIndex {
    /// Uppercased cell -> column name -> row indexes.
    hits: HashMap<String, HashMap<String, HashSet<usize>>>,
}

impl IdIndex {
    fn build(table: &Table) -> Self {
        let mut hits: HashMap<String, HashMap<String, HashSet<usize>>> = HashMap::new();
        for (row_index, row) in table.rows.iter().enumerate() {
            for (column_index, column) in table.columns.iter().enumerate() {
                if !is_id_column(column) {
                    continue;
                }
                let Some(text) = id_text(row.get(column_index)) else {
                    continue;
                };
                hits.entry(text.to_ascii_uppercase())
                    .or_default()
                    .entry(column.name.clone())
                    .or_default()
                    .insert(row_index);
            }
        }
        Self { hits }
    }

    fn contains(&self, uid: &str) -> bool {
        self.hits.contains_key(&uid.to_ascii_uppercase())
    }

    fn columns_of(&self, uid: &str) -> String {
        let Some(columns) = self.hits.get(&uid.to_ascii_uppercase()) else {
            return "0".into();
        };
        let mut names = columns.keys().cloned().collect::<Vec<_>>();
        names.sort();
        if names.is_empty() {
            "0".into()
        } else {
            names.join(",")
        }
    }

    fn rows(&self, uid: &str, column: &str) -> Option<&HashSet<usize>> {
        self.hits
            .get(&uid.to_ascii_uppercase())
            .and_then(|columns| columns.get(column))
    }

    fn same_row(&self, uid_left: &str, left_col: &str, uid_right: &str, right_col: &str) -> bool {
        let Some(left) = self.rows(uid_left, left_col) else {
            return false;
        };
        let Some(right) = self.rows(uid_right, right_col) else {
            return false;
        };
        left.intersection(right).next().is_some()
    }

    fn best_pair(&self, rels: &[&Rel]) -> Option<(String, String, usize, usize)> {
        let mut names = BTreeMap::<String, ()>::new();
        for columns in self.hits.values() {
            for name in columns.keys() {
                names.insert(name.clone(), ());
            }
        }
        let names = names.into_keys().collect::<Vec<_>>();
        let mut best: Option<(String, String, usize, usize)> = None;
        for left in &names {
            for right in &names {
                if left == right {
                    continue;
                }
                let same = rels
                    .iter()
                    .filter(|rel| self.same_row(&rel.uid1, left, &rel.uid2, right))
                    .count();
                let swapped = rels
                    .iter()
                    .filter(|rel| self.same_row(&rel.uid2, left, &rel.uid1, right))
                    .count();
                let rank = same.max(swapped);
                let replace = match &best {
                    None => rank > 0,
                    Some((_, _, best_same, best_swapped)) => {
                        let best_rank = (*best_same).max(*best_swapped);
                        rank > best_rank || (rank == best_rank && same > *best_same)
                    }
                };
                if replace {
                    best = Some((left.clone(), right.clone(), same, swapped));
                }
            }
        }
        best
    }
}

fn is_id_column(column: &Column) -> bool {
    column.kind == Kind::Char && column.form == Some(2) && column.name.contains("ID")
}

fn id_text(field: Option<&Field>) -> Option<String> {
    match field? {
        Field::Text(text) if !text.is_empty() => Some(text.clone()),
        Field::Int(value) => Some(value.to_string()),
        _ => None,
    }
}

fn column_index(table: &Table, name: &str) -> Option<usize> {
    table.columns.iter().position(|column| column.name == name)
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
    match row.get(index) {
        Some(Field::Int(value)) => Some(*value),
        _ => None,
    }
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
            if len % 2 != 0 {
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
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
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
    assert_eq!(decode_i64(&[0xc2, 0x1a, 0x34]), Some(2551));
    assert_eq!(decode_i64(&[0x3e, 0x64, 0x66]), Some(-1));
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
        objects: HashMap::new(),
        rels: Vec::new(),
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
                    finish(&tag, &values, &mut doc);
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
            *doc_uid = value;
        }
    }
}

fn take(start: &BytesStart<'_>, values: &mut BTreeMap<String, String>) {
    let interface = local_name(start);
    for (name, value) in attributes_of(start) {
        values.insert(format!("{interface}.{name}"), value);
    }
}

fn finish(tag: &str, values: &BTreeMap<String, String>, doc: &mut XmlDoc) {
    if tag == "Rel" {
        doc.rels.push(Rel {
            def: values.get("IRel.DefUID").cloned().unwrap_or_default(),
            uid1: values.get("IRel.UID1").cloned().unwrap_or_default(),
            uid2: values.get("IRel.UID2").cloned().unwrap_or_default(),
        });
        return;
    }
    let Some(uid) = values.get("IObject.UID") else {
        return;
    };
    doc.objects.insert(uid.clone(), tag.to_string());
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
