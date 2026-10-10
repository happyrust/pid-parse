//! The DDL an Oracle `exp` dump carries as plain text: every
//! `CREATE TABLE` statement sits on a line of its own, and belongs to
//! the schema the nearest `CONNECT <OWNER>` line before it names
//! (P12 of plan `2026-10-09-a-plant-backup-becomes-one-backup-store`).
//! The rows follow in exp's own binary format and are not read here
//! (Q3: the first version of the Backup Store registers an Oracle
//! dump's tables and leaves them empty).
//!
//! Names are kept as the dump spells them -- upper case in every
//! sample (Q20). Two tables of the same name under two owners are two
//! tables; the DWG and `SQPlant` dumps hold 154 each over 126 distinct
//! names (`MAX_ID` under all four owners, `SPIDCACHE` under two).
//!
//! `examples/oracle_exp_schema.rs` prints what [`scan_create_tables`]
//! finds.

use thiserror::Error;

/// The bytes an Oracle `exp` dump opens with: a 3-byte framing and
/// the ASCII `EXPORT:V<major>.<minor>.<patch>` header line.
pub const EXP_MAGIC: &[u8] = b"\x03\x03iEXPORT:V";

/// Whether `data` opens like an Oracle `exp` dump.
pub fn is_exp_dump(data: &[u8]) -> bool {
    data.starts_with(EXP_MAGIC)
}

/// One column of a `CREATE TABLE` statement, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpColumn {
    /// The name between the quotes (`SP_ID`).
    pub name: String,
    /// The type with its arguments: `NUMBER(11, 0)`, `NVARCHAR2(32)`,
    /// `FLOAT(126)`, `DATE`, `BLOB`.
    pub type_spec: String,
    /// What follows the type, trimmed: `NOT NULL ENABLE` or nothing in
    /// the samples.
    pub modifiers: String,
}

impl ExpColumn {
    /// The type without its arguments (`NUMBER` of `NUMBER(11, 0)`).
    pub fn type_name(&self) -> &str {
        self.type_spec
            .split('(')
            .next()
            .unwrap_or(&self.type_spec)
            .trim()
    }

    /// The arguments between the type's parentheses, trimmed, in order
    /// (`["11", "0"]` of `NUMBER(11, 0)`); none without parentheses.
    pub fn type_arguments(&self) -> Vec<&str> {
        let Some(open) = self.type_spec.find('(') else {
            return Vec::new();
        };
        match balanced(&self.type_spec[open..]) {
            Some(inner) => split_top_level(inner),
            None => Vec::new(),
        }
    }

    /// Whether the column is declared `NOT NULL`.
    pub fn not_null(&self) -> bool {
        self.modifiers.contains("NOT NULL")
    }
}

/// One `CREATE TABLE` statement with the owner it was filed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpTable {
    /// The `CONNECT` owner before the statement, as spelled there.
    pub owner: String,
    /// The table's name between the quotes.
    pub name: String,
    /// Its columns, in the statement's order.
    pub columns: Vec<ExpColumn>,
    /// Byte offset of the statement's line in the dump.
    pub offset: usize,
}

/// What stops the DDL scan.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExpDdlError {
    /// The data does not open with [`EXP_MAGIC`].
    #[error("not an Oracle exp dump: the file does not open with \\x03\\x03iEXPORT:V")]
    NotAnExpDump,
    /// A `CREATE TABLE` line came before any `CONNECT` line, so it has
    /// no owner to belong to.
    #[error("offset {offset}: CREATE TABLE before any CONNECT line")]
    TableBeforeConnect {
        /// Byte offset of the line.
        offset: usize,
    },
    /// A `CREATE TABLE` line is not shaped as every one in the samples.
    #[error("offset {offset}: CREATE TABLE statement {what}")]
    Malformed {
        /// Byte offset of the line.
        offset: usize,
        /// What was found wanting.
        what: &'static str,
    },
}

/// Every `CREATE TABLE` statement of `dump`, in file order, each under
/// the `CONNECT` owner before it.
pub fn scan_create_tables(dump: &[u8]) -> Result<Vec<ExpTable>, ExpDdlError> {
    if !is_exp_dump(dump) {
        return Err(ExpDdlError::NotAnExpDump);
    }
    let mut tables = Vec::new();
    let mut owner: Option<String> = None;
    let mut offset = 0usize;
    for line in dump.split(|byte| *byte == b'\n') {
        let line_offset = offset;
        offset += line.len() + 1;
        if let Some(rest) = line.strip_prefix(b"CONNECT ") {
            if let Some(name) = connect_owner(rest) {
                owner = Some(name);
            }
        } else if line.starts_with(b"CREATE TABLE \"") {
            let Some(owner) = owner.clone() else {
                return Err(ExpDdlError::TableBeforeConnect {
                    offset: line_offset,
                });
            };
            let text = std::str::from_utf8(line).map_err(|_| ExpDdlError::Malformed {
                offset: line_offset,
                what: "is not ASCII / UTF-8",
            })?;
            let (name, columns) =
                parse_create_table(text).map_err(|what| ExpDdlError::Malformed {
                    offset: line_offset,
                    what,
                })?;
            tables.push(ExpTable {
                owner,
                name,
                columns,
                offset: line_offset,
            });
        }
    }
    Ok(tables)
}

/// The owner a `CONNECT ` line names, when the rest of the line is one
/// identifier; binary data that happens to start a line with those
/// bytes is left alone.
fn connect_owner(rest: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(rest).ok()?.trim();
    if text.is_empty()
        || !text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'#'))
    {
        return None;
    }
    Some(text.to_string())
}

/// `CREATE TABLE "NAME" ("COL" TYPE [modifiers], ...)  PCTFREE ...`
/// to its name and columns.
fn parse_create_table(line: &str) -> Result<(String, Vec<ExpColumn>), &'static str> {
    let rest = line
        .strip_prefix("CREATE TABLE \"")
        .ok_or("does not open with CREATE TABLE \"")?;
    let close_name = rest
        .find('"')
        .ok_or("has no closing quote on the table name")?;
    let name = rest[..close_name].to_string();
    let after_name = &rest[close_name + 1..];
    let open = after_name.find('(').ok_or("has no column list")?;
    let column_list = balanced(&after_name[open..]).ok_or("has an unbalanced column list")?;
    let mut columns = Vec::new();
    for piece in split_top_level(column_list) {
        columns.push(parse_column(piece)?);
    }
    if columns.is_empty() {
        return Err("has no columns");
    }
    Ok((name, columns))
}

/// The text between the parenthesis `text` opens with and its match.
fn balanced(text: &str) -> Option<&str> {
    let mut depth = 0i32;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[1..index]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `text` split on the commas outside parentheses, pieces trimmed and
/// empty ones dropped.
fn split_top_level(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                pieces.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    pieces.push(text[start..].trim());
    pieces.retain(|piece| !piece.is_empty());
    pieces
}

/// `"COL" TYPE[(args)] [modifiers]` to a column.
fn parse_column(piece: &str) -> Result<ExpColumn, &'static str> {
    let after_quote = piece
        .strip_prefix('"')
        .ok_or("has a column entry that does not open with a quoted name")?;
    let close = after_quote
        .find('"')
        .ok_or("has a column name without its closing quote")?;
    let name = after_quote[..close].to_string();
    let rest = after_quote[close + 1..].trim_start();
    if rest.is_empty() {
        return Err("has a column without a type");
    }
    // The type name runs to the first space or parenthesis; a
    // parenthesis right after it opens the type's arguments; the few
    // multi-word types take their second word(s) too.
    let name_end = rest.find([' ', '(']).unwrap_or(rest.len());
    let mut type_end = if rest[name_end..].starts_with('(') {
        let args = balanced(&rest[name_end..]).ok_or("has a type with unbalanced parentheses")?;
        name_end + args.len() + 2
    } else {
        name_end
    };
    for (first_word, continuation) in MULTI_WORD_TYPES {
        if rest[..name_end].eq_ignore_ascii_case(first_word) {
            let after = &rest[type_end..];
            if let Some(stripped) = after.strip_prefix(' ') {
                if stripped.len() >= continuation.len()
                    && stripped[..continuation.len()].eq_ignore_ascii_case(continuation)
                    && stripped[continuation.len()..]
                        .chars()
                        .next()
                        .is_none_or(|c| c == ' ')
                {
                    type_end += 1 + continuation.len();
                }
            }
        }
    }
    Ok(ExpColumn {
        name,
        type_spec: rest[..type_end].to_string(),
        modifiers: rest[type_end..].trim().to_string(),
    })
}

/// Oracle types written as more than one word: the first word and what
/// follows it (after the arguments, if any).
const MULTI_WORD_TYPES: [(&str, &str); 4] = [
    ("LONG", "RAW"),
    ("DOUBLE", "PRECISION"),
    ("TIMESTAMP", "WITH TIME ZONE"),
    ("TIMESTAMP", "WITH LOCAL TIME ZONE"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn dump(body: &[u8]) -> Vec<u8> {
        let mut bytes = b"\x03\x03iEXPORT:V12.01.00\nDSYSTEM\nRUSERS\n".to_vec();
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn tables_are_filed_under_the_connect_line_before_them() {
        let dump = dump(
            b"CONNECT PLANTPIDD\n\
              CREATE TABLE \"MAX_ID\" (\"ID\" NUMBER(11, 0), \"NAME\" NVARCHAR2(50) NOT NULL ENABLE)  PCTFREE 10 TABLESPACE \"X\" LOGGING NOCOMPRESS\n\
              \x00\x01binary\xff\n\
              CONNECT PLANTPID\n\
              CREATE TABLE \"MAX_ID\" (\"ID\" NUMBER(11, 0))  PCTFREE 10\n\
              CREATE TABLE \"T_DRAWINGVERSION\" (\"SP_ID\" NVARCHAR2(32) NOT NULL ENABLE, \"SP_STORAGE\" BLOB, \"X\" FLOAT(126), \"D\" DATE, \"V\" VARCHAR2(4000), \"R\" LONG RAW, \"T\" TIMESTAMP(6) WITH TIME ZONE NOT NULL ENABLE)  PCTFREE 10 LOB (\"SP_STORAGE\") STORE AS SECUREFILE  (TABLESPACE \"X\" ENABLE STORAGE IN ROW CHUNK 8192 STORAGE(INITIAL 106496 NEXT 1048576))\n",
        );
        let tables = scan_create_tables(&dump).unwrap();
        assert_eq!(
            vec![
                ("PLANTPIDD", "MAX_ID", 2),
                ("PLANTPID", "MAX_ID", 1),
                ("PLANTPID", "T_DRAWINGVERSION", 7),
            ],
            tables
                .iter()
                .map(|t| (t.owner.as_str(), t.name.as_str(), t.columns.len()))
                .collect::<Vec<_>>()
        );
        let first = &tables[0];
        assert_eq!(
            vec![
                ("ID", "NUMBER(11, 0)", "", false),
                ("NAME", "NVARCHAR2(50)", "NOT NULL ENABLE", true),
            ],
            first
                .columns
                .iter()
                .map(|c| (
                    c.name.as_str(),
                    c.type_spec.as_str(),
                    c.modifiers.as_str(),
                    c.not_null()
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!("NUMBER", first.columns[0].type_name());
        assert_eq!(vec!["11", "0"], first.columns[0].type_arguments());
        let version = &tables[2];
        assert_eq!(
            vec![
                "NVARCHAR2(32)",
                "BLOB",
                "FLOAT(126)",
                "DATE",
                "VARCHAR2(4000)",
                "LONG RAW",
                "TIMESTAMP(6) WITH TIME ZONE"
            ],
            version
                .columns
                .iter()
                .map(|c| c.type_spec.as_str())
                .collect::<Vec<_>>()
        );
        assert!(version.columns[1].type_arguments().is_empty());
        assert_eq!("BLOB", version.columns[1].type_name());
        assert_eq!("LONG RAW", version.columns[5].type_name());
        assert_eq!("", version.columns[5].modifiers);
        assert_eq!("TIMESTAMP", version.columns[6].type_name());
        assert_eq!(vec!["6"], version.columns[6].type_arguments());
        assert!(version.columns[6].not_null());
        assert!(tables[1].offset > tables[0].offset);
        assert!(dump[tables[2].offset..].starts_with(b"CREATE TABLE \"T_DRAWINGVERSION\""));
    }

    #[test]
    fn a_table_before_any_connect_line_is_an_error() {
        let dump = dump(b"CREATE TABLE \"T\" (\"A\" DATE)  PCTFREE 10\n");
        assert_eq!(
            Err(ExpDdlError::TableBeforeConnect { offset: 35 }),
            scan_create_tables(&dump)
        );
    }

    #[test]
    fn a_connect_line_that_is_not_one_identifier_is_not_an_owner() {
        let dump = dump(
            b"CONNECT \x80\x81 noise\n\
              CONNECT OWNER_1\n\
              CONNECT two words\n\
              CREATE TABLE \"T\" (\"A\" DATE)  PCTFREE 10\n",
        );
        let tables = scan_create_tables(&dump).unwrap();
        assert_eq!("OWNER_1", tables[0].owner);
    }

    #[test]
    fn a_malformed_statement_and_a_file_that_is_not_a_dump_are_refused() {
        let dump = dump(b"CONNECT O\nCREATE TABLE \"T\" (\"A\" DATE, B NUMBER)  PCTFREE 10\n");
        assert_eq!(
            Err(ExpDdlError::Malformed {
                offset: 45,
                what: "has a column entry that does not open with a quoted name",
            }),
            scan_create_tables(&dump)
        );
        assert_eq!(
            Err(ExpDdlError::NotAnExpDump),
            scan_create_tables(b"TAPE\x00\x00")
        );
        assert!(!is_exp_dump(b"TAPE"));
        assert!(is_exp_dump(&dump));
    }
}
