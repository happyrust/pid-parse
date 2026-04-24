// SPDX-License-Identifier: GPL-3.0-or-later
// Original: oxidized-mdf by schrieveslaach (https://gitlab.com/schrieveslaach/oxidized-mdf)
// Modified: 2026-04-24 by happyrust
//   - Removed async runtime (async-std, futures-lite, async-log)
//   - Converted public API to sync (MdfDatabase::open, db.rows)
//   - Eliminated panic edges: unwrap/todo replaced with Result propagation
//   - Changed read() to read_exact() for page integrity
//   - Upgraded to edition 2021, uuid 1.x

#![allow(dead_code)]

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
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use uuid::Uuid;

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
        Self::from_read(Box::new(file))
    }

    pub fn from_read(read: Box<dyn Read>) -> Result<Self, Error> {
        let mut buffer = [0u8; 8192];
        let mut page_reader = PageReader::new(read);

        for _i in 0u8..9u8 {
            page_reader.read_next_page(&mut buffer)?;
        }
        page_reader.read_next_page(&mut buffer)?;

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

        log::debug!("reading pages of {}", table_name);
        Some(
            self.page_reader
                .read_pages_of_pointers(page_pointers)
                .flat_map(move |page| {
                    let mut rows = Vec::new();

                    let page = match page {
                        Ok(page) => page,
                        Err(err) => {
                            error!("Cannot read page: {}", err);
                            return rows;
                        }
                    };

                    for record in page.records().into_iter() {
                        let mut columns = BTreeMap::new();

                        let mut record = Some(record);
                        for column in &table.columns {
                            let Some(rec) = record.take() else {
                                break;
                            };
                            let (value, r) = match Value::parse(column, rec) {
                                Ok((value, r)) => (value, r),
                                Err(e) => {
                                    warn!(
                                        "Column {:?} parse skipped (NULL): {}",
                                        column, e
                                    );
                                    break;
                                }
                            };

                            columns.insert(column.name.to_string(), value);

                            record = Some(r);
                        }

                        rows.push(Row { columns });
                    }
                    rows
                }),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bit(bool),
    TinyInt(i8),
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
            Value::Bit(bit) => write!(fmt, "{}", bit),
            Value::TinyInt(i) => write!(fmt, "{}", i),
            Value::SmallInt(i) => write!(fmt, "{}", i),
            Value::Int(i) => write!(fmt, "{}", i),
            Value::BigInt(i) => write!(fmt, "{}", i),
            Value::Real(f) => write!(fmt, "{}", f),
            Value::Float(f) => write!(fmt, "{}", f),
            Value::Decimal(decimal) => write!(fmt, "{}", decimal),
            Value::String(s) => write!(fmt, "{}", s),
            Value::Binary(b) => write!(fmt, "{:?}", b),
            Value::DateTime(d) => write!(fmt, "{}", d),
            Value::Uuid(uuid) => write!(fmt, "{}", uuid),
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
                let (int, r) = record.parse_i8()?;
                Ok((Value::TinyInt(int), r))
            }
            "smallint" => {
                let (int, r) = record.parse_i16()?;
                Ok((Value::SmallInt(int), r))
            }
            "int" | "money" => {
                let (int, r) = record.parse_i32_opt()?;
                Ok((int.map_or(Value::Null, Value::Int), r))
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
            "char" | "nchar" => {
                let (string, r) =
                    record.parse_string_from_fixed_bytes(column.max_length as usize)?;
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
                Ok((int.map_or(Value::Null, Value::Int), r))
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

struct PageReader {
    read: Box<dyn Read>,
    page_index: u32,
    page_cache: HashMap<PagePointer, Rc<Page>>,
}

impl PageReader {
    fn new(read: Box<dyn Read>) -> Self {
        Self {
            read,
            page_index: 0,
            page_cache: HashMap::new(),
        }
    }

    fn read_next_page(&mut self, buffer: &mut [u8; 8192]) -> Result<(), Error> {
        let page_id = self.page_index;
        self.read.read_exact(&mut buffer[..])?;
        if let Ok(page) = Page::try_from(*buffer) {
            self.page_cache.insert(
                PagePointer {
                    page_id,
                    file_id: 1,
                },
                Rc::new(page),
            );
        }
        self.page_index += 1;
        Ok(())
    }

    fn read_page(&mut self, page_pointer: &PagePointer) -> Result<Rc<Page>, Error> {
        if let Some(page) = self.page_cache.get(page_pointer) {
            return Ok(page.clone());
        }
        if let Some((_, page)) = self
            .page_cache
            .iter()
            .find(|(pointer, _)| pointer.page_id == page_pointer.page_id)
        {
            return Ok(page.clone());
        }

        if self.page_index > page_pointer.page_id {
            return Err(Error::ParseError(
                "forward-only reader cannot re-read an earlier page",
            ));
        }

        for i in self.page_index..=page_pointer.page_id {
            let mut buffer = [0u8; 8192];
            self.read_next_page(&mut buffer)?;

            let page = Page::try_from(buffer).map_err(Error::from)?;

            self.page_cache
                .insert(page_pointer.with_page_id(i), Rc::new(page));
        }

        self.page_cache
            .get(page_pointer)
            .cloned()
            .ok_or(Error::ParseError("page not found in cache after read"))
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

    fn read_pages_of_pointer<'a, 'b: 'a>(
        &'b mut self,
        page_pointer: PagePointer,
    ) -> PageIter<'a> {
        PageIter {
            page_pointers: Box::new(std::iter::once(page_pointer)),
            page_reader: self,
            current_page: None,
        }
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

    #[test]
    fn should_result_in_io_error_when_file_does_not_exists() {
        match MdfDatabase::open("some-random-path") {
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::NotFound => {}
            _ => panic!("Unexpected result"),
        }
    }
}
