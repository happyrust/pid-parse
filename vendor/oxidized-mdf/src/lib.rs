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
// Modified: 2026-11-16 by happyrust
//   - PageReader reads the page it is asked for from a Read + Seek source,
//     not every page up to it; a page is parsed once; the page cache is capped
//   - MdfDatabase::from_read takes a Read + Seek source

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
use crate::pages::{BootPage, Page, PagePointer, Record};
use crate::sys::{BaseTableData, Column};
use chrono::{DateTime, Utc};
use core::fmt::{Display, Formatter};
use log::{error, warn};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use uuid::Uuid;

/// The page of an `.mdf` that holds its boot record.
const BOOT_PAGE_ID: u32 = 9;

pub struct MdfDatabase {
    page_reader: PageReader,
    boot_page: BootPage,
    pub(crate) base_table_data: BaseTableData,
}

impl MdfDatabase {
    pub fn open<P>(p: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let mut path = PathBuf::new();
        path.push(p);

        let file = File::open(&path)?;
        Self::from_read(file)
    }

    /// Reads a database out of `read`, which holds an `.mdf` from its first
    /// byte on.
    ///
    /// Pages are read from where they lie in the file, so `read` has to seek.
    pub fn from_read<R>(read: R) -> Result<Self, Error>
    where
        R: Read + Seek + 'static,
    {
        let mut page_reader = PageReader::new(Box::new(read));

        let mut buffer = [0u8; 8192];
        page_reader.read_page_bytes(BOOT_PAGE_ID, &mut buffer)?;

        let boot_page = BootPage::try_from(buffer).map_err(Error::from)?;
        let base_table_data = BaseTableData::parse(&mut page_reader, &boot_page)?;

        Ok(Self {
            page_reader,
            boot_page,
            base_table_data,
        })
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
    pub fn rows<'a, 'b: 'a>(
        &'b mut self,
        table_name: &str,
    ) -> Option<impl Iterator<Item = Row> + 'a> {
        let table = self.base_table_data.table(table_name)?;
        let page_pointers = table.page_pointers();
        let columns = table.columns;

        log::debug!("reading pages of {table_name}");
        Some(
            self.page_reader
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
                        rows.push(parse_record_columns_lenient(record, &columns));
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
        &'b mut self,
        table_name: &str,
    ) -> Option<impl Iterator<Item = Result<Row, Error>> + 'a> {
        let table = self.base_table_data.table(table_name)?;
        let page_pointers = table.page_pointers();
        let columns = table.columns;
        let table_name = table_name.to_string();

        log::debug!("reading pages of {table_name}");
        Some(
            self.page_reader
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
                        rows.push(parse_record_columns(&table_name, record, &columns));
                    }
                    rows
                }),
        )
    }
}

fn parse_record_columns_lenient(record: Record<'_>, columns: &[Column<'_>]) -> Row {
    let mut parsed_columns = BTreeMap::new();
    let mut record = Some(record);

    for column in columns {
        let Some(rec) = record.take() else {
            break;
        };
        let (value, remaining) = match Value::parse(column, rec) {
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
) -> Result<Row, Error> {
    let mut parsed_columns = BTreeMap::new();
    let mut record = Some(record);

    for column in columns {
        let Some(rec) = record.take() else {
            break;
        };
        let (value, remaining) = match Value::parse(column, rec) {
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
        record = Some(remaining);
    }

    Ok(Row {
        columns: parsed_columns,
    })
}

fn is_omitted_trailing_column_error(err: &str) -> bool {
    matches!(
        err,
        "requested fixed-length bytes exceed record bounds" | "no variable column data"
    )
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
                let (datetime, r) = record.parse_datetime_opt()?;
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
            "text" | "ntext" => {
                let (string, r) = record.parse_string()?;
                Ok((string.map_or(Value::Null, Value::String), r))
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
            "varbinary" | "image" => {
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

/// What a [`PageReader`] reads its pages from.
trait PageSource: Read + Seek {}

impl<T: Read + Seek> PageSource for T {}

/// How many pages a [`PageReader`] keeps, which is 2 MiB of page bytes.
///
/// This bounds what a reader holds however large the file is. A page that is
/// asked for again after the reader let it go is read from the source again.
const PAGE_CACHE_LIMIT: usize = 256;

struct PageReader {
    read: Box<dyn PageSource>,
    page_cache: PageCache,
}

impl PageReader {
    fn new(read: Box<dyn PageSource>) -> Self {
        Self {
            read,
            page_cache: PageCache::default(),
        }
    }

    /// Reads the bytes of page `page_id` from where they lie in the source.
    fn read_page_bytes(&mut self, page_id: u32, buffer: &mut [u8; 8192]) -> Result<(), Error> {
        self.read.seek(SeekFrom::Start(u64::from(page_id) * 8192))?;
        self.read.read_exact(&mut buffer[..])?;
        Ok(())
    }

    /// The page `page_pointer` names, read from the source unless the reader
    /// still keeps it.
    ///
    /// Only the page id of the pointer is looked at: the reader has one file
    /// to read from, whichever file the pointer names.
    fn read_page(&mut self, page_pointer: &PagePointer) -> Result<Rc<Page>, Error> {
        let page_id = page_pointer.page_id;
        if let Some(page) = self.page_cache.get(page_id) {
            return Ok(page);
        }

        let mut buffer = [0u8; 8192];
        self.read_page_bytes(page_id, &mut buffer)?;
        let page = Rc::new(Page::try_from(buffer).map_err(Error::from)?);

        self.page_cache.keep(page_id, page.clone());
        Ok(page)
    }

    fn read_pages_of_pointers<'a, 'b: 'a>(
        &'b mut self,
        page_pointers: Vec<PagePointer>,
    ) -> PageIter<'a> {
        PageIter {
            page_pointers: Box::new(page_pointers.into_iter()),
            page_reader: self,
            current_page: None,
        }
    }

    fn read_pages_of_pointer<'a, 'b: 'a>(&'b mut self, page_pointer: PagePointer) -> PageIter<'a> {
        PageIter {
            page_pointers: Box::new(std::iter::once(page_pointer)),
            page_reader: self,
            current_page: None,
        }
    }
}

/// The pages a [`PageReader`] read last, [`PAGE_CACHE_LIMIT`] of them at most.
#[derive(Default)]
struct PageCache {
    pages: HashMap<u32, Rc<Page>>,
    /// The ids of `pages`, the one read longest ago first.
    oldest_first: VecDeque<u32>,
}

impl PageCache {
    fn get(&self, page_id: u32) -> Option<Rc<Page>> {
        self.pages.get(&page_id).cloned()
    }

    /// Keeps `page`, and lets the page read longest ago go when that makes
    /// one more than the limit.
    fn keep(&mut self, page_id: u32, page: Rc<Page>) {
        if self.pages.insert(page_id, page).is_some() {
            return;
        }

        self.oldest_first.push_back(page_id);
        if self.oldest_first.len() > PAGE_CACHE_LIMIT {
            if let Some(oldest) = self.oldest_first.pop_front() {
                self.pages.remove(&oldest);
            }
        }
    }

    fn len(&self) -> usize {
        self.pages.len()
    }
}

struct PageIter<'a> {
    page_pointers: Box<dyn Iterator<Item = PagePointer>>,
    page_reader: &'a mut PageReader,
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
    use std::cell::Cell;
    use std::io::Cursor;

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

    /// A source that counts the bytes it hands out.
    struct Counting {
        image: Cursor<Vec<u8>>,
        read: Rc<Cell<u64>>,
    }

    impl Read for Counting {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let count = self.image.read(buffer)?;
            self.read.set(self.read.get() + count as u64);
            Ok(count)
        }
    }

    impl Seek for Counting {
        fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
            self.image.seek(to)
        }
    }

    /// An image of `pages` pages that link to nothing. A page's slot count is
    /// its own page id, which is how [`id_of`] tells one from another.
    fn image_of(pages: u32) -> Vec<u8> {
        let mut image = vec![0u8; pages as usize * 8192];
        for page_id in 0..pages {
            let slot_count = page_id as usize * 8192 + 22;
            let id = u16::try_from(page_id).expect("page id fits a slot count");
            image[slot_count..slot_count + 2].copy_from_slice(&id.to_le_bytes());
        }
        image
    }

    /// Makes page `from` of `image` name page `to` as the next of its chain.
    fn link(image: &mut [u8], from: u32, to: u32) {
        let next_page = from as usize * 8192 + 16;
        image[next_page..next_page + 4].copy_from_slice(&to.to_le_bytes());
        image[next_page + 4..next_page + 6].copy_from_slice(&1u16.to_le_bytes());
    }

    /// A reader over `image`, and the count of the bytes it has read from it.
    fn reader_over(image: Vec<u8>) -> (PageReader, Rc<Cell<u64>>) {
        let read = Rc::new(Cell::new(0));
        let source = Counting {
            image: Cursor::new(image),
            read: read.clone(),
        };
        (PageReader::new(Box::new(source)), read)
    }

    fn pointer_to(page_id: u32) -> PagePointer {
        PagePointer {
            page_id,
            file_id: 1,
        }
    }

    fn id_of(page: &Page) -> u32 {
        u32::from(page.header().slot_count)
    }

    #[test]
    fn should_result_in_io_error_when_file_does_not_exists() {
        match MdfDatabase::open("some-random-path") {
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::NotFound => {}
            _ => panic!("Unexpected result"),
        }
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

        let err = parse_record_columns("T_Test", record, &columns)
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

    #[test]
    fn a_page_reads_without_the_pages_before_it() {
        let (mut reader, read) = reader_over(image_of(16));

        let page = reader.read_page(&pointer_to(12)).expect("read page 12");

        assert_eq!(12, id_of(&page));
        assert_eq!(8192, read.get());
    }

    #[test]
    fn an_earlier_page_reads_after_a_later_one() {
        let (mut reader, _read) = reader_over(image_of(16));

        let later = reader.read_page(&pointer_to(12)).expect("read page 12");
        let earlier = reader.read_page(&pointer_to(3)).expect("read page 3");

        assert_eq!((12, 3), (id_of(&later), id_of(&earlier)));
    }

    #[test]
    fn a_chain_is_followed_to_wherever_its_pages_lie() {
        let mut image = image_of(16);
        link(&mut image, 12, 3);
        link(&mut image, 3, 7);
        let (mut reader, read) = reader_over(image);

        let chain: Vec<u32> = reader
            .read_pages_of_pointer(pointer_to(12))
            .map(|page| id_of(&page.expect("read page")))
            .collect();

        assert_eq!(vec![12, 3, 7], chain);
        assert_eq!(3 * 8192, read.get());
    }

    #[test]
    fn a_page_the_reader_keeps_is_not_read_again() {
        let (mut reader, read) = reader_over(image_of(16));

        let first = reader.read_page(&pointer_to(5)).expect("read page 5");
        let again = reader.read_page(&pointer_to(5)).expect("read page 5 again");

        assert!(Rc::ptr_eq(&first, &again));
        assert_eq!(8192, read.get());
    }

    #[test]
    fn the_reader_keeps_no_more_pages_than_its_limit() {
        let pages = PAGE_CACHE_LIMIT as u32 + 8;
        let (mut reader, _read) = reader_over(image_of(pages));

        for page_id in 0..pages {
            reader.read_page(&pointer_to(page_id)).expect("read page");
        }

        assert_eq!(PAGE_CACHE_LIMIT, reader.page_cache.len());
    }

    #[test]
    fn a_page_the_reader_let_go_reads_again() {
        let pages = PAGE_CACHE_LIMIT as u32 + 8;
        let (mut reader, read) = reader_over(image_of(pages));
        for page_id in 0..pages {
            reader.read_page(&pointer_to(page_id)).expect("read page");
        }
        let read_so_far = read.get();

        let first = reader.read_page(&pointer_to(0)).expect("read page 0 again");

        assert_eq!(0, id_of(&first));
        assert_eq!(8192, read.get() - read_so_far);
    }

    #[test]
    fn a_page_past_the_end_is_an_io_error_and_the_ones_before_it_still_read() {
        let (mut reader, _read) = reader_over(image_of(4));

        match reader.read_page(&pointer_to(4)) {
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {}
            _ => panic!("Unexpected result"),
        }

        let last = reader.read_page(&pointer_to(3)).expect("read page 3");
        assert_eq!(3, id_of(&last));
    }

    #[test]
    fn an_image_that_ends_before_its_boot_page_is_an_io_error() {
        let image = Cursor::new(vec![0u8; 9 * 8192]);

        match MdfDatabase::from_read(image) {
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {}
            _ => panic!("Unexpected result"),
        }
    }
}
