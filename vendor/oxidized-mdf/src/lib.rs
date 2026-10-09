// SPDX-License-Identifier: GPL-3.0-or-later
// Original: oxidized-mdf by schrieveslaach (https://gitlab.com/schrieveslaach/oxidized-mdf)
// Modified: 2026-04-24 by happyrust
//   - Removed async runtime (async-std, futures-lite, async-log)
//   - Converted public API to sync (MdfDatabase::open, db.rows)
//   - Eliminated panic edges: unwrap/todo replaced with Result propagation
//   - Row iterator: record.take().unwrap() → let-else (zero unwrap in production)
//   - Column parse failure: error! → warn! (compact format NULL is expected)
//   - Changed read() to read_exact() for page integrity
//   - Upgraded to edition 2021, uuid 1.x
//   - Added #![warn(...)] lint set to mirror parent crate's quality gate
// Modified: 2026-10-09 by happyrust
//   - rows / try_rows read records in slot order and leave ghost records out
//   - Added GhostRow and MdfDatabase::ghost_rows: a table's ghost records, raw
//   - Added MdfDatabase::from_bytes, user_tables (TableInfo / ColumnInfo: schema, columns,
//     rcrows) and scan_table (every live or ghost record of a table with its page and slot)
//   - The whole file is held in memory and pages are read by id in any order; open and
//     from_read load the file first. rows / try_rows / ghost_rows / scan_table take &self
//   - text / ntext / image columns follow their 16-byte text pointer to the LOB's root and
//     data fragments (PageReader::read_lob); scan_table names each LOB's root (LobRef).
//     ntext is decoded as UTF-16LE; image and text are handed out as bytes (the code page
//     of a text column is not known to the reader)
//   - datetime is built from its stored days and ticks (pages::datetime_from_parts)
//   - An nvarchar / varchar column zero bytes long is "" and no longer NULL

#![allow(dead_code)]
// Mirror the pedantic lint subset baked into the parent `pid-parse`
// crate (see `../../../../src/lib.rs`) so vendored code maintains the
// same quality bar as our own modules. Combined with the CI
// `-D warnings` gate this hard-fails regressions across the workspace.
#![warn(
    clippy::uninlined_format_args,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls,
    clippy::manual_let_else,
    clippy::map_unwrap_or,
    clippy::unreadable_literal,
    clippy::bool_to_int_with_if,
    clippy::implicit_clone,
    clippy::explicit_iter_loop,
    clippy::unnecessary_map_or
)]

//! # A Crate for Parsing MDF files
//!
//! `oxidized-mdf` provides utilities to parse MDF files of the [Microsoft SQL Server](https://en.wikipedia.org/wiki/Microsoft_SQL_Server).
//!
//! ```rust
//! use oxidized_mdf::MdfDatabase;
//!
//! # fn main() {
//! let mut db = MdfDatabase::open("data/AWLT2005.mdf").unwrap();
//! let mut rows = db.rows("Address").unwrap();
//!
//! for row in rows {
//!    println!("{:?}", row.value("City"));
//! }
//! # }
//! ```

#![warn(rust_2018_idioms)]

pub mod error;
mod pages;
mod sys;

use crate::error::Error;
use crate::pages::{
    datetime_from_parts, BootPage, LobLink, Page, PagePointer, Record, SlottedRecord, TextPointer,
    TextRecord,
};
use crate::sys::{BaseTableData, Column};
use chrono::{DateTime, Utc};
use core::fmt::{Display, Formatter};
use log::{error, warn};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::rc::Rc;
use uuid::Uuid;

/// Bytes of one page; an MDF is a sequence of them.
const PAGE_SIZE: usize = 8192;

/// The page the boot page sits at.
const BOOT_PAGE_ID: u32 = 9;

pub struct MdfDatabase {
    page_reader: PageReader,
    boot_page: BootPage,
    pub(crate) base_table_data: BaseTableData,
}

impl MdfDatabase {
    /// Reads the whole file into memory and opens it; see
    /// [`MdfDatabase::from_bytes`].
    pub fn open<P>(p: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        Self::from_bytes(std::fs::read(p)?)
    }

    /// Opens an MDF held in memory. Every page can be read in any order,
    /// which following a LOB's text pointer back to its pages needs.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let page_reader = PageReader::new(bytes);

        let boot_page = page_reader.read_boot_page()?;
        let base_table_data = BaseTableData::parse(&page_reader, &boot_page)?;

        Ok(Self {
            page_reader,
            boot_page,
            base_table_data,
        })
    }

    /// Reads `read` to its end and opens what it held; see
    /// [`MdfDatabase::from_bytes`].
    pub fn from_read(mut read: Box<dyn Read>) -> Result<Self, Error> {
        let mut bytes = Vec::new();
        read.read_to_end(&mut bytes)?;
        Self::from_bytes(bytes)
    }

    pub fn database_name(&self) -> &str {
        &self.boot_page.database_name
    }

    /// Returns the table names of this database file.
    ///
    /// ```rust
    /// # use oxidized_mdf::MdfDatabase;
    /// # fn main() {
    /// let db = MdfDatabase::open("data/AWLT2005.mdf").unwrap();
    /// let table_names = db.table_names();
    /// assert!(table_names.contains(&String::from("Customer")));
    /// # }
    /// ```
    pub fn table_names(&self) -> Vec<String> {
        self.base_table_data.tables()
    }

    /// Returns every user table with its schema, its columns and the row
    /// count the database keeps for it, in object-id order. Unlike
    /// [`MdfDatabase::table_names`], tables of the same name in different
    /// schemas stay apart.
    pub fn user_tables(&self) -> Result<Vec<TableInfo>, Error> {
        self.base_table_data.user_tables()
    }

    /// Returns every record of the table's heap or clustered index, live and
    /// ghost, in page-chain and slot order, each with the page and slot it
    /// was read from. A live row's `text` / `ntext` / `image` values are
    /// read from their LOB pages and each names its root in `lobs`. A page
    /// that cannot be read, or a live record that does not parse, yields an
    /// `Err` in its place.
    pub fn scan_table<'a>(
        &'a self,
        table: &'a TableInfo,
    ) -> Result<impl Iterator<Item = Result<ScannedRecord, Error>> + 'a, Error> {
        let (page_pointers, allocation_units): (Vec<_>, Vec<_>) = self
            .base_table_data
            .base_page_pointers(table.object_id)?
            .into_iter()
            .unzip();
        let columns = table
            .columns
            .iter()
            .map(|column| Column {
                name: &column.name,
                r#type: &column.type_name,
                max_length: column.max_length,
                precision: column.precision,
                scale: column.scale,
            })
            .collect::<Vec<_>>();
        let page_reader = &self.page_reader;

        Ok(page_reader
            .read_pages_of_pointers(page_pointers)
            .flat_map(move |page| {
                let page = match page {
                    Ok(page) => page,
                    Err(err) => return vec![Err(err)],
                };
                if !allocation_units.contains(&page.header().allocation_unit_id) {
                    return vec![Err(Error::ParseError(
                        "page chain left the allocation units of the table",
                    ))];
                }
                let page_id = page.header().page_id;
                page.slotted_records()
                    .into_iter()
                    .map(|slotted| {
                        if slotted.is_ghost() {
                            return Ok(ScannedRecord::Ghost(GhostRow {
                                page_id,
                                slot: slotted.slot,
                                record_type: slotted.record_type,
                                bytes: slotted.bytes.to_vec(),
                            }));
                        }
                        let record = Record::try_from(slotted.bytes).map_err(Error::from)?;
                        let (row, lobs) =
                            parse_record_columns(&table.name, record, &columns, page_reader)?;
                        Ok(ScannedRecord::Live {
                            page_id,
                            slot: slotted.slot,
                            row,
                            lobs,
                        })
                    })
                    .collect()
            }))
    }

    /// Returns the column names of the given table name.
    ///
    /// ```rust
    /// # use oxidized_mdf::MdfDatabase;
    /// # fn main() {
    /// let db = MdfDatabase::open("data/AWLT2005.mdf").unwrap();
    ///
    /// let column_names = db.column_names("Address").unwrap();
    /// assert!(column_names.contains(&String::from("City")));
    /// # }
    /// ```
    pub fn column_names(&self, table_name: &str) -> Option<Vec<String>> {
        Some(
            self.base_table_data
                .table(table_name)?
                .columns
                .into_iter()
                .map(|c| c.name.to_string())
                .collect(),
        )
    }

    /// Returns an iterator over the rows in the given table.
    ///
    /// ```rust
    /// use oxidized_mdf::{MdfDatabase, Value};
    ///
    /// # fn main() {
    /// let mut db = MdfDatabase::open("data/AWLT2005.mdf").unwrap();
    /// let mut rows = db.rows("Address").unwrap();
    /// let first_row = rows.next().unwrap();
    ///
    /// assert_eq!(
    ///     first_row.value("AddressLine1").cloned(),
    ///     Some(Value::String(String::from("8713 Yosemite Ct.")))
    /// );
    /// # }
    /// ```
    pub fn rows<'a, 'b: 'a>(&'b self, table_name: &str) -> Option<impl Iterator<Item = Row> + 'a> {
        let table = self.base_table_data.table(table_name)?;
        let page_pointers = table.page_pointers();
        let columns = table.columns;
        let page_reader = &self.page_reader;

        log::debug!("reading pages of {table_name}");
        Some(
            page_reader
                .read_pages_of_pointers(page_pointers)
                .flat_map(move |page| {
                    let mut rows = Vec::new();

                    let page = match page {
                        Ok(page) => page,
                        Err(err) => {
                            error!("Cannot read page: {err}");
                            return rows;
                        }
                    };

                    for record in page.records().into_iter() {
                        rows.push(parse_record_columns_lenient(record, &columns, page_reader));
                    }
                    rows
                }),
        )
    }

    /// Returns an iterator over parse results for rows in the given table.
    ///
    /// Unlike [`MdfDatabase::rows`], this method preserves page and column
    /// parse failures so callers that stage authoritative data can fail fast
    /// instead of silently producing partial output.
    pub fn try_rows<'a, 'b: 'a>(
        &'b self,
        table_name: &str,
    ) -> Option<impl Iterator<Item = Result<Row, Error>> + 'a> {
        let table = self.base_table_data.table(table_name)?;
        let page_pointers = table.page_pointers();
        let columns = table.columns;
        let table_name = table_name.to_string();
        let page_reader = &self.page_reader;

        log::debug!("reading pages of {table_name}");
        Some(
            page_reader
                .read_pages_of_pointers(page_pointers)
                .flat_map(move |page| {
                    let mut rows = Vec::new();

                    let page = match page {
                        Ok(page) => page,
                        Err(err) => {
                            error!("Cannot read page: {err}");
                            rows.push(Err(err));
                            return rows;
                        }
                    };

                    for record in page.records().into_iter() {
                        rows.push(
                            parse_record_columns(&table_name, record, &columns, page_reader)
                                .map(|(row, _lobs)| row),
                        );
                    }
                    rows
                }),
        )
    }

    /// Returns the ghost rows of the given table, in page-chain and slot
    /// order. A page that cannot be read yields an `Err` in its place.
    pub fn ghost_rows<'a, 'b: 'a>(
        &'b self,
        table_name: &str,
    ) -> Option<impl Iterator<Item = Result<GhostRow, Error>> + 'a> {
        let table = self.base_table_data.table(table_name)?;
        let page_pointers = table.page_pointers();

        Some(
            self.page_reader
                .read_pages_of_pointers(page_pointers)
                .flat_map(|page| {
                    let page = match page {
                        Ok(page) => page,
                        Err(err) => return vec![Err(err)],
                    };
                    let page_id = page.header().page_id;
                    page.slotted_records()
                        .into_iter()
                        .filter(SlottedRecord::is_ghost)
                        .map(|record| {
                            Ok(GhostRow {
                                page_id,
                                slot: record.slot,
                                record_type: record.record_type,
                                bytes: record.bytes.to_vec(),
                            })
                        })
                        .collect()
                }),
        )
    }
}

/// A user table as the system catalog describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableInfo {
    /// Object id (`sysschobjs.id`).
    pub object_id: i32,
    /// Id of the table's schema (`sysschobjs.nsid`).
    pub schema_id: i32,
    /// Name of that schema (its `sysclsobjs` row, class 50).
    pub schema_name: String,
    /// Table name.
    pub name: String,
    /// Columns in column-id order.
    pub columns: Vec<ColumnInfo>,
    /// Rows the database counts for the table's heap or clustered index
    /// (`sysrowsets.rcrows`, summed over partitions).
    pub rcrows: i64,
}

/// One column of a [`TableInfo`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// The SQL Server type the reader parses the column as.
    pub type_name: String,
    /// Declared length in bytes (`-1` for `max`).
    pub max_length: i16,
    /// Declared precision.
    pub precision: u8,
    /// Declared scale.
    pub scale: u8,
}

/// A record [`MdfDatabase::scan_table`] read, with the page and slot it sits in.
#[derive(Debug)]
pub enum ScannedRecord {
    /// A live row.
    Live {
        /// Page the record sits on.
        page_id: u32,
        /// The record's slot in that page.
        slot: u16,
        /// Its column values; a `text` / `ntext` / `image` value holds the
        /// bytes read from its LOB pages.
        row: Row,
        /// Where each non-null `text` / `ntext` / `image` value of the row
        /// is rooted, in column order.
        lobs: Vec<LobRef>,
    },
    /// A ghost row, kept as stored.
    Ghost(GhostRow),
}

/// The root record of a LOB value: what the row's 16-byte text pointer
/// names, and where reading the value started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobRef {
    /// The column holding the value.
    pub column: String,
    /// Page of the root record.
    pub root_page: u32,
    /// Slot of the root record in that page.
    pub root_slot: u16,
    /// Bytes the LOB holds.
    pub length: usize,
}

/// The column types whose values live on text pages behind a text pointer.
fn is_lob_type(type_name: &str) -> bool {
    matches!(type_name, "text" | "ntext" | "image")
}

/// A LOB value as [`Value`]: `ntext` as UTF-16LE text, `image` as bytes,
/// and `text` as bytes too. A `text` column is single-byte text in the
/// code page of its collation, which the reader does not know; the bytes
/// are handed out as stored rather than decoded by a guess.
fn lob_value(type_name: &str, bytes: Vec<u8>) -> Value {
    match type_name {
        "ntext" => Value::String(encoding_rs::UTF_16LE.decode(&bytes).0.into_owned()),
        _ => Value::Binary(bytes),
    }
}

/// Parses one column off `record`: a LOB column through its text pointer
/// and the pages it names, any other through [`Value::parse`]. A LOB value
/// comes back with its [`LobRef`].
fn parse_column<'a>(
    column: &Column<'_>,
    record: Record<'a>,
    page_reader: &PageReader,
) -> Result<(Value, Option<LobRef>, Record<'a>), &'static str> {
    if !is_lob_type(column.r#type) {
        let (value, record) = Value::parse(column, record)?;
        return Ok((value, None, record));
    }
    let (pointer, record) = record.parse_text_pointer_opt()?;
    let Some(pointer) = pointer else {
        return Ok((Value::Null, None, record));
    };
    let bytes = page_reader.read_lob(&pointer)?;
    let lob = LobRef {
        column: column.name.to_string(),
        root_page: pointer.page.page_id,
        root_slot: pointer.slot,
        length: bytes.len(),
    };
    Ok((lob_value(column.r#type, bytes), Some(lob), record))
}

/// A row deleted from a table whose bytes its page still holds, because the
/// database had not reclaimed them yet. [`MdfDatabase::rows`] never yields it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhostRow {
    /// Page the record sits on.
    pub page_id: u32,
    /// The record's slot in that page.
    pub slot: u16,
    /// 5 ghost index, 6 ghost data or 7 ghost version record.
    pub record_type: u8,
    /// The record's bytes exactly as stored.
    pub bytes: Vec<u8>,
}

fn parse_record_columns_lenient(
    record: Record<'_>,
    columns: &[Column<'_>],
    page_reader: &PageReader,
) -> Row {
    let mut parsed_columns = BTreeMap::new();
    let mut record = Some(record);

    for column in columns {
        let Some(rec) = record.take() else {
            break;
        };
        let (value, _lob, remaining) = match parse_column(column, rec, page_reader) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!("Row parse stopped after column {column:?}: {err}");
                break;
            }
        };

        parsed_columns.insert(column.name.to_string(), value);
        record = Some(remaining);
    }

    Row {
        columns: parsed_columns,
    }
}

fn parse_record_columns(
    table_name: &str,
    record: Record<'_>,
    columns: &[Column<'_>],
    page_reader: &PageReader,
) -> Result<(Row, Vec<LobRef>), Error> {
    let mut parsed_columns = BTreeMap::new();
    let mut lobs = Vec::new();
    let mut record = Some(record);

    for column in columns {
        let Some(rec) = record.take() else {
            break;
        };
        let (value, lob, remaining) = match parse_column(column, rec, page_reader) {
            Ok(parsed) => parsed,
            Err(err) if is_omitted_trailing_column_error(err) && !parsed_columns.is_empty() => {
                warn!("Row parse stopped at omitted trailing column {column:?}: {err}");
                break;
            }
            Err(err) => {
                warn!("Row parse failed after column {column:?}: {err}");
                return Err(Error::RowParseError {
                    table: table_name.to_string(),
                    column: column.name.to_string(),
                    source: err,
                });
            }
        };

        parsed_columns.insert(column.name.to_string(), value);
        lobs.extend(lob);
        record = Some(remaining);
    }

    Ok((
        Row {
            columns: parsed_columns,
        },
        lobs,
    ))
}

fn is_omitted_trailing_column_error(err: &str) -> bool {
    err == "requested fixed-length bytes exceed record bounds"
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bit(bool),
    TinyInt(u8),
    SmallInt(i16),
    Int(i32),
    BigInt(i64),
    Real(f32),
    Float(f64),
    Decimal(Decimal),
    String(String),
    Binary(Vec<u8>),
    DateTime(DateTime<Utc>),
    Uuid(Uuid),
    Null,
}

impl Display for Value {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Value::Bit(bit) => write!(fmt, "{bit}"),
            Value::TinyInt(i) => write!(fmt, "{i}"),
            Value::SmallInt(i) => write!(fmt, "{i}"),
            Value::Int(i) => write!(fmt, "{i}"),
            Value::BigInt(i) => write!(fmt, "{i}"),
            Value::Real(f) => write!(fmt, "{f}"),
            Value::Float(f) => write!(fmt, "{f}"),
            Value::Decimal(decimal) => write!(fmt, "{decimal}"),
            Value::String(s) => write!(fmt, "{s}"),
            Value::Binary(b) => write!(fmt, "{b:?}"),
            Value::DateTime(d) => write!(fmt, "{d}"),
            Value::Uuid(uuid) => write!(fmt, "{uuid}"),
            Value::Null => write!(fmt, "null"),
        }
    }
}

impl Value {
    fn parse<'a>(
        column: &Column<'_>,
        record: Record<'a>,
    ) -> Result<(Self, Record<'a>), &'static str> {
        match column.r#type {
            "bit" => {
                let (bit, r) = record.parse_bit()?;
                Ok((Value::Bit(bit), r))
            }
            "datetime" => {
                let (parts, r) = record.parse_datetime_parts_opt()?;
                let datetime = parts
                    .map(|(days, ticks)| datetime_from_parts(days, ticks))
                    .transpose()?;
                Ok((datetime.map_or(Value::Null, Value::DateTime), r))
            }
            "datetime2" => {
                let (datetime, r) = record.parse_datetime2_opt(column.scale)?;
                Ok((datetime.map_or(Value::Null, Value::DateTime), r))
            }
            "smalldatetime" => {
                let (datetime, r) = record.parse_smalldatetime_opt()?;
                Ok((datetime.map_or(Value::Null, Value::DateTime), r))
            }
            "date" => {
                let (datetime, r) = record.parse_date_opt()?;
                Ok((datetime.map_or(Value::Null, Value::DateTime), r))
            }
            "tinyint" => {
                let (int, r) = record.parse_u8()?;
                Ok((Value::TinyInt(int), r))
            }
            "smallint" => {
                let (int, r) = record.parse_i16()?;
                Ok((Value::SmallInt(int), r))
            }
            "int" => {
                let (int, r) = record.parse_i32_opt()?;
                Ok((int.map_or(Value::Null, Value::Int), r))
            }
            "money" => {
                let (int, r) = record.parse_i64_opt()?;
                Ok((
                    int.map_or(Value::Null, |value| {
                        Value::Decimal(Decimal::from_i128_with_scale(value as i128, 4))
                    }),
                    r,
                ))
            }
            "bigint" => {
                let (int, r) = record.parse_i64_opt()?;
                Ok((int.map_or(Value::Null, Value::BigInt), r))
            }
            "real" => {
                let (float, r) = record.parse_f32_opt()?;
                Ok((float.map_or(Value::Null, Value::Real), r))
            }
            "float" => {
                let (float, r) = record.parse_f64_opt()?;
                Ok((float.map_or(Value::Null, Value::Float), r))
            }
            "char" => {
                let (string, r) =
                    record.parse_string_from_fixed_bytes(column.max_length as usize)?;
                Ok((Value::String(string), r))
            }
            "nchar" => {
                let (string, r) =
                    record.parse_utf16le_string_from_fixed_bytes(column.max_length as usize)?;
                Ok((Value::String(string), r))
            }
            "nvarchar" | "varchar" | "sysname" => {
                let (string, r) = record.parse_string()?;
                Ok((string.map_or(Value::Null, Value::String), r))
            }
            // Read through their text pointer by `parse_column`, which has
            // the pages; here they hold no in-row value to parse.
            "text" | "ntext" | "image" => {
                Err("LOB columns are read from their text pages, not the row alone")
            }
            "uniqueidentifier" => {
                let (uuid, r) = record.parse_uuid()?;
                Ok((Value::Uuid(uuid), r))
            }
            "decimal" | "numeric" => {
                let (decimal, r) = record.parse_decimal_opt(column.precision, column.scale)?;
                Ok((decimal.map_or(Value::Null, Value::Decimal), r))
            }
            "smallmoney" => {
                let (int, r) = record.parse_i32_opt()?;
                Ok((
                    int.map_or(Value::Null, |value| {
                        Value::Decimal(Decimal::from_i128_with_scale(value as i128, 4))
                    }),
                    r,
                ))
            }
            "varbinary" => {
                let (bytes, r) = record.parse_binary()?;
                Ok((bytes.map_or(Value::Null, Value::Binary), r))
            }
            "binary" | "timestamp" => {
                let (bytes, r) = record.parse_bytes(column.max_length as usize)?;
                Ok((Value::Binary(bytes.to_vec()), r))
            }
            _ => Err("Unknown column type"),
        }
    }
}

#[derive(Debug)]
pub struct Row {
    pub columns: BTreeMap<String, Value>,
}

impl Row {
    pub fn value(&self, column_name: &str) -> Option<&Value> {
        self.columns.get(column_name)
    }

    pub fn values(self) -> Vec<(String, Value)> {
        self.columns.into_iter().collect()
    }
}

/// The file's bytes, read a page at a time by page id. Only the primary
/// data file (file id 1) is known; a pointer into another file is an error.
struct PageReader {
    bytes: Vec<u8>,
}

/// LOB trees of the corpus are a root with data children (level 0) or a
/// root over internal nodes (level 1); a deeper tree is refused rather than
/// followed into a cycle.
const MAX_LOB_LEVEL: u16 = 2;

impl PageReader {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    fn page_count(&self) -> usize {
        self.bytes.len() / PAGE_SIZE
    }

    fn page_bytes(&self, page_pointer: &PagePointer) -> Result<[u8; PAGE_SIZE], Error> {
        if page_pointer.file_id != 1 {
            return Err(Error::ParseError(
                "page pointer names a file other than the primary data file",
            ));
        }
        let start = page_pointer.page_id as usize * PAGE_SIZE;
        let bytes = self
            .bytes
            .get(start..start + PAGE_SIZE)
            .ok_or(Error::ParseError("page id beyond the end of the file"))?;
        let mut buffer = [0u8; PAGE_SIZE];
        buffer.copy_from_slice(bytes);
        Ok(buffer)
    }

    fn read_boot_page(&self) -> Result<BootPage, Error> {
        let bytes = self.page_bytes(&PagePointer {
            page_id: BOOT_PAGE_ID,
            file_id: 1,
        })?;
        BootPage::try_from(bytes).map_err(Error::from)
    }

    fn read_page(&self, page_pointer: &PagePointer) -> Result<Rc<Page>, Error> {
        let bytes = self.page_bytes(page_pointer)?;
        Page::try_from(bytes).map(Rc::new).map_err(Error::from)
    }

    fn read_pages_of_pointers<'a, 'b: 'a>(
        &'b self,
        page_pointers: Vec<PagePointer>,
    ) -> PageIter<'a> {
        PageIter {
            page_pointers: Box::new(page_pointers.into_iter()),
            page_reader: self,
            current_page: None,
        }
    }

    fn read_pages_of_pointer<'a, 'b: 'a>(&'b self, page_pointer: PagePointer) -> PageIter<'a> {
        PageIter {
            page_pointers: Box::new(std::iter::once(page_pointer)),
            page_reader: self,
            current_page: None,
        }
    }

    /// The bytes of the LOB a text pointer names: its `LARGE_ROOT_YUKON`
    /// root, then every `DATA` fragment in link order, through `INTERNAL`
    /// nodes when the root is a level above them. Every record must carry
    /// the pointer's blob id, and the fragments must end exactly at the
    /// offsets the links state. Any other root shape -- a `SMALL_ROOT`
    /// holding its data inline, the older `LARGE_ROOT`, a
    /// `SUPER_LARGE_ROOT` -- is an error, not a guess.
    fn read_lob(&self, pointer: &TextPointer) -> Result<Vec<u8>, &'static str> {
        let root_page = self
            .read_page(&pointer.page)
            .map_err(|_| "LOB root page cannot be read")?;
        let (blob_id, root) = root_page.text_record(pointer.slot)?;
        if blob_id != pointer.blob_id {
            return Err("LOB root record carries another blob id");
        }
        let TextRecord::LargeRootYukon { level, links } = root else {
            return Err("LOB root is not a LARGE_ROOT_YUKON record; not supported");
        };
        if level >= MAX_LOB_LEVEL {
            return Err("LOB tree deeper than the corpus shows; not supported");
        }

        let mut bytes = Vec::new();
        let mut visited = HashSet::from([(pointer.page.page_id, pointer.slot)]);
        self.read_lob_links(pointer.blob_id, level, &links, &mut bytes, &mut visited)?;
        Ok(bytes)
    }

    /// Appends the children `links` name to `bytes`: `DATA` fragments under
    /// a node at level 0, `INTERNAL` nodes a level down otherwise.
    fn read_lob_links(
        &self,
        blob_id: u64,
        level: u16,
        links: &[LobLink],
        bytes: &mut Vec<u8>,
        visited: &mut HashSet<(u32, u16)>,
    ) -> Result<(), &'static str> {
        for link in links {
            if !visited.insert((link.page.page_id, link.slot)) {
                return Err("LOB links lead back to a record already read");
            }
            let page = self
                .read_page(&link.page)
                .map_err(|_| "LOB page cannot be read")?;
            let (record_blob_id, record) = page.text_record(link.slot)?;
            if record_blob_id != blob_id {
                return Err("LOB record carries another blob id");
            }
            match (level, record) {
                (0, TextRecord::Data(fragment)) => bytes.extend_from_slice(fragment),
                (0, _) => return Err("LOB link at level 0 does not lead to a DATA record"),
                (
                    _,
                    TextRecord::Internal {
                        level: child_level,
                        links: child_links,
                    },
                ) => {
                    if child_level + 1 != level {
                        return Err("LOB internal node is not one level below its parent");
                    }
                    self.read_lob_links(blob_id, child_level, &child_links, bytes, visited)?;
                }
                (_, _) => return Err("LOB link above level 0 does not lead to an INTERNAL record"),
            }
            if bytes.len() as u64 != link.end_offset {
                return Err("LOB fragments do not end at the offset their link states");
            }
        }
        Ok(())
    }
}

struct PageIter<'a> {
    page_pointers: Box<dyn Iterator<Item = PagePointer>>,
    page_reader: &'a PageReader,
    current_page: Option<Rc<Page>>,
}

impl<'a> Iterator for PageIter<'a> {
    type Item = Result<Rc<Page>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let page_pointer = match self.current_page.take() {
            Some(current_page) => current_page.next_page_pointer().cloned(),
            None => self.page_pointers.next(),
        };

        match page_pointer {
            Some(page_pointer) => {
                let page = self.page_reader.read_page(&page_pointer);

                if let Ok(current_page) = &page {
                    self.current_page = Some(current_page.clone());
                }

                Some(page)
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::Column;
    use chrono::{Duration, TimeZone};
    use rust_decimal::Decimal;

    fn test_column(
        r#type: &'static str,
        max_length: i16,
        precision: u8,
        scale: u8,
    ) -> Column<'static> {
        Column {
            name: "value",
            r#type,
            max_length,
            precision,
            scale,
        }
    }

    fn fixed_record_bytes(fixed_bytes: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(6 + fixed_bytes.len());
        bytes.extend_from_slice(&[0u8, 0u8]);
        bytes.extend_from_slice(&((4 + fixed_bytes.len()) as u16).to_le_bytes());
        bytes.extend_from_slice(fixed_bytes);
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes
    }

    #[test]
    fn should_result_in_io_error_when_file_does_not_exists() {
        match MdfDatabase::open("some-random-path") {
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::NotFound => {}
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn a_lob_is_text_only_when_its_column_is_ntext() {
        let utf16 = b"a\0\xe4\x00".to_vec();
        assert_eq!(
            Value::String(String::from("a\u{e4}")),
            lob_value("ntext", utf16.clone())
        );
        // Single-byte text in an unknown code page stays bytes, as image does.
        assert_eq!(
            Value::Binary(utf16.clone()),
            lob_value("text", utf16.clone())
        );
        assert_eq!(Value::Binary(utf16.clone()), lob_value("image", utf16));
    }

    #[test]
    fn money_values_are_scaled_decimal_and_consume_eight_bytes() {
        let mut fixed = 1_234_567i64.to_le_bytes().to_vec();
        fixed.extend_from_slice(&42i32.to_le_bytes());
        let bytes = fixed_record_bytes(&fixed);
        let record = Record::try_from(&bytes[..]).unwrap();

        let (value, record) =
            Value::parse(&test_column("money", 8, 19, 4), record).expect("parse money");
        let (next, _record) =
            Value::parse(&test_column("int", 4, 10, 0), record).expect("parse following int");

        assert_eq!(Value::Decimal(Decimal::new(1_234_567, 4)), value);
        assert_eq!(Value::Int(42), next);
    }

    #[test]
    fn smallmoney_values_are_scaled_decimal() {
        let bytes = fixed_record_bytes(&123_456i32.to_le_bytes());
        let record = Record::try_from(&bytes[..]).unwrap();

        let (value, _record) =
            Value::parse(&test_column("smallmoney", 4, 10, 4), record).expect("parse smallmoney");

        assert_eq!(Value::Decimal(Decimal::new(123_456, 4)), value);
    }

    #[test]
    fn datetime2_uses_scale_specific_length_and_preserves_following_columns() {
        let mut fixed = vec![1u8, 0u8, 0u8, 0u8, 0u8, 0u8];
        fixed.extend_from_slice(&42i32.to_le_bytes());
        let bytes = fixed_record_bytes(&fixed);
        let record = Record::try_from(&bytes[..]).unwrap();

        let (value, record) =
            Value::parse(&test_column("datetime2", 6, 0, 2), record).expect("parse datetime2");
        let (next, _record) =
            Value::parse(&test_column("int", 4, 10, 0), record).expect("parse following int");

        let expected = Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0).unwrap() + Duration::milliseconds(10);
        assert_eq!(Value::DateTime(expected), value);
        assert_eq!(Value::Int(42), next);
    }

    #[test]
    fn tinyint_values_preserve_unsigned_range() {
        let bytes = fixed_record_bytes(&[255u8]);
        let record = Record::try_from(&bytes[..]).unwrap();

        let (value, _record) =
            Value::parse(&test_column("tinyint", 1, 3, 0), record).expect("parse tinyint");

        assert_eq!("255", value.to_string());
    }

    #[test]
    fn nchar_fixed_bytes_decode_as_utf16le() {
        let bytes = fixed_record_bytes(&[0x2d, 0x4e]);
        let record = Record::try_from(&bytes[..]).unwrap();

        let (value, _record) =
            Value::parse(&test_column("nchar", 2, 0, 0), record).expect("parse nchar");

        assert_eq!(Value::String(String::from("中")), value);
    }

    #[test]
    fn row_column_parse_failure_returns_error_instead_of_partial_row() {
        let bytes = fixed_record_bytes(&[1u8, 0u8]);
        let record = Record::try_from(&bytes[..]).unwrap();
        let columns = vec![test_column("int", 4, 10, 0)];

        let err = parse_record_columns("T_Test", record, &columns, &PageReader::new(Vec::new()))
            .expect_err("truncated column should fail the whole row");

        assert!(matches!(
            err,
            Error::RowParseError {
                table,
                column,
                source: "requested fixed-length bytes exceed record bounds"
            } if table == "T_Test" && column == "value"
        ));
    }
}
