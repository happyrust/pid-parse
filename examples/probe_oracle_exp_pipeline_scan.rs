//! Search every identifier column of every table in an Oracle `exp` dump
//! for I井场 `PIDPipeline` UIDs, and ask whether each `PipingConnectors`
//! pair sits on one row.
//!
//! Row decoding is copied from `examples/probe_oracle_exp_rows.rs`.
//! Three column kinds that probe never met are added here so every table
//! can be read: type 12 (DATE: one extra header `u16`, cell still a `u16`
//! length), type 113 (LONG/BLOB: a short piece in column order, then the
//! rest of the payload after the row), and type `0xA0` (a character
//! column whose header is charset + form, with no separate max length).
//!
//! Identifier columns are character columns whose names contain `ID`.
//! A cell matches after braces, hyphens and whitespace are removed, the
//! rest is uppercased, and the result is 32 hex digits.
//!
//! This probe does not run `imp`, does not connect to Oracle, and does
//! not write `src/` or `publish/`.
//!
//! ```powershell
//! cargo run --example probe_oracle_exp_pipeline_scan
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use quick_xml::events::BytesStart;
use quick_xml::Reader;

const DEFAULT_DUMP: &str = r"D:\work\cad\pid-test-data\Export.dmp";
const DEFAULT_REFERENCE: &str =
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图_Data.xml";

fn main() {
    if let Err(err) = run() {
        eprintln!("probe_oracle_exp_pipeline_scan: {err}");
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

    let reference = read_xml(&reference_path)?;
    if reference.pipelines.len() != 79 || reference.rels.len() != 106 {
        return Err(format!(
            "reference pipelines={} rels={}, expected 79 and 106",
            reference.pipelines.len(),
            reference.rels.len()
        ));
    }
    println!(
        "reference DocUID={} pipelines={} piping_connectors={}",
        reference.doc_uid,
        reference.pipelines.len(),
        reference.rels.len()
    );

    let sites = insert_sites(&dump);
    if sites.is_empty() {
        return Err("no INSERT INTO statements".into());
    }
    let apps = sites
        .iter()
        .find(|(name, _)| name == "APPLICATIONS")
        .ok_or("APPLICATIONS insert not found")?;
    let apps_table = read_table_at(&dump, &apps.0, apps.1)?;
    pin_applications(&apps_table)?;

    let pipeline_ids: HashSet<&str> = reference.pipelines.keys().map(String::as_str).collect();
    let rel_uid2: HashSet<&str> = reference.rels.iter().map(|rel| rel.uid2.as_str()).collect();
    if pipeline_ids != rel_uid2 {
        return Err("PIDPipeline UIDs are not exactly PipingConnectors UID2".into());
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut hits: BTreeMap<(String, String), ColHit> = BTreeMap::new();
    let mut pair_tables: BTreeMap<String, PairHit> = BTreeMap::new();
    let mut id_columns = 0usize;
    let mut id_form2 = 0usize;
    let mut piperun_uid1 = 0usize;
    let mut unparsed: Vec<String> = Vec::new();
    let uid1s: HashSet<&str> = reference.rels.iter().map(|rel| rel.uid1.as_str()).collect();

    for (label, at) in &sites {
        let table = match read_table_at(&dump, label, *at) {
            Ok(table) => table,
            Err(err) => {
                println!("unparsed {label}: {err}");
                unparsed.push(label.clone());
                continue;
            }
        };
        let id_cols: Vec<usize> = table
            .columns
            .iter()
            .enumerate()
            .filter(|(_, column)| column.kind == Kind::Char && is_id_column(&column.name))
            .map(|(index, _)| index)
            .collect();
        id_columns += id_cols.len();
        for index in &id_cols {
            let column = &table.columns[*index];
            if column.form == Some(2) {
                id_form2 += 1;
            } else {
                println!(
                    "id_column_other {} {} form={:?} charset={:?}",
                    table.name, column.name, column.form, column.charset
                );
            }
        }
        if table.name == "T_DRAWINGVERSION" {
            let project = table
                .columns
                .iter()
                .position(|column| column.name == "SP_PROJECTNAME");
            let non_null = project.map_or(0, |index| {
                table
                    .rows
                    .iter()
                    .filter(|row| matches!(row[index], Field::Text(_)))
                    .count()
            });
            let column = project.map(|index| &table.columns[index]);
            println!(
                "T_DRAWINGVERSION rows={} SP_PROJECTNAME non_null={non_null} charset={:?} form={:?}",
                table.rows.len(),
                column.and_then(|column| column.charset),
                column.and_then(|column| column.form)
            );
        }
        if table.name == "T_PIPERUN" {
            let sp = table
                .columns
                .iter()
                .position(|column| column.name == "SP_ID")
                .ok_or("T_PIPERUN has no SP_ID")?;
            let mut found = HashSet::new();
            for row in &table.rows {
                if let Field::Text(text) = &row[sp] {
                    if let Some(uid) = norm_uid(text) {
                        if uid1s.contains(uid.as_str()) {
                            found.insert(uid);
                        }
                    }
                }
            }
            piperun_uid1 = found.len();
        }
        scan_ids(
            &table,
            &id_cols,
            &pipeline_ids,
            &reference.rels,
            &mut seen,
            &mut hits,
            &mut pair_tables,
        );
    }

    let insert_names = sites
        .iter()
        .filter(|(label, _)| !label.contains('#'))
        .count();
    println!(
        "scan inserts={} insert_names={insert_names} parsed={} unparsed={} id_columns={} id_columns_form2={}",
        sites.len(),
        sites.len() - unparsed.len(),
        unparsed.len(),
        id_columns,
        id_form2
    );
    if !unparsed.is_empty() {
        println!("unparsed_tables {}", unparsed.join(","));
    }
    println!(
        "pipeline_uid hit={} miss={} of={}",
        seen.len(),
        pipeline_ids.len() - seen.len(),
        pipeline_ids.len()
    );
    if hits.is_empty() {
        println!("pipeline_uid columns=0");
    }
    for ((table, column), hit) in &hits {
        println!(
            "pipeline_uid {table} col {column} uids={} rows={}",
            hit.uids.len(),
            hit.rows
        );
    }
    let mut ordered = 0usize;
    let mut swapped = 0usize;
    let mut either = 0usize;
    let mut ordered_rels = HashSet::new();
    let mut swapped_rels = HashSet::new();
    let mut either_rels = HashSet::new();
    if pair_tables.is_empty() {
        println!("pairs tables=0");
    }
    for (table, hit) in &pair_tables {
        ordered += hit.ordered.len();
        swapped += hit.swapped.len();
        either += hit.either.len();
        ordered_rels.extend(hit.ordered.iter().copied());
        swapped_rels.extend(hit.swapped.iter().copied());
        either_rels.extend(hit.either.iter().copied());
        println!(
            "pairs {table} same_row_ordered={} same_row_swapped={} same_row_either={} of={}",
            hit.ordered.len(),
            hit.swapped.len(),
            hit.either.len(),
            reference.rels.len()
        );
        for ((left, right), count) in &hit.columns {
            println!("  pairs {table} {left} + {right} rows={count}");
        }
    }
    println!(
        "pairs any same_row_ordered={} same_row_swapped={} same_row_either={} of={}",
        ordered_rels.len(),
        swapped_rels.len(),
        either_rels.len(),
        reference.rels.len()
    );
    println!("pairs sum_tables ordered={ordered} swapped={swapped} either={either}");
    println!(
        "sanity T_PIPERUN.SP_ID rel_uid1={piperun_uid1}/{}",
        uid1s.len()
    );
    let byte_scan = byte_scan_uids(&dump, reference.pipelines.keys().map(String::as_str));
    println!(
        "byte_scan utf16be={} ascii={} raw16={} of={}",
        byte_scan.utf16be,
        byte_scan.ascii,
        byte_scan.raw16,
        pipeline_ids.len()
    );
    if piperun_uid1 != uid1s.len() {
        return Err(format!(
            "decoder pin failed: T_PIPERUN.SP_ID has {piperun_uid1} of {} PipingConnectors UID1",
            uid1s.len()
        ));
    }
    if seen.is_empty() && either_rels.is_empty() {
        println!("conclusion dump_has_pipeline=false");
    } else {
        println!("conclusion dump_has_pipeline=true");
    }
    Ok(())
}

struct ColHit {
    uids: BTreeSet<String>,
    rows: usize,
}

struct PairHit {
    ordered: HashSet<usize>,
    swapped: HashSet<usize>,
    either: HashSet<usize>,
    columns: BTreeMap<(String, String), usize>,
}

struct Cell {
    index: usize,
    name: String,
    uid: String,
}

fn scan_ids(
    table: &Table,
    id_cols: &[usize],
    pipelines: &HashSet<&str>,
    rels: &[Rel],
    seen: &mut HashSet<String>,
    hits: &mut BTreeMap<(String, String), ColHit>,
    pairs: &mut BTreeMap<String, PairHit>,
) {
    for row in &table.rows {
        let mut cells = Vec::new();
        for index in id_cols {
            let Field::Text(text) = &row[*index] else {
                continue;
            };
            let Some(uid) = norm_uid(text) else {
                continue;
            };
            if pipelines.contains(uid.as_str()) {
                seen.insert(uid.clone());
                let hit = hits
                    .entry((table.name.clone(), table.columns[*index].name.clone()))
                    .or_insert_with(|| ColHit {
                        uids: BTreeSet::new(),
                        rows: 0,
                    });
                hit.uids.insert(uid.clone());
                hit.rows += 1;
            }
            cells.push(Cell {
                index: *index,
                name: table.columns[*index].name.clone(),
                uid,
            });
        }
        if cells.len() < 2
            || !cells
                .iter()
                .any(|cell| pipelines.contains(cell.uid.as_str()))
        {
            continue;
        }
        for (rel_index, rel) in rels.iter().enumerate() {
            let mut ordered = false;
            let mut swapped = false;
            let mut ordered_cols: Option<(String, String)> = None;
            let mut swapped_cols: Option<(String, String)> = None;
            for left in &cells {
                for right in &cells {
                    if left.index >= right.index {
                        continue;
                    }
                    if left.uid == rel.uid1 && right.uid == rel.uid2 {
                        ordered = true;
                        ordered_cols = Some((left.name.clone(), right.name.clone()));
                    } else if left.uid == rel.uid2 && right.uid == rel.uid1 {
                        swapped = true;
                        swapped_cols = Some((left.name.clone(), right.name.clone()));
                    }
                }
            }
            if !ordered && !swapped {
                continue;
            }
            let hit = pairs.entry(table.name.clone()).or_insert_with(|| PairHit {
                ordered: HashSet::new(),
                swapped: HashSet::new(),
                either: HashSet::new(),
                columns: BTreeMap::new(),
            });
            if ordered {
                hit.ordered.insert(rel_index);
            }
            if swapped {
                hit.swapped.insert(rel_index);
            }
            hit.either.insert(rel_index);
            if let Some(cols) = ordered_cols.or(swapped_cols) {
                *hit.columns.entry(cols).or_default() += 1;
            }
        }
    }
}

fn is_id_column(name: &str) -> bool {
    name.to_ascii_uppercase().contains("ID")
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
            "APPLICATIONS row 0 is not (1, SmartPlant PID, SPPID); refusing to scan".into(),
        );
    }
    println!("format_pin ok tail={}", table.tail);
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
    Date,
    Long,
}

#[derive(Clone)]
enum Field {
    Null,
    Int(i64),
    Text(String),
    /// Non-null number that is fractional or does not fit in `i64`.
    Unscaled,
    Skip,
}

struct XmlDoc {
    doc_uid: String,
    pipelines: HashMap<String, String>,
    rels: Vec<Rel>,
}

struct Rel {
    uid1: String,
    uid2: String,
}

fn insert_sites(dump: &[u8]) -> Vec<(String, usize)> {
    let marker = b"INSERT INTO \"";
    let mut sites = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut from = 0usize;
    while let Some(rel) = find(&dump[from..], marker) {
        let at = from + rel;
        from = at + marker.len();
        if at > 0 && dump[at - 1] != b'\n' {
            continue;
        }
        let name_at = at + marker.len();
        let Some(end) = dump[name_at..].iter().position(|byte| *byte == b'"') else {
            continue;
        };
        let Ok(raw) = std::str::from_utf8(&dump[name_at..name_at + end]) else {
            continue;
        };
        let n = seen.entry(raw.to_string()).or_insert(0);
        *n += 1;
        let label = if *n == 1 {
            raw.to_string()
        } else {
            format!("{raw}#{}", *n)
        };
        sites.push((label, at));
    }
    sites
}

fn text_at(row: &[Field], columns: &[Column], name: &str) -> String {
    match field_at(row, columns, name) {
        Field::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn int_at(row: &[Field], columns: &[Column], name: &str) -> Option<i64> {
    match field_at(row, columns, name) {
        Field::Int(value) => Some(*value),
        _ => None,
    }
}

fn field_at<'a>(row: &'a [Field], columns: &[Column], name: &str) -> &'a Field {
    let index = columns
        .iter()
        .position(|column| column.name == name)
        .unwrap_or_else(|| panic!("column {name}"));
    &row[index]
}

fn read_table_at(dump: &[u8], name: &str, at: usize) -> Result<Table, String> {
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
    let has_long = columns.iter().any(|column| column.kind == Kind::Long);
    if has_long {
        pos = skip_long_preamble(dump, pos, &columns).map_err(|err| format!("{name}: {err}"))?;
    } else if dump.get(pos..pos + 4) == Some(&[0, 0, 0, 0]) {
        pos += 4;
    } else {
        return Err(format!("{name}: column header terminator at {pos:#x}"));
    }
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
        let mut long_present = false;
        for column in &columns {
            let (field, next) = read_field(dump, pos, column)
                .map_err(|err| format!("{name}: row {} {err}", rows.len()))?;
            if column.kind == Kind::Long && matches!(field, Field::Skip) {
                long_present = true;
            }
            pos = next;
            row.push(field);
        }
        pos = finish_row(dump, pos, long_present)
            .map_err(|err| format!("{name}: row {} {err}", rows.len()))?;
        rows.push(row);
    }
}

fn skip_long_preamble(dump: &[u8], pos: usize, columns: &[Column]) -> Result<usize, String> {
    if dump.get(pos..pos + 4) == Some(&[0, 0, 0, 0]) {
        return skip_long_descriptor(dump, pos + 4);
    }
    if pos + 2 <= dump.len() && u16(dump, pos) == columns.len() as u16 {
        let name = columns
            .iter()
            .find(|column| column.kind == Kind::Long)
            .map(|column| column.name.as_str())
            .ok_or("long column missing")?;
        return skip_named_padding(dump, pos + 2, name);
    }
    Err(format!("column header terminator at {pos:#x}"))
}

fn skip_named_padding(dump: &[u8], pos: usize, name: &str) -> Result<usize, String> {
    let window = dump.get(pos..dump.len().min(pos + 80)).unwrap_or_default();
    let rel =
        find(window, name.as_bytes()).ok_or_else(|| format!("long name {name} not in preamble"))?;
    let mut at = pos + rel + name.len();
    while dump.get(at) == Some(&0) {
        at += 1;
    }
    Ok(at)
}

fn skip_long_descriptor(dump: &[u8], mut pos: usize) -> Result<usize, String> {
    if pos + 4 > dump.len() {
        return Err("truncated long descriptor".into());
    }
    let pre = u32_at(dump, pos);
    pos += 4;
    if pre != 0 {
        return Err(format!("long descriptor prefix {pre:#x} at {pos:#x}"));
    }
    if pos >= dump.len() {
        return Err("truncated long name".into());
    }
    let name_len = dump[pos] as usize;
    pos += 1 + name_len;
    if pos + 2 > dump.len() {
        return Err("truncated long name tail".into());
    }
    pos += 2;
    Ok(pos)
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
            113 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                let _flag = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Long,
                    maxlen,
                    charset: None,
                    form: None,
                }
            }
            0xA0 => {
                let charset = u16(dump, pos);
                pos += 2;
                let form = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Char,
                    maxlen: 4000,
                    charset: Some(charset),
                    form: Some(form),
                }
            }
            other => return Err(format!("column {name} type {other:#x} at {pos:#x}")),
        };
        columns.push(column);
    }
    Ok((columns, pos))
}

fn read_field(dump: &[u8], pos: usize, column: &Column) -> Result<(Field, usize), String> {
    if column.kind == Kind::Long {
        return read_long(dump, pos);
    }
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
        Kind::Char => {
            if column.form == Some(2) {
                if !len.is_multiple_of(2) {
                    return Err(format!("odd UTF-16 length {len}"));
                }
                Field::Text(decode_utf16_be(raw))
            } else {
                Field::Text(raw.iter().map(|byte| char::from(*byte)).collect())
            }
        }
        Kind::Num => match decode_i64(raw) {
            Some(value) => Field::Int(value),
            None => Field::Unscaled,
        },
        Kind::Date => Field::Skip,
        Kind::Long => Field::Skip,
    };
    Ok((field, next))
}

fn read_long(dump: &[u8], mut pos: usize) -> Result<(Field, usize), String> {
    if pos + 2 > dump.len() {
        return Err(format!("truncated long at {pos:#x}"));
    }
    let first = u16(dump, pos) as usize;
    pos += 2;
    if first == 0xFFFE {
        return Ok((Field::Null, pos));
    }
    if pos + first > dump.len() {
        return Err(format!("long piece {first} at {pos:#x}"));
    }
    pos += first;
    Ok((Field::Skip, pos))
}

fn finish_row(dump: &[u8], pos: usize, long_present: bool) -> Result<usize, String> {
    if long_present && dump.get(pos..pos + 4) == Some(&[0x01, 0x00, 0xfc, 0xff]) {
        let zip_at = pos + 16;
        let mut end = zip_end(dump, zip_at)?;
        if dump.get(end) == Some(&0) {
            end += 1;
        }
        return Ok(end);
    }
    if long_present && dump.get(pos..pos + 2) == Some(&[0x01, 0x00]) {
        if pos + 4 > dump.len() {
            return Err(format!("truncated long tail at {pos:#x}"));
        }
        let second = u16(dump, pos + 2) as usize;
        let next = pos + 4 + second;
        if next > dump.len() {
            return Err(format!("long payload {second} at {pos:#x}"));
        }
        return Ok(next);
    }
    if dump.get(pos..pos + 2) == Some(&[0, 0]) {
        return Ok(pos + 2);
    }
    Err(format!(
        "separator is {:02x?} at {pos:#x}",
        dump.get(pos..pos + 2)
    ))
}

fn zip_end(dump: &[u8], zip_at: usize) -> Result<usize, String> {
    let sig = b"PK\x05\x06";
    let rel = find(dump.get(zip_at..).unwrap_or_default(), sig)
        .ok_or_else(|| format!("long payload has no zip end at {zip_at:#x}"))?;
    let end_rec = zip_at + rel;
    if end_rec + 22 > dump.len() {
        return Err(format!("truncated zip end at {end_rec:#x}"));
    }
    let comment = u16(dump, end_rec + 20) as usize;
    let end = end_rec + 22 + comment;
    if end > dump.len() {
        return Err(format!("zip comment runs past the dump at {end_rec:#x}"));
    }
    Ok(end)
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

fn u32_at(dump: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes([dump[pos], dump[pos + 1], dump[pos + 2], dump[pos + 3]])
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

fn byte_scan_uids<'a>(dump: &[u8], uids: impl Iterator<Item = &'a str>) -> ByteScan {
    let mut scan = ByteScan::default();
    for uid in uids {
        if contains_bytes(dump, &utf16_be(uid)) {
            scan.utf16be += 1;
        }
        if contains_bytes(dump, uid.as_bytes()) {
            scan.ascii += 1;
        }
        if let Some(raw) = hex_bytes(uid) {
            if contains_bytes(dump, &raw) {
                scan.raw16 += 1;
            }
        }
    }
    scan
}

#[derive(Default)]
struct ByteScan {
    utf16be: usize,
    ascii: usize,
    raw16: usize,
}

fn utf16_be(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        out.push(0);
        out.push(byte);
    }
    out
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = hex_value(pair[0])?;
        let lo = hex_value(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn contains_bytes(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return needle.is_empty();
    }
    let last = needle.len() - 1;
    let mut skip = [needle.len(); 256];
    for (index, &byte) in needle[..last].iter().enumerate() {
        skip[byte as usize] = last - index;
    }
    let mut index = 0usize;
    while index + last < hay.len() {
        let mut matched = last;
        while hay[index + matched] == needle[matched] {
            if matched == 0 {
                return true;
            }
            matched -= 1;
        }
        index += skip[hay[index + last] as usize];
    }
    false
}

fn norm_uid(text: &str) -> Option<String> {
    let mut out = String::new();
    for ch in text.chars() {
        if ch == '{' || ch == '}' || ch == '-' || ch.is_whitespace() {
            continue;
        }
        if ch.is_ascii_hexdigit() {
            out.push(ch.to_ascii_uppercase());
        } else {
            return None;
        }
    }
    if out.len() == 32 {
        Some(out)
    } else {
        None
    }
}

fn read_xml(path: &Path) -> Result<XmlDoc, String> {
    let xml =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut doc = XmlDoc {
        doc_uid: String::new(),
        pipelines: HashMap::new(),
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
        if values.get("IRel.DefUID").map(String::as_str) == Some("PipingConnectors") {
            doc.rels.push(Rel {
                uid1: norm_uid(values.get("IRel.UID1").map(String::as_str).unwrap_or(""))
                    .unwrap_or_default(),
                uid2: norm_uid(values.get("IRel.UID2").map(String::as_str).unwrap_or(""))
                    .unwrap_or_default(),
            });
        }
        return;
    }
    if tag == "PIDPipeline" {
        if let Some(uid) = values.get("IObject.UID").and_then(|value| norm_uid(value)) {
            doc.pipelines
                .insert(uid, values.get("IObject.Name").cloned().unwrap_or_default());
        }
    }
}

fn attributes_of(start: &BytesStart<'_>) -> Vec<(String, String)> {
    start
        .attributes()
        .flatten()
        .map(|attribute| {
            (
                attribute.key.as_ref().to_string(),
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn local_name(start: &BytesStart<'_>) -> String {
    start.name().as_ref().to_string()
}
