//! Piping and signal port rows from an Oracle conventional `exp` dump.
//!
//! Plan `docs/plans/2026-10-12-n5-ports-from-oracle-exp.md` N5-1. The byte
//! layout is the one pinned by `examples/probe_oracle_exp_rows.rs` and
//! `docs/analysis/n5-exp-rows.md`: an ASCII `INSERT INTO` line, a binary
//! column header, length-prefixed fields, `0xFFFE` for NULL, two zero
//! bytes after each row, and `0xFFFF` at the end of the table. Character
//! payloads are UTF-16BE (charset id 2000). Numbers are Oracle `NUMBER`.
//!
//! `APPLICATIONS` is read first. Its first row must be `ID` 1,
//! `SmartPlant PID`, `SPPID`. If it is not, the port tables are not read.
//! This module does not run `imp`, does not connect to Oracle, and does
//! not write XML.

use std::collections::HashSet;
use std::fmt;

use super::pid_load::PidPublishDrawing;

const CHARSET_AL16UTF16: u16 = 2000;
const CHARSET_FORM: u16 = 2;

/// Why an export dump was not read.
#[derive(Debug)]
pub enum ExpError {
    /// `APPLICATIONS` row 0 is not `ID` 1, `SmartPlant PID`, `SPPID`.
    /// The port tables are not read.
    FormatUnpinned,
    /// `name` is not the pinned row format.
    Table {
        /// `APPLICATIONS`, `T_PIPINGPOINT`, or `T_SIGNALPOINT`.
        name: String,
        /// What was wrong.
        detail: String,
    },
}

impl fmt::Display for ExpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormatUnpinned => write!(
                f,
                "APPLICATIONS row 0 is not (1, SmartPlant PID, SPPID); refusing to read the port tables"
            ),
            Self::Table { name, detail } => write!(f, "{name}: {detail}"),
        }
    }
}

impl std::error::Error for ExpError {}

/// One `T_PIPINGPOINT` row with the columns N5 publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipingPoint {
    /// `SP_PLANTITEMID`.
    pub plant_item_id: String,
    /// `PIPINGPOINTNUMBER`.
    pub number: i64,
    /// `FLOWDIRECTION`, when the column is not NULL.
    pub flow_direction: Option<i64>,
    /// `NOMINALDIAMETER`, when the column is not NULL.
    pub nominal_diameter: Option<i64>,
}

impl PipingPoint {
    /// Parent UID, a dot, and the point number.
    #[must_use]
    pub fn uid(&self) -> String {
        format!("{}.{}", self.plant_item_id, self.number)
    }
}

/// One `T_SIGNALPOINT` row with the columns N5 publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPoint {
    /// `SP_PLANTITEMID`.
    pub plant_item_id: String,
    /// `SIGNALPOINTNUMBER`.
    pub number: i64,
}

impl SignalPoint {
    /// Parent UID, a dot, and the point number.
    #[must_use]
    pub fn uid(&self) -> String {
        format!("{}.{}", self.plant_item_id, self.number)
    }
}

/// The two port tables, after `APPLICATIONS` has pinned the row format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortTables {
    /// Parsed `T_PIPINGPOINT` rows, including ones that are not port rows.
    pub piping_row_count: usize,
    /// Parsed `T_SIGNALPOINT` rows, including ones that are not port rows.
    pub signal_row_count: usize,
    /// Rows with a plant-item id and an integer point number.
    pub piping_points: Vec<PipingPoint>,
    /// Rows with a plant-item id and an integer point number.
    pub signal_points: Vec<SignalPoint>,
}

/// Port rows whose parent is in the caller's set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawingPorts {
    /// Piping ports, in table order.
    pub piping: Vec<PipingPoint>,
    /// Signal ports, in table order.
    pub signal: Vec<SignalPoint>,
}

/// Read `T_PIPINGPOINT` and `T_SIGNALPOINT`.
///
/// `APPLICATIONS` is pinned first. A dump that fails the pin returns
/// [`ExpError::FormatUnpinned`] and the two port tables are not read.
///
/// # Errors
///
/// Returns [`ExpError::FormatUnpinned`] when the pin fails, and
/// [`ExpError::Table`] when a table's bytes are not the pinned format.
pub fn read_port_tables(dump: &[u8]) -> Result<PortTables, ExpError> {
    let applications = read_table(dump, "APPLICATIONS")?;
    if !applications_pinned(&applications) {
        return Err(ExpError::FormatUnpinned);
    }
    let piping = read_table(dump, "T_PIPINGPOINT")?;
    let signal = read_table(dump, "T_SIGNALPOINT")?;
    Ok(PortTables {
        piping_row_count: piping.rows.len(),
        signal_row_count: signal.rows.len(),
        piping_points: project_piping(&piping)?,
        signal_points: project_signal(&signal)?,
    })
}

/// UIDs [`super::pid_load::load_publish_from_pid`] emits as model items.
///
/// An item whose class the file does not decide is not emitted. Process
/// points and branch points are not plant-item parents, so they are not
/// in this set.
#[must_use]
pub fn published_parent_ids(drawing: &PidPublishDrawing) -> HashSet<String> {
    drawing
        .items
        .iter()
        .filter(|item| item.class.is_some())
        .map(|item| item.uid.clone())
        .collect()
}

/// Keep port rows whose `SP_PLANTITEMID` is in `parents`.
///
/// The set is the drawing's published item UIDs (Q2 a). This function
/// does not read a reference XML.
#[must_use]
pub fn ports_for_parents(tables: &PortTables, parents: &HashSet<String>) -> DrawingPorts {
    DrawingPorts {
        piping: tables
            .piping_points
            .iter()
            .filter(|point| parents.contains(&point.plant_item_id))
            .cloned()
            .collect(),
        signal: tables
            .signal_points
            .iter()
            .filter(|point| parents.contains(&point.plant_item_id))
            .cloned()
            .collect(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Char,
    Num,
}

struct Column {
    name: String,
    kind: Kind,
    maxlen: u16,
}

enum Field {
    Null,
    Int(i64),
    Text(String),
}

struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Field>>,
}

fn applications_pinned(table: &Table) -> bool {
    let Some(row) = table.rows.first() else {
        return false;
    };
    int_at(row, &table.columns, "ID") == Some(1)
        && text_at(row, &table.columns, "NAME").as_deref() == Some("SmartPlant PID")
        && text_at(row, &table.columns, "SHORTNAME").as_deref() == Some("SPPID")
}

fn project_piping(table: &Table) -> Result<Vec<PipingPoint>, ExpError> {
    let mut points = Vec::with_capacity(table.rows.len());
    for (index, row) in table.rows.iter().enumerate() {
        let plant_item_id = required_text("T_PIPINGPOINT", table, row, "SP_PLANTITEMID", index)?;
        let number = required_int("T_PIPINGPOINT", table, row, "PIPINGPOINTNUMBER", index)?;
        points.push(PipingPoint {
            plant_item_id,
            number,
            flow_direction: optional_int("T_PIPINGPOINT", table, row, "FLOWDIRECTION", index)?,
            nominal_diameter: optional_int("T_PIPINGPOINT", table, row, "NOMINALDIAMETER", index)?,
        });
    }
    Ok(points)
}

fn project_signal(table: &Table) -> Result<Vec<SignalPoint>, ExpError> {
    let mut points = Vec::with_capacity(table.rows.len());
    for (index, row) in table.rows.iter().enumerate() {
        points.push(SignalPoint {
            plant_item_id: required_text("T_SIGNALPOINT", table, row, "SP_PLANTITEMID", index)?,
            number: required_int("T_SIGNALPOINT", table, row, "SIGNALPOINTNUMBER", index)?,
        });
    }
    Ok(points)
}

fn required_text(
    table_name: &str,
    table: &Table,
    row: &[Field],
    name: &str,
    index: usize,
) -> Result<String, ExpError> {
    match text_at(row, &table.columns, name) {
        Some(text) if !text.is_empty() => Ok(text),
        _ => Err(table_error(
            table_name,
            format!("row {index} {name} is empty"),
        )),
    }
}

fn required_int(
    table_name: &str,
    table: &Table,
    row: &[Field],
    name: &str,
    index: usize,
) -> Result<i64, ExpError> {
    int_at(row, &table.columns, name)
        .ok_or_else(|| table_error(table_name, format!("row {index} {name} is not an integer")))
}

fn optional_int(
    table_name: &str,
    table: &Table,
    row: &[Field],
    name: &str,
    index: usize,
) -> Result<Option<i64>, ExpError> {
    match field_at(row, &table.columns, name) {
        Some(Field::Null) => Ok(None),
        Some(Field::Int(value)) => Ok(Some(*value)),
        _ => Err(table_error(
            table_name,
            format!("row {index} {name} is not an integer or NULL"),
        )),
    }
}

fn table_error(name: &str, detail: String) -> ExpError {
    ExpError::Table {
        name: name.to_string(),
        detail,
    }
}

fn int_at(row: &[Field], columns: &[Column], name: &str) -> Option<i64> {
    match field_at(row, columns, name)? {
        Field::Int(value) => Some(*value),
        Field::Null | Field::Text(_) => None,
    }
}

fn text_at(row: &[Field], columns: &[Column], name: &str) -> Option<String> {
    match field_at(row, columns, name)? {
        Field::Text(text) => Some(text.clone()),
        Field::Null => Some(String::new()),
        Field::Int(value) => Some(value.to_string()),
    }
}

fn field_at<'a>(row: &'a [Field], columns: &[Column], name: &str) -> Option<&'a Field> {
    let index = columns.iter().position(|column| column.name == name)?;
    row.get(index)
}

fn read_table(dump: &[u8], name: &str) -> Result<Table, ExpError> {
    let fail = |detail: String| ExpError::Table {
        name: name.to_string(),
        detail,
    };
    let marker = format!("INSERT INTO \"{name}\"");
    let at =
        find_slice(dump, marker.as_bytes()).ok_or_else(|| fail("INSERT not found".to_string()))?;
    let line_end = dump[at..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| at + offset)
        .ok_or_else(|| fail("INSERT line has no newline".to_string()))?;
    let line = std::str::from_utf8(&dump[at..line_end])
        .map_err(|_| fail("INSERT line is not ASCII".to_string()))?;
    let names = column_names(line).map_err(fail)?;
    let (columns, mut pos) = parse_header(dump, line_end + 1, &names).map_err(fail)?;
    let mut rows = Vec::new();
    loop {
        let peek = read_u16(dump, pos).map_err(fail)?;
        if peek == 0xFFFF {
            return Ok(Table { columns, rows });
        }
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            let (field, next) = read_field(dump, pos, column)
                .map_err(|detail| fail(format!("row {} {detail}", rows.len())))?;
            pos = next;
            row.push(field);
        }
        if dump.get(pos..pos + 2) != Some(&[0, 0]) {
            return Err(fail(format!(
                "row {} separator is {:02x?} at {pos:#x}",
                rows.len(),
                dump.get(pos..pos + 2)
            )));
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
            return Err("empty column name".to_string());
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
    let count = read_u16(dump, pos)? as usize;
    pos += 2;
    if count != names.len() {
        return Err(format!(
            "header columns {count} != INSERT names {}",
            names.len()
        ));
    }
    let mut columns = Vec::with_capacity(count);
    for name in names {
        let kind_code = read_u16(dump, pos)?;
        pos += 2;
        let (kind, maxlen) = match kind_code {
            1 => {
                let maxlen = read_u16(dump, pos)?;
                pos += 2;
                let charset = read_u16(dump, pos)?;
                pos += 2;
                let form = read_u16(dump, pos)?;
                pos += 2;
                if charset != CHARSET_AL16UTF16 || form != CHARSET_FORM {
                    return Err(format!(
                        "column {name} charset {charset} form {form}, want 2000/2"
                    ));
                }
                (Kind::Char, maxlen)
            }
            2 => {
                let maxlen = read_u16(dump, pos)?;
                pos += 2;
                (Kind::Num, maxlen)
            }
            other => return Err(format!("column {name} type {other:#x}")),
        };
        columns.push(Column {
            name: name.clone(),
            kind,
            maxlen,
        });
    }
    if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
        return Err(format!("column header terminator at {pos:#x}"));
    }
    Ok((columns, pos + 4))
}

fn read_field(dump: &[u8], pos: usize, column: &Column) -> Result<(Field, usize), String> {
    let len = read_u16(dump, pos)? as usize;
    let pos = pos + 2;
    if len == 0xFFFE {
        return Ok((Field::Null, pos));
    }
    if len > column.maxlen as usize + 8 {
        return Err(format!("length {len} at {pos:#x}"));
    }
    let raw = dump
        .get(pos..pos + len)
        .ok_or_else(|| format!("length {len} at {pos:#x}"))?;
    let next = pos + len;
    let field = match column.kind {
        Kind::Char => {
            if !len.is_multiple_of(2) {
                return Err(format!("odd UTF-16 length {len}"));
            }
            Field::Text(decode_utf16_be(raw))
        }
        Kind::Num => Field::Int(
            decode_i64(raw).ok_or_else(|| format!("NUMBER at {pos:#x} is not an integer"))?,
        ),
    };
    Ok((field, next))
}

fn decode_utf16_be(raw: &[u8]) -> String {
    let (chunks, _) = raw.as_chunks::<2>();
    let units = chunks
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

/// Oracle internal `NUMBER` to `i64`.
///
/// `80` is 0, `c1 02` is 1, `c2 1a 34` is 2551, `3e 64 66` is −1.
/// A fraction or a value that does not fit returns `None`.
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

fn read_u16(dump: &[u8], pos: usize) -> Result<u16, String> {
    let bytes = dump
        .get(pos..pos + 2)
        .ok_or_else(|| format!("truncated at {pos:#x}"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn find_slice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_number_samples() {
        assert_eq!(decode_i64(&[0x80]), Some(0));
        assert_eq!(decode_i64(&[0xc1, 0x02]), Some(1));
        assert_eq!(decode_i64(&[0xc2, 0x1a, 0x34]), Some(2551));
        assert_eq!(decode_i64(&[0x3e, 0x64, 0x66]), Some(-1));
        assert_eq!(decode_i64(&[0xc1, 0x03]), Some(2));
        assert_eq!(decode_i64(&[0xc1, 0x06]), Some(5));
        assert_eq!(decode_i64(&[0xc2, 0x02]), Some(100));
        assert_eq!(decode_i64(&[0xc2, 0x10, 0x1f]), Some(1530));
        assert_eq!(decode_i64(&[0x3e, 0x62, 0x66]), Some(-3));
    }

    #[test]
    fn fragment_covers_null_utf16_number_and_table_end() {
        let mut dump = table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(vec![0xc1, 0x02]),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        );
        dump.extend(table(
            "T_PIPINGPOINT",
            &[
                ch("SP_PLANTITEMID"),
                num("PIPINGPOINTNUMBER"),
                num("FLOWDIRECTION"),
                num("NOMINALDIAMETER"),
            ],
            &[row(&[
                Some(utf16("AB")),
                Some(vec![0xc2, 0x1a, 0x34]),
                None,
                None,
            ])],
        ));
        dump.extend(b"\nCREATE TABLE ignored");
        dump.extend(table(
            "T_SIGNALPOINT",
            &[ch("SP_PLANTITEMID"), num("SIGNALPOINTNUMBER")],
            &[
                row(&[Some(utf16("AB")), Some(vec![0x80])]),
                row(&[Some(utf16("CD")), Some(vec![0xc1, 0x02])]),
            ],
        ));
        dump.extend(b"\nGRANT ignored");

        let tables = read_port_tables(&dump).unwrap();
        assert_eq!(tables.piping_row_count, 1);
        assert_eq!(tables.signal_row_count, 2);
        assert_eq!(tables.piping_points.len(), 1);
        let piping = &tables.piping_points[0];
        assert_eq!(piping.plant_item_id, "AB");
        assert_eq!(piping.number, 2551);
        assert_eq!(piping.flow_direction, None);
        assert_eq!(piping.nominal_diameter, None);
        assert_eq!(piping.uid(), "AB.2551");
        assert_eq!(tables.signal_points[0].number, 0);
        assert_eq!(tables.signal_points[1].uid(), "CD.1");

        let parents = HashSet::from(["AB".to_string()]);
        let selected = ports_for_parents(&tables, &parents);
        assert_eq!(selected.piping.len(), 1);
        assert_eq!(selected.signal.len(), 1);
        assert_eq!(selected.signal[0].plant_item_id, "AB");
    }

    #[test]
    fn unpinned_applications_does_not_read_port_tables() {
        let dump = table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(vec![0xc1, 0x03]),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        );
        let error = read_port_tables(&dump).unwrap_err();
        assert!(matches!(error, ExpError::FormatUnpinned), "{error}");
        assert!(!error.to_string().contains("T_PIPINGPOINT"));
    }

    #[derive(Clone, Copy)]
    struct Col {
        name: &'static str,
        kind: u16,
    }

    fn num(name: &'static str) -> Col {
        Col { name, kind: 2 }
    }

    fn ch(name: &'static str) -> Col {
        Col { name, kind: 1 }
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    fn row(fields: &[Option<Vec<u8>>]) -> Vec<Option<Vec<u8>>> {
        fields.to_vec()
    }

    fn table(name: &str, columns: &[Col], rows: &[Vec<Option<Vec<u8>>>]) -> Vec<u8> {
        let listed = columns
            .iter()
            .map(|column| format!("\"{}\"", column.name))
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = format!("INSERT INTO \"{name}\" ({listed}) VALUES\n").into_bytes();
        out.extend(u16::try_from(columns.len()).unwrap().to_le_bytes());
        for column in columns {
            out.extend(column.kind.to_le_bytes());
            out.extend(4000u16.to_le_bytes());
            if column.kind == 1 {
                out.extend(CHARSET_AL16UTF16.to_le_bytes());
                out.extend(CHARSET_FORM.to_le_bytes());
            }
        }
        out.extend([0, 0, 0, 0]);
        for fields in rows {
            for field in fields {
                match field {
                    None => out.extend(0xFFFEu16.to_le_bytes()),
                    Some(payload) => {
                        out.extend(u16::try_from(payload.len()).unwrap().to_le_bytes());
                        out.extend(payload);
                    }
                }
            }
            out.extend([0, 0]);
        }
        out.extend(0xFFFFu16.to_le_bytes());
        out
    }

    #[test]
    fn ijingchang_ports_match_the_reference() {
        let dump_path = std::path::PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
        let corpus = std::path::PathBuf::from(r"D:\work\cad\PID自研软件验证");
        let stem = "I井场 注采阀组工艺及自控流程图";
        let pid = corpus.join(format!("{stem}.pid"));
        let reference_path = corpus.join(format!("{stem}_Data.xml"));
        if !dump_path.is_file() || !pid.is_file() || !reference_path.is_file() {
            eprintln!("skip: oracle exp corpus or I井场 .pid is missing");
            return;
        }
        let dump = std::fs::read(&dump_path).unwrap();
        let tables = read_port_tables(&dump).unwrap();
        assert_eq!(tables.piping_row_count, 9_628);
        assert_eq!(tables.signal_row_count, 22_057);
        assert_eq!(tables.piping_points.len(), 9_628);
        assert_eq!(tables.signal_points.len(), 22_057);

        let drawing = super::super::pid_load::load_publish_from_pid(&pid).unwrap();
        let parents = published_parent_ids(&drawing);
        let selected = ports_for_parents(&tables, &parents);
        let xml = std::fs::read_to_string(&reference_path).unwrap();
        let (reference_piping, reference_signal) = reference_port_uids(&xml);

        let piping_extra = extras(&selected.piping, &reference_piping);
        let signal_extra = extras_signal(&selected.signal, &reference_signal);
        assert!(
            piping_extra.is_empty() && signal_extra.is_empty(),
            "extra piping {} signal {}\n{piping_extra:?}\n{signal_extra:?}",
            piping_extra.len(),
            signal_extra.len()
        );
        assert_eq!(selected.piping.len(), 362);
        assert_eq!(selected.signal.len(), 915);
        assert_eq!(reference_piping.len(), 362);
        assert_eq!(reference_signal.len(), 915);
        let piping_uids = selected
            .piping
            .iter()
            .map(PipingPoint::uid)
            .collect::<HashSet<_>>();
        let signal_uids = selected
            .signal
            .iter()
            .map(SignalPoint::uid)
            .collect::<HashSet<_>>();
        assert!(reference_piping.is_subset(&piping_uids));
        assert!(reference_signal.is_subset(&signal_uids));
    }

    fn extras(points: &[PipingPoint], reference: &HashSet<String>) -> Vec<String> {
        points
            .iter()
            .map(PipingPoint::uid)
            .filter(|uid| !reference.contains(uid))
            .collect()
    }

    fn extras_signal(points: &[SignalPoint], reference: &HashSet<String>) -> Vec<String> {
        points
            .iter()
            .map(SignalPoint::uid)
            .filter(|uid| !reference.contains(uid))
            .collect()
    }

    fn reference_port_uids(xml: &str) -> (HashSet<String>, HashSet<String>) {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut depth = 0usize;
        let mut tag = String::new();
        let mut uid = String::new();
        let mut piping = HashSet::new();
        let mut signal = HashSet::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(start)) => {
                    if depth == 1 {
                        tag = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                        uid.clear();
                    } else if depth == 2 {
                        take_uid(&start, &mut uid);
                    }
                    depth += 1;
                }
                Ok(Event::Empty(start)) => {
                    if depth == 2 {
                        take_uid(&start, &mut uid);
                    }
                }
                Ok(Event::End(_)) => {
                    depth = depth.saturating_sub(1);
                    if depth == 1 && !uid.is_empty() {
                        if tag == "PIDPipingPort" {
                            piping.insert(uid.clone());
                        } else if tag == "PIDSignalPort" {
                            signal.insert(uid.clone());
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => panic!("reference xml: {err}"),
                _ => {}
            }
        }
        (piping, signal)
    }

    fn take_uid(start: &quick_xml::events::BytesStart<'_>, uid: &mut String) {
        if start.name().as_ref() != b"IObject" {
            return;
        }
        for attribute in start.attributes().flatten() {
            if attribute.key.as_ref() == b"UID" {
                *uid = attribute
                    .unescape_value()
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default();
            }
        }
    }
}
