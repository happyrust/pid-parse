//! Which representations the plant database draws on a drawing a publish
//! leaves out.
//!
//! A `SmartPlant` plant database keeps one `T_REPRESENTATION` row for every
//! graphic an item has on a drawing, and a publish states each as a
//! `PIDRepresentation` whose UID is the row's `SP_ID`. This probe holds the
//! `_Data.xml` files of a publish directory against an Oracle `exp` dump of
//! the plant. It needs no vendor reference, so it reads every drawing of a
//! package, the ones nobody published a reference for included.
//!
//! ```text
//! probe_which_database_representations_a_publish_leaves_out
//!     <Export.dmp> <directory of _Data.xml> [--reference DIR]
//! ```
//!
//! For each `<name>_Data.xml` it takes the rows the database draws on the
//! drawing of that `DocUID` (`INSTOCKPILE` 1) and asks which of them the
//! document states. The rows are counted by class: the representation type,
//! named by the dump's own codelist 39, and the item type of the model item
//! behind the row (`T_MODELITEM.ITEMTYPENAME`). A class is one of three
//! things — stated on every drawing, stated on none, or stated on some rows
//! and left out on others — and every row of the third kind is listed with
//! its drawing, model item, `SP_ID` and `GraphicOID`.
//!
//! `--reference DIR` is a directory of vendor `_Data.xml` files. A drawing
//! whose `<name>_Data.xml` there carries the same `DocUID` is read the same
//! way, so the table shows beside each class what `SmartPlant` itself stated
//! and left out of the same rows.
//!
//! Only representations are compared, by UID: no attribute value, no
//! relation, and no item apart from its representation. The dump is read by
//! this file's own reader, not by `publish::oracle_exp`.
//!
//! What this reads on 2026-11-12, the 53-sheet plant backup against the
//! package publish of commit `7de1ef1` with the eleven acceptance references:
//! 42 048 rows drawn on 51 drawings (`D03` and `D04` have none), 23 762
//! stated and none stated that the database lacks. Three classes are stated
//! on no drawing and by no reference: the 12 584 labels, the 4 678 symbols
//! of pipe runs and the 1 001 symbols of signal runs. Twenty-three rows are
//! left out of classes stated everywhere else, 21 on `C井场` and 2 on
//! `Q井场`: instruments, notes, nozzles, mechanical equipment and a vessel
//! the database draws and the `.pid` of the package does not. On the seven
//! drawings with a reference of the dump's revision there is no such row.
//!
//! Exit code 2 on a usage error, 1 when a file cannot be read, 0 otherwise.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The first `INSERT INTO "name"` of an Oracle `exp` dump with each cell as
/// text: national text as written, an integer `NUMBER` in decimal, and
/// `None` for NULL, a date, a fraction or database-charset text.
struct ExpTable {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
}

impl ExpTable {
    fn read(dump: &[u8], name: &str) -> Result<Self, String> {
        let fail = |detail: &str| format!("{name}: {detail}");
        let marker = format!("INSERT INTO \"{name}\"");
        let at = dump
            .windows(marker.len())
            .position(|window| window == marker.as_bytes())
            .ok_or_else(|| fail("no INSERT in the dump"))?;
        let line_end = dump[at..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| at + offset)
            .ok_or_else(|| fail("the INSERT line does not end"))?;
        let line = std::str::from_utf8(&dump[at..line_end])
            .map_err(|_| fail("the INSERT line is not ASCII"))?;
        let list = line
            .find('(')
            .zip(line.find(") VALUES"))
            .and_then(|(open, close)| line.get(open + 1..close))
            .ok_or_else(|| fail("the INSERT line has no column list"))?;
        let columns: Vec<String> = list
            .split(',')
            .map(|piece| piece.trim().trim_matches('"').to_string())
            .collect();
        let word = |pos: usize| {
            dump.get(pos..pos + 2)
                .map(|bytes| usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
                .ok_or_else(|| fail("the table is cut short"))
        };
        let mut pos = line_end + 1;
        if word(pos)? != columns.len() {
            return Err(fail(
                "the header and the INSERT line disagree on the columns",
            ));
        }
        pos += 2;
        // Per column: is it national text (charset 2000, form 2), is it a number.
        let mut kinds = Vec::with_capacity(columns.len());
        for _ in &columns {
            let kind = word(pos)?;
            pos += 2;
            match kind {
                1 => {
                    let (charset, form) = (word(pos + 2)?, word(pos + 4)?);
                    pos += 6;
                    kinds.push((charset == 2000 && form == 2, false));
                }
                2 | 12 => {
                    pos += 2;
                    kinds.push((false, kind == 2));
                }
                _ => return Err(fail("a column type this probe does not read")),
            }
        }
        if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
            return Err(fail("the column header does not end"));
        }
        pos += 4;
        let mut rows = Vec::new();
        while word(pos)? != 0xFFFF {
            let mut row = Vec::with_capacity(columns.len());
            for (text, number) in &kinds {
                let len = word(pos)?;
                pos += 2;
                if len == 0xFFFE {
                    row.push(None);
                    continue;
                }
                let raw = dump
                    .get(pos..pos + len)
                    .ok_or_else(|| fail("a cell runs past the dump"))?;
                pos += len;
                row.push(if *text {
                    let (pairs, _) = raw.as_chunks::<2>();
                    let units: Vec<u16> =
                        pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
                    Some(String::from_utf16_lossy(&units))
                } else if *number {
                    oracle_integer(raw).map(|value| value.to_string())
                } else {
                    None
                });
            }
            if dump.get(pos..pos + 2) != Some(&[0, 0]) {
                return Err(fail("a row does not end"));
            }
            pos += 2;
            rows.push(row);
        }
        Ok(Self {
            name: name.to_string(),
            columns,
            rows,
        })
    }

    fn column(&self, name: &str) -> Result<usize, String> {
        self.columns
            .iter()
            .position(|column| column == name)
            .ok_or_else(|| format!("{}: no column {name}", self.name))
    }
}

/// An Oracle `NUMBER` that is a non-negative integer: `80` is 0, `c1 02` is
/// 1, `c2 1a 34` is 2551. Anything else is `None`.
fn oracle_integer(raw: &[u8]) -> Option<u64> {
    let (&first, digits) = raw.split_first()?;
    if first == 0x80 && digits.is_empty() {
        return Some(0);
    }
    let exponent = usize::from(first.checked_sub(193)?);
    if digits.len() > exponent + 1 {
        return None;
    }
    let mut total: u64 = 0;
    for index in 0..=exponent {
        let digit = match digits.get(index) {
            Some(byte) => u64::from(byte.checked_sub(1).filter(|digit| *digit < 100)?),
            None => 0,
        };
        total = total.checked_mul(100)?.checked_add(digit)?;
    }
    Some(total)
}

/// A UID as the two sides are compared: its hex digits, upper case.
fn uid_key(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_hexdigit)
        .map(|digit| digit.to_ascii_uppercase())
        .collect()
}

/// One row the database draws on a drawing.
struct Drawn {
    id: String,
    /// The representation type's name and the model item's type.
    class: (String, String),
    model_item: String,
    graphic_oid: String,
}

/// What the dump says of the drawings and of what is drawn on each.
struct Database {
    /// `T_DRAWING.SP_ID` → `NAME`.
    drawings: BTreeMap<String, String>,
    /// Drawing → the rows of `T_REPRESENTATION` with `INSTOCKPILE` 1.
    drawn: HashMap<String, Vec<Drawn>>,
    /// Rows on some drawing with another `INSTOCKPILE`.
    in_a_stockpile: usize,
}

impl Database {
    fn read(dump: &[u8]) -> Result<Self, String> {
        let text = |row: &[Option<String>], column: usize| row[column].clone().unwrap_or_default();

        let table = ExpTable::read(dump, "T_DRAWING")?;
        let (id, name) = (table.column("SP_ID")?, table.column("NAME")?);
        let drawings: BTreeMap<String, String> = table
            .rows
            .iter()
            .map(|row| (uid_key(&text(row, id)), text(row, name)))
            .collect();

        let table = ExpTable::read(dump, "T_MODELITEM")?;
        let (id, kind) = (table.column("SP_ID")?, table.column("ITEMTYPENAME")?);
        let item_types: HashMap<String, String> = table
            .rows
            .iter()
            .map(|row| (uid_key(&text(row, id)), text(row, kind)))
            .collect();

        // Codelist 39 is the representation type: the list whose entries
        // name every value `REPRESENTATIONTYPE` takes.
        let table = ExpTable::read(dump, "CODELISTS")?;
        let (list, index, label) = (
            table.column("CODELIST_NUMBER")?,
            table.column("CODELIST_INDEX")?,
            table.column("CODELIST_TEXT")?,
        );
        let type_names: HashMap<String, String> = table
            .rows
            .iter()
            .filter(|row| row[list].as_deref() == Some("39"))
            .map(|row| (text(row, index), text(row, label)))
            .collect();

        let table = ExpTable::read(dump, "T_REPRESENTATION")?;
        let (id, kind, oid, stockpile, drawing, item) = (
            table.column("SP_ID")?,
            table.column("REPRESENTATIONTYPE")?,
            table.column("GRAPHICOID")?,
            table.column("INSTOCKPILE")?,
            table.column("SP_DRAWINGID")?,
            table.column("SP_MODELITEMID")?,
        );
        let mut drawn: HashMap<String, Vec<Drawn>> = HashMap::new();
        let mut in_a_stockpile = 0;
        for row in &table.rows {
            let on = uid_key(&text(row, drawing));
            if !drawings.contains_key(&on) {
                continue;
            }
            if row[stockpile].as_deref() != Some("1") {
                in_a_stockpile += 1;
                continue;
            }
            let code = text(row, kind);
            let model_item = uid_key(&text(row, item));
            drawn.entry(on).or_default().push(Drawn {
                id: uid_key(&text(row, id)),
                class: (
                    type_names
                        .get(&code)
                        .cloned()
                        .unwrap_or_else(|| format!("type {code}")),
                    item_types
                        .get(&model_item)
                        .filter(|name| !name.is_empty())
                        .cloned()
                        .unwrap_or_else(|| "(no model item)".to_string()),
                ),
                model_item,
                graphic_oid: row[oid].clone().unwrap_or_else(|| "none".to_string()),
            });
        }
        Ok(Self {
            drawings,
            drawn,
            in_a_stockpile,
        })
    }
}

/// The drawing a `_Data.xml` is of and the representations it states.
struct Document {
    doc_uid: String,
    representations: HashSet<String>,
}

/// The value of the first `name="…"` at or after `from`.
fn attribute<'a>(xml: &'a str, from: usize, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = from + xml.get(from..)?.find(&needle)? + needle.len();
    let end = start + xml.get(start..)?.find('"')?;
    xml.get(start..end)
}

fn read_document(path: &Path) -> Result<Document, String> {
    let xml =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let doc_uid = attribute(&xml, 0, "DocUID")
        .map(uid_key)
        .ok_or_else(|| format!("{}: no DocUID", path.display()))?;
    let mut representations = HashSet::new();
    let open = "<PIDRepresentation>";
    let mut at = 0;
    while let Some(found) = xml.get(at..).and_then(|rest| rest.find(open)) {
        let block = at + found + open.len();
        let uid = attribute(&xml, block, "UID")
            .ok_or_else(|| format!("{}: a PIDRepresentation with no UID", path.display()))?;
        representations.insert(uid_key(uid));
        at = block;
    }
    Ok(Document {
        doc_uid,
        representations,
    })
}

/// How many rows of a class were stated and how many left out.
#[derive(Default, Clone, Copy)]
struct Share {
    stated: usize,
    left_out: usize,
}

impl Share {
    fn add(&mut self, stated: bool) {
        if stated {
            self.stated += 1;
        } else {
            self.left_out += 1;
        }
    }

    fn show(self) -> String {
        format!("{} / {}", self.stated, self.left_out)
    }
}

struct Arguments {
    dump: PathBuf,
    publish: PathBuf,
    reference: Option<PathBuf>,
}

fn arguments() -> Result<Arguments, String> {
    let mut positional = Vec::new();
    let mut reference = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg.to_str() == Some("--reference") {
            reference = Some(PathBuf::from(
                args.next().ok_or("--reference takes a directory")?,
            ));
        } else {
            positional.push(PathBuf::from(arg));
        }
    }
    let [dump, publish] = <[PathBuf; 2]>::try_from(positional).map_err(|_| {
        "usage: probe_which_database_representations_a_publish_leaves_out <Export.dmp> \
         <directory of _Data.xml> [--reference DIR]"
            .to_string()
    })?;
    Ok(Arguments {
        dump,
        publish,
        reference,
    })
}

/// The `<name>_Data.xml` files of `directory`, by name.
fn data_files(directory: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut files = BTreeMap::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        let name = path.file_name().and_then(|name| name.to_str());
        if let Some(stem) = name.and_then(|name| name.strip_suffix("_Data.xml")) {
            if path.is_file() {
                files.insert(stem.to_string(), path);
            }
        }
    }
    Ok(files)
}

fn run(arguments: &Arguments) -> Result<(), String> {
    let dump = std::fs::read(&arguments.dump)
        .map_err(|error| format!("{}: {error}", arguments.dump.display()))?;
    let database = Database::read(&dump)?;
    let published = data_files(&arguments.publish)?;

    // Per class: what the publish made of the rows on every drawing, and
    // what the vendor made of the rows on the drawings it is a reference for.
    let mut classes: BTreeMap<(String, String), (Share, Share)> = BTreeMap::new();
    let mut left_out: Vec<(String, &Drawn)> = Vec::new();
    let mut totals = (0usize, 0usize, 0usize);
    let mut without_a_row = Vec::new();
    let mut not_in_the_dump = Vec::new();
    let mut with_a_reference = Vec::new();
    for (name, path) in &published {
        let document = read_document(path)?;
        if !database.drawings.contains_key(&document.doc_uid) {
            not_in_the_dump.push(name.clone());
            continue;
        }
        let rows = database
            .drawn
            .get(&document.doc_uid)
            .map_or(&[][..], Vec::as_slice);
        if rows.is_empty() {
            without_a_row.push(name.clone());
        }
        let reference = match &arguments.reference {
            Some(directory) => {
                let path = directory.join(format!("{name}_Data.xml"));
                if path.is_file() {
                    Some(read_document(&path)?).filter(|other| other.doc_uid == document.doc_uid)
                } else {
                    None
                }
            }
            None => None,
        };
        if reference.is_some() {
            with_a_reference.push(name.clone());
        }
        let ids: HashSet<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        totals.0 += rows.len();
        totals.2 += document
            .representations
            .iter()
            .filter(|uid| !ids.contains(uid.as_str()))
            .count();
        for row in rows {
            let stated = document.representations.contains(&row.id);
            let shares = classes.entry(row.class.clone()).or_default();
            shares.0.add(stated);
            totals.1 += usize::from(stated);
            if !stated {
                left_out.push((name.clone(), row));
            }
            if let Some(reference) = &reference {
                shares.1.add(reference.representations.contains(&row.id));
            }
        }
    }

    println!("dump       {}", arguments.dump.display());
    println!("publish    {}", arguments.publish.display());
    if let Some(reference) = &arguments.reference {
        println!("reference  {}", reference.display());
    }
    println!();
    println!(
        "{} drawings in the dump, {} documents in the publish directory",
        database.drawings.len(),
        published.len()
    );
    if !not_in_the_dump.is_empty() {
        println!(
            "documents whose DocUID the dump has no drawing for (not read): {}",
            not_in_the_dump.join(", ")
        );
    }
    println!(
        "drawings the database draws nothing on: {}{}",
        without_a_row.len(),
        if without_a_row.is_empty() {
            String::new()
        } else {
            format!(" ({})", without_a_row.join(", "))
        }
    );
    println!(
        "representations the database draws on the published drawings: {}; in a drawing's \
         stockpile, not counted: {}",
        totals.0, database.in_a_stockpile
    );
    println!(
        "the publish states {} of them, leaves out {}, and states {} the database does not draw",
        totals.1,
        totals.0 - totals.1,
        totals.2
    );
    println!();

    let heading = if arguments.reference.is_some() {
        format!("reference, {} drawing(s): same", with_a_reference.len())
    } else {
        String::new()
    };
    println!(
        "{:<12} {:<18} {:>27}   {:<34} the class is",
        "type", "item type", "publish: stated / left out", heading
    );
    let mut partly = HashSet::new();
    for (class, (ours, theirs)) in &classes {
        let verdict = if ours.left_out == 0 {
            "stated everywhere"
        } else if ours.stated == 0 {
            "stated nowhere"
        } else {
            partly.insert(class.clone());
            "LEFT OUT ON SOME ROWS"
        };
        let reference = if arguments.reference.is_some() {
            theirs.show()
        } else {
            String::new()
        };
        println!(
            "{:<12} {:<18} {:>27}   {:<34} {verdict}",
            class.0,
            class.1,
            ours.show(),
            reference
        );
    }
    println!();

    let listed: Vec<&(String, &Drawn)> = left_out
        .iter()
        .filter(|(_, row)| partly.contains(&row.class))
        .collect();
    println!(
        "rows left out of a class the publish states elsewhere: {}",
        listed.len()
    );
    for (drawing, row) in listed {
        println!(
            "  {drawing}: {} of {} {}, representation {}, GraphicOID {}",
            row.class.0, row.class.1, row.model_item, row.id, row.graphic_oid
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let arguments = match arguments() {
        Ok(arguments) => arguments,
        Err(usage) => {
            eprintln!("{usage}");
            return ExitCode::from(2);
        }
    };
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}
