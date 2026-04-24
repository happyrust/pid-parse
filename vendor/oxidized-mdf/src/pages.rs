// SPDX-License-Identifier: GPL-3.0-or-later
// Original: oxidized-mdf by schrieveslaach (https://gitlab.com/schrieveslaach/oxidized-mdf)
// Modified: 2026-04-24 by happyrust
//   - Migrated record/page envelope parsing to nom 8 take helpers
//   - Replaced all ReadBytesExt::unwrap() with from_le_bytes (panic-free)
//   - Hardened VariableColumns to return Err on descending end offsets
//   - Hardened Page::slots() to skip+log impossible slot directory sizes
//   - Removed byteorder dependency; all byte decoding via nom or from_le_bytes
//   - Added manual i24 sign-extension for datetime2
//   - Extended column type coverage to 27 SQL Server types

use bitvec::{order::Lsb0, slice::BitSlice};
use chrono::{DateTime, Duration, TimeZone, Utc};
use nom::{bytes::complete::take, error::Error as NomError, Parser};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct PageHeader {
    pub(crate) slot_count: u16,
    pub(crate) next_page_pointer: Option<PagePointer>,
}

#[derive(Debug)]
pub struct BootPage {
    pub(crate) header: PageHeader,
    pub(crate) database_name: String,
    pub(crate) first_sys_indexes: PagePointer,
}

#[derive(Debug)]
pub(crate) struct Record<'a> {
    fixed_bytes: &'a [u8],
    r#type: RecordType,
    null_bitmap: Option<NullBitmap<'a>>,
    variable_columns: Option<VariableColumns<'a>>,
}

#[derive(Debug)]
enum RecordType {
    Primary,
    Forwarded,
    ForwardingStub,
    Index,
    BlobFragment,
    GhostIndex,
    GhostData,
    GhostVersion,
}

fn take_bytes<'a>(
    input: &'a [u8],
    len: usize,
    err: &'static str,
) -> Result<(&'a [u8], &'a [u8]), &'static str> {
    take::<_, _, NomError<&'a [u8]>>(len)
        .parse(input)
        .map_err(|_| err)
}

fn parse_le_u16<'a>(input: &'a [u8], err: &'static str) -> Result<(&'a [u8], u16), &'static str> {
    let (input, bytes) = take_bytes(input, 2, err)?;
    Ok((input, u16::from_le_bytes([bytes[0], bytes[1]])))
}

fn parse_le_u32<'a>(input: &'a [u8], err: &'static str) -> Result<(&'a [u8], u32), &'static str> {
    let (input, bytes) = take_bytes(input, 4, err)?;
    Ok((input, u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])))
}

fn parse_le_i16<'a>(input: &'a [u8], err: &'static str) -> Result<(&'a [u8], i16), &'static str> {
    let (input, bytes) = take_bytes(input, 2, err)?;
    Ok((input, i16::from_le_bytes([bytes[0], bytes[1]])))
}

impl<'a> TryFrom<&'a [u8]> for Record<'a> {
    type Error = &'static str;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        let (bytes, header_bytes) = take_bytes(bytes, 2, "record too short for header")?;

        // Bits 1-3 represents record type
        let record_type = (header_bytes[0] & 0b0000_1110) >> 1;
        let r#type = match record_type {
            0 => RecordType::Primary,
            1 => RecordType::Forwarded,
            2 => RecordType::ForwardingStub,
            3 => RecordType::Index,
            4 => RecordType::BlobFragment,
            5 => RecordType::GhostIndex,
            6 => RecordType::GhostData,
            7 => RecordType::GhostVersion,
            _ => return Err("unknown record type"),
        };

        // Bit 4 determines whether a null bitmap is present
        let has_null_bitmap = (header_bytes[0] & 0b0001_0000) > 0;

        // Bit 5 determines whether there are variable length columns
        let has_variable_length_columns = (header_bytes[0] & 0b0010_0000) > 0;

        let mut read_bytes = 2usize;

        // Parse fixed length size
        let (bytes, fixed_length_size) =
            parse_le_u16(bytes, "record too short for fixed-length size")?;
        let fixed_length_size = fixed_length_size
            .checked_sub(4)
            .ok_or("record fixed-length size smaller than header")?;
        read_bytes += 2;

        let (bytes, fixed_bytes) = take_bytes(
            bytes,
            fixed_length_size as usize,
            "record fixed-length region exceeds record bounds",
        )?;
        read_bytes += fixed_length_size as usize;

        let (bytes, number_of_columns) = parse_le_u16(bytes, "record too short for column count")?;
        let number_of_columns = number_of_columns as usize;
        read_bytes += 2;

        let (null_bitmap, bytes) = if has_null_bitmap {
            let null_bitmap_length = (number_of_columns + 7) / 8;
            let (bytes, null_bitmap) =
                take_bytes(bytes, null_bitmap_length, "record too short for null bitmap")?;
            read_bytes += null_bitmap_length;
            (Some(null_bitmap), bytes)
        } else {
            (None, bytes)
        };

        let variable_columns = if has_variable_length_columns {
            Some(VariableColumns::try_new(read_bytes, bytes)?)
        } else {
            None
        };

        Ok(Self {
            fixed_bytes,
            r#type,
            null_bitmap: null_bitmap.map(NullBitmap::new),
            variable_columns,
        })
    }
}

impl<'a> Record<'a> {
    pub(crate) fn has_variable_length_columns(&self) -> bool {
        self.variable_columns.is_some()
    }

    pub(crate) fn parse_i8(self) -> Result<(i8, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes(1)?;
        Ok((bytes[0] as i8, record))
    }

    pub(crate) fn parse_i16(self) -> Result<(i16, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes(2)?;
        Ok((i16::from_le_bytes([bytes[0], bytes[1]]), record))
    }

    pub(crate) fn parse_i32(self) -> Result<(i32, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes(4)?;
        Ok((i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), record))
    }

    pub(crate) fn parse_i32_opt(self) -> Result<(Option<i32>, Record<'a>), &'static str> {
        self.parse_bytes_opt(4).map(|(bytes, record)| {
            (
                bytes.map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                record,
            )
        })
    }

    pub(crate) fn parse_f32_opt(self) -> Result<(Option<f32>, Record<'a>), &'static str> {
        self.parse_bytes_opt(4).map(|(bytes, record)| {
            (
                bytes.map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                record,
            )
        })
    }

    pub(crate) fn parse_i64(self) -> Result<(i64, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes(8)?;
        Ok((
            i64::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]),
            record,
        ))
    }

    pub(crate) fn parse_i64_opt(self) -> Result<(Option<i64>, Record<'a>), &'static str> {
        self.parse_bytes_opt(8).map(|(bytes, record)| {
            (
                bytes.map(|b| {
                    i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
                }),
                record,
            )
        })
    }

    pub(crate) fn parse_f64_opt(self) -> Result<(Option<f64>, Record<'a>), &'static str> {
        self.parse_bytes_opt(8).map(|(bytes, record)| {
            (
                bytes.map(|b| {
                    f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
                }),
                record,
            )
        })
    }

    fn parse_u128(self) -> Result<(u128, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes(16)?;
        Ok((
            u128::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
            ]),
            record,
        ))
    }

    pub(crate) fn parse_decimal_opt(
        self,
        precision: u8,
        scale: u8,
    ) -> Result<(Option<Decimal>, Record<'a>), &'static str> {
        let required_storage_bytes = 1 + if precision <= 9 {
            4
        } else if precision <= 19 {
            2 * 4
        } else if precision <= 28 {
            3 * 4
        } else {
            4 * 4
        };

        let (bytes, record) = self.parse_bytes_opt(required_storage_bytes)?;
        Ok((
            bytes.map(|bytes| {
                let (sign_byte, bytes) = bytes.split_at(1usize);

                let x = if precision <= 9 {
                    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i128
                } else if precision <= 19 {
                    i64::from_le_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3],
                        bytes[4], bytes[5], bytes[6], bytes[7],
                    ]) as i128
                } else if precision <= 28 {
                    let mut padded = [0u8; 16];
                    padded[..12].copy_from_slice(bytes);
                    i128::from_le_bytes(padded)
                } else {
                    i128::from_le_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3],
                        bytes[4], bytes[5], bytes[6], bytes[7],
                        bytes[8], bytes[9], bytes[10], bytes[11],
                        bytes[12], bytes[13], bytes[14], bytes[15],
                    ])
                };

                let mut decimal = Decimal::from_i128_with_scale(x, scale as u32);
                decimal.set_sign_positive(sign_byte[0] != 0);
                decimal
            }),
            record,
        ))
    }

    pub(crate) fn parse_bit(self) -> Result<(bool, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes(1)?;

        Ok((bytes[0] > 0, record))
    }

    const CLOCK_TICK_MS: f64 = 10.0 / 3.0;

    pub(crate) fn parse_datetime_opt(
        self,
    ) -> Result<(Option<DateTime<Utc>>, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes_opt(8)?;

        let datetime = match bytes {
            Some(bytes) => {
                let time = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                let days = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

                let datetime = Utc
                    .with_ymd_and_hms(1900, 1, 1, 0, 0, 0)
                    .single()
                    .ok_or("Cannot construct datetime epoch 1900-01-01")?
                    .checked_add_signed(Duration::milliseconds(
                        (time as f64 * Self::CLOCK_TICK_MS) as i64,
                    ))
                    .ok_or("Cannot parse datetime due to overflow")?
                    .checked_add_signed(Duration::days(days as i64))
                    .ok_or("Cannot parse datetime due to overflow")?;

                Some(datetime)
            }
            None => None,
        };

        Ok((datetime, record))
    }

    pub(crate) fn parse_smalldatetime_opt(
        self,
    ) -> Result<(Option<DateTime<Utc>>, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes_opt(4)?;

        let datetime = match bytes {
            Some(bytes) => {
                let days = u16::from_le_bytes([bytes[0], bytes[1]]);
                let minutes = u16::from_le_bytes([bytes[2], bytes[3]]);

                let datetime = Utc
                    .with_ymd_and_hms(1900, 1, 1, 0, 0, 0)
                    .single()
                    .ok_or("Cannot construct smalldatetime epoch 1900-01-01")?
                    .checked_add_signed(Duration::days(days as i64))
                    .ok_or("Cannot parse smalldatetime due to day overflow")?
                    .checked_add_signed(Duration::minutes(minutes as i64))
                    .ok_or("Cannot parse smalldatetime due to minute overflow")?;

                Some(datetime)
            }
            None => None,
        };

        Ok((datetime, record))
    }

    pub(crate) fn parse_date_opt(
        self,
    ) -> Result<(Option<DateTime<Utc>>, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes_opt(3)?;

        let datetime = match bytes {
            Some(bytes) => {
                let mut day_buf = [0u8; 4];
                day_buf[0] = bytes[0];
                day_buf[1] = bytes[1];
                day_buf[2] = bytes[2];
                let days = i32::from_le_bytes(day_buf);

                let datetime = Utc
                    .with_ymd_and_hms(1, 1, 1, 0, 0, 0)
                    .single()
                    .ok_or("Cannot construct date epoch 0001-01-01")?
                    .checked_add_signed(Duration::days(days as i64))
                    .ok_or("Cannot parse date due to day overflow")?;

                Some(datetime)
            }
            None => None,
        };

        Ok((datetime, record))
    }

    pub(crate) fn parse_datetime2_opt(
        self,
        scale: u8,
    ) -> Result<(Option<DateTime<Utc>>, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes_opt(8)?;

        let datetime = match bytes {
            Some(bytes) => {
                let bytes_of_time = if scale <= 2 {
                    3
                } else if (3..=4).contains(&scale) {
                    4
                } else {
                    5
                };

                // TODO: include time in the calculation
                let d = bytes_of_time;
                let mut day_buf = [0u8; 4];
                day_buf[0] = bytes[d];
                day_buf[1] = bytes[d + 1];
                day_buf[2] = bytes[d + 2];
                if bytes[d + 2] & 0x80 != 0 {
                    day_buf[3] = 0xFF;
                }
                let days = i32::from_le_bytes(day_buf);

                let datetime = Utc
                    .with_ymd_and_hms(1, 1, 1, 0, 0, 0)
                    .single()
                    .ok_or("Cannot construct datetime epoch 0001-01-01")?
                    .checked_add_signed(Duration::days(days as i64))
                    .ok_or("Cannot parse datetime due to overflow")?;

                Some(datetime)
            }
            None => None,
        };

        Ok((datetime, record))
    }

    pub(crate) fn parse_bytes(self, len: usize) -> Result<(&'a [u8], Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes_opt(len)?;

        match bytes {
            Some(bytes) => Ok((bytes, record)),
            None => Err("Requested none null bytes but value is null"),
        }
    }

    fn pop_next_null_bit(&mut self) -> bool {
        if let Some(null_bitmap) = self.null_bitmap.as_mut() {
            if let Some(null_bit) = null_bitmap.next() {
                return null_bit;
            }
        }

        false
    }

    pub(crate) fn parse_bytes_opt(
        mut self,
        len: usize,
    ) -> Result<(Option<&'a [u8]>, Record<'a>), &'static str> {
        if len > self.fixed_bytes.len() {
            return Err("requested fixed-length bytes exceed record bounds");
        }
        let is_null = self.pop_next_null_bit();
        let (bytes, remaining_bytes) = &self.fixed_bytes.split_at(len);

        let record = Self {
            fixed_bytes: remaining_bytes,
            r#type: self.r#type,
            null_bitmap: self.null_bitmap,
            variable_columns: self.variable_columns,
        };

        Ok((if is_null { None } else { Some(bytes) }, record))
    }

    const EMPTY_SLICE: &'static [u8] = &[];

    pub(crate) fn parse_variables_bytes_opt(
        mut self,
    ) -> Result<(Option<&'a [u8]>, Record<'a>), &'static str> {
        let is_null = self.pop_next_null_bit();

        let mut variable_columns = match self.variable_columns {
            Some(columns) => columns,
            None => {
                return Err("no variable column data");
            }
        };

        if is_null {
            let _ = variable_columns.next_bytes()?;
            let record = Self {
                fixed_bytes: self.fixed_bytes,
                r#type: self.r#type,
                null_bitmap: self.null_bitmap,
                variable_columns: Some(variable_columns),
            };
            return Ok((None, record));
        }

        let bytes = variable_columns
            .next_bytes()?
            // If the current variable length column index exceeds the number of stored
            // variable length columns, the value is empty by definition (that is, 0 bytes, but not null).
            .unwrap_or(Self::EMPTY_SLICE);

        let record = Self {
            fixed_bytes: self.fixed_bytes,
            r#type: self.r#type,
            null_bitmap: self.null_bitmap,
            variable_columns: Some(variable_columns),
        };

        Ok((Some(bytes), record))
    }

    pub(crate) fn parse_string_from_fixed_bytes(
        self,
        len: usize,
    ) -> Result<(String, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_bytes(len)?;

        let (s, _, _) = encoding_rs::UTF_8.decode(bytes);
        let s = s.into_owned();

        Ok((s, record))
    }

    pub(crate) fn parse_binary(self) -> Result<(Option<Vec<u8>>, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_variables_bytes_opt()?;

        let b = match bytes {
            Some(x) => Some(x.to_vec()),
            None => None,
        };

        Ok((b, record))
    }

    pub(crate) fn parse_string(self) -> Result<(Option<String>, Record<'a>), &'static str> {
        let (bytes, record) = self.parse_variables_bytes_opt()?;

        let s = match bytes {
            Some(first) => {
                if first.is_empty() {
                    // TODO: this is an open question: is it correct to assume that an
                    // empty array is an null string? Some SQL Server do so but is that
                    // true for MSSQL and therefore, is this true for MDF files?
                    // One of the integration tests demands this assumption.
                    None
                } else {
                    let (s, _, _) = encoding_rs::UTF_16LE.decode(first);
                    Some(s.into_owned())
                }
            }
            None => None,
        };

        Ok((s, record))
    }

    pub(crate) fn parse_uuid(self) -> Result<(Uuid, Self), &'static str> {
        let (bytes, record) = self.parse_u128()?;

        let uuid = Uuid::from_u128_le(bytes);

        Ok((uuid, record))
    }
}

#[derive(Debug)]
struct NullBitmap<'a> {
    index: usize,
    null_bitmap: &'a BitSlice<u8, Lsb0>,
}

impl<'a> NullBitmap<'a> {
    fn new(null_bitmap: &'a [u8]) -> Self {
        Self {
            index: 0,
            null_bitmap: BitSlice::from_slice(null_bitmap),
        }
    }
}

impl<'a> Iterator for NullBitmap<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.null_bitmap.len() {
            return None;
        }

        let index = self.index;
        self.index += 1;
        if self.null_bitmap[index] {
            Some(true)
        } else {
            Some(false)
        }
    }
}

#[derive(Debug)]
struct VariableColumns<'a> {
    variable_columns: &'a [u8],
    variable_length_column_lengths: &'a [u8],
    read_bytes_index: Option<usize>,
}

impl<'a> VariableColumns<'a> {
    fn try_new(mut read_bytes: usize, bytes: &'a [u8]) -> Result<Self, &'static str> {
        let (bytes, number_of_variable_length_columns) = parse_le_u16(
            bytes,
            "record too short for variable-length column count",
        )?;
        read_bytes += 2;

        /* TODO: from the original coder
        // If there is no fixed length data and no null bitmap, only the number of variable length columns is stored.
        if (FixedLengthData.Length == 0 && !HasNullBitmap)
            NumberOfVariableLengthColumns = NumberOfColumns;
        else
        {
            NumberOfVariableLengthColumns = BitConverter.ToInt16(bytes, offset);
            offset += 2;
        }
        */

        let (variable_columns, variable_length_column_lengths) = take_bytes(
            bytes,
            number_of_variable_length_columns as usize * 2,
            "record too short for variable-length column lengths",
        )?;

        Ok(Self {
            variable_columns,
            variable_length_column_lengths,
            read_bytes_index: Some(read_bytes + variable_length_column_lengths.len()),
        })
    }
    fn next_bytes(&mut self) -> Result<Option<&'a [u8]>, &'static str> {
        let read_bytes_index = match self.read_bytes_index.take() {
            Some(read_bytes_index) => read_bytes_index,
            None => return Ok(None),
        };

        if self.variable_length_column_lengths.is_empty() {
            return Ok(None);
        }

        let (variable_length_column_lengths, end_idx) = parse_le_i16(
            self.variable_length_column_lengths,
            "variable column length entry truncated",
        )?;
        self.variable_length_column_lengths = variable_length_column_lengths;

        let (complex, end_index_of_readable_bytes) = if end_idx < 0 {
            (true, -end_idx as usize)
        } else {
            (false, end_idx as usize)
        };

        if complex {
            return Ok(None);
        }

        if end_index_of_readable_bytes < read_bytes_index {
            return Err("variable column end offset precedes current read position");
        }

        self.read_bytes_index = Some(end_index_of_readable_bytes);

        let length = end_index_of_readable_bytes - read_bytes_index;
        let (bytes, remaining_bytes) = self
            .variable_columns
            .split_at(std::cmp::min(length, self.variable_columns.len()));

        self.variable_columns = remaining_bytes;

        Ok(Some(bytes))
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PagePointer {
    pub(crate) page_id: u32,
    pub(crate) file_id: u16,
}

impl PagePointer {
    pub(crate) fn with_page_id(&self, page_id: u32) -> Self {
        Self {
            page_id,
            file_id: self.file_id,
        }
    }
}

impl TryFrom<&[u8]> for PagePointer {
    type Error = &'static str;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 6 {
            return Err("Page pointer must be 6 bytes.");
        }

        let (bytes, page_id) = parse_le_u32(bytes, "Page pointer must be 6 bytes.")?;
        let (bytes, file_id) = parse_le_u16(bytes, "Page pointer must be 6 bytes.")?;
        if !bytes.is_empty() {
            return Err("Page pointer must be 6 bytes.");
        }

        Ok(Self {
            page_id,
            file_id,
        })
    }
}

/// Converts the bytes into an `BootPage`.
///
/// ```text
/// Bytes       Content
/// -----       -------
/// ...         ?
/// 148-404     DatabaseName (nchar(128))
/// 612-615     FirstSysIndexes PageID (int)
/// 616-617     FirstSysIndexes FileID (smallint)
/// ...         ?
/// ```
impl TryFrom<[u8; 8192]> for BootPage {
    type Error = &'static str;

    fn try_from(bytes: [u8; 8192]) -> Result<Self, Self::Error> {
        let header = PageHeader::try_from(&bytes[0..96])?;

        let (s, _, _) = encoding_rs::UTF_16LE.decode(&bytes[148..(404)]);
        let database_name = String::from_iter(s.chars().filter(|c| *c != '†'));

        let first_sys_indexes = PagePointer::try_from(&bytes[612..618])?;

        Ok(Self {
            header,
            database_name,
            first_sys_indexes,
        })
    }
}

/// Converts the given bytes into a `PageHeader`.
///
/// ```text
/// Bytes       Content
/// -----       -------
/// ...         ?
//  16-19       NextPageID (int)
/// 20-21       NextPageFileID (smallint)
/// 22-23       SlotCnt (smallint)
/// ...         ?
/// ```
impl TryFrom<&[u8]> for PageHeader {
    type Error = &'static str;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 96 {
            return Err("Page header must be 96 bytes.");
        }

        let (bytes, _) = take_bytes(bytes, 16, "Page header must be 96 bytes.")?;
        let (bytes, next_page_bytes) = take_bytes(bytes, 6, "Page header must be 96 bytes.")?;
        let next_page_pointer = PagePointer::try_from(next_page_bytes)?;
        let next_page_pointer = if next_page_pointer.page_id > 0 {
            Some(next_page_pointer)
        } else {
            None
        };
        let (_, slot_count) = parse_le_u16(bytes, "Page header must be 96 bytes.")?;

        Ok(PageHeader {
            slot_count,
            next_page_pointer,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Page {
    header: PageHeader,
    bytes: [u8; 8192],
}

impl Page {
    pub(crate) fn header(&self) -> &PageHeader {
        &self.header
    }

    fn slots(&self) -> Vec<usize> {
        let slot_count = self.header.slot_count as usize;
        let mut slots = Vec::with_capacity(slot_count);

        let slot_bytes_len = match slot_count.checked_mul(2) {
            Some(slot_bytes_len) => slot_bytes_len,
            None => {
                log::error!("Skipping malformed slot directory: slot count {} overflows", slot_count);
                return slots;
            }
        };
        let slot_range_start = match self.bytes.len().checked_sub(slot_bytes_len) {
            Some(slot_range_start) => slot_range_start,
            None => {
                log::error!(
                    "Skipping malformed slot directory: {} slots exceed page size {}",
                    slot_count,
                    self.bytes.len()
                );
                return slots;
            }
        };
        let mut slot_bytes = &self.bytes[slot_range_start..];

        while !slot_bytes.is_empty() {
            let (remaining_bytes, slot_value) =
                match parse_le_u16(slot_bytes, "page slot directory entry truncated") {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        log::error!("Skipping malformed slot directory: {}", err);
                        return Vec::new();
                    }
                };
            slots.push(slot_value as usize);
            slot_bytes = remaining_bytes;
        }

        slots.sort_unstable();

        slots
    }

    pub(crate) fn records<'a, 'b: 'a>(&'b self) -> Vec<Record<'a>> {
        let mut records = Vec::with_capacity(self.header.slot_count as usize);

        let slots = self.slots();
        for (index, slot) in slots.iter().enumerate() {
            let range = match slots.get(index + 1) {
                Some(next_slot) => *slot..*next_slot,
                None => *slot..self.bytes.len(),
            };

            if range.start >= self.bytes.len() || range.start >= range.end || range.end > self.bytes.len() {
                log::error!(
                    "Skipping malformed record slot range {}..{} (page size {})",
                    range.start,
                    range.end,
                    self.bytes.len()
                );
                continue;
            }

            match Record::try_from(&self.bytes[range.clone()]) {
                Ok(record) => records.push(record),
                Err(err) => {
                    log::error!(
                        "Skipping malformed record slot {}..{}: {}",
                        range.start,
                        range.end,
                        err
                    );
                }
            }
        }
        records
    }

    pub(crate) fn next_page_pointer(&self) -> Option<&PagePointer> {
        self.header.next_page_pointer.as_ref()
    }
}

impl TryFrom<[u8; 8192]> for Page {
    type Error = &'static str;

    fn try_from(bytes: [u8; 8192]) -> Result<Self, Self::Error> {
        let header = PageHeader::try_from(&bytes[0..96])?;

        Ok(Self { header, bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest(
        bytes,
        expected_value,
        case(vec![0u8, 0u8, 5u8, 0u8, 1u8, 0u8, 0u8], 1i8),
        case(vec![0u8, 0u8, 5u8, 0u8, 255u8, 0u8, 0u8], -1i8)
    )]
    fn parse_i8(bytes: Vec<u8>, expected_value: i8) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_i8().unwrap();

        assert_eq!(expected_value, parsed_value);
    }

    #[rstest(
        bytes,
        expected_value,
        case(vec![0u8, 0u8, 6u8, 0u8, 1u8, 0u8, 0u8, 0u8], 1i16),
        case(vec![0u8, 0u8, 6u8, 0u8, 255u8, 255u8, 0u8, 0u8], -1i16)
    )]
    fn parse_i16(bytes: Vec<u8>, expected_value: i16) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_i16().unwrap();

        assert_eq!(expected_value, parsed_value);
    }

    #[rstest(
        bytes,
        expected_value,
        case(vec![0u8, 0u8, 8u8, 0u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8], 1i32),
        case(vec![0u8, 0u8, 8u8, 0u8, 255u8, 255u8, 255u8, 255u8, 0u8, 0u8, 0u8, 0u8], -1i32)
    )]
    fn parse_i32(bytes: Vec<u8>, expected_value: i32) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_i32().unwrap();

        assert_eq!(expected_value, parsed_value);
    }

    #[rstest(
        bytes,
        expected_value,
        case(vec![0u8, 0u8, 12u8, 0u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8], 1i64),
        case(vec![0u8, 0u8, 12u8, 0u8, 255u8, 255u8, 255u8, 255u8, 255u8, 255u8, 255u8, 255u8, 0u8, 0u8], -1i64)
    )]
    fn parse_i64(bytes: Vec<u8>, expected_value: i64) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_i64().unwrap();

        assert_eq!(expected_value, parsed_value);
    }

    #[rstest(
        bytes,
        precision,
        scale,
        expected_value,
        case(vec![0u8, 0u8, 9u8, 0u8, 0x01, 0x39, 0x30, 0u8, 0u8, 0u8, 0u8], 5u8, 0u8, Decimal::new(12345, 0)),
        case(vec![0u8, 0u8, 9u8, 0u8, 0x01, 0x39, 0x30, 0u8, 0u8, 0u8, 0u8], 5u8, 3u8, Decimal::new(12345, 3)),
        case(vec![0u8, 0u8, 9u8, 0u8, 0x00, 0x39, 0x30, 0u8, 0u8, 0u8, 0u8], 5u8, 3u8, Decimal::new(-12345, 3)),
        case(vec![0u8, 0u8, 9u8, 0u8, 0x01, 0x4e, 0xe4, 0x01, 0x00, 0u8, 0u8], 9u8, 1u8, Decimal::new(123982, 1)),
        case(vec![0u8, 0u8, 13u8, 0u8, 0x01, 0xb9, 0xe3, 0x5d, 0xb6, 0x40, 0x70, 0x00, 0x00, 0u8, 0u8], 17u8, 5u8, Decimal::new(123423239824313, 5)),
        case(
            vec![0u8, 0u8, 17u8, 0u8, 0x01, 121u8, 223u8, 226u8, 61u8, 68u8, 166u8, 54u8, 15u8, 110u8, 5u8, 1u8, 0u8, 0u8, 0u8],
            25u8,
            4u8,
            Decimal::from_i128_with_scale(1234567890123456789012345i128, 4)
        )
    )]
    fn parse_decimal(bytes: Vec<u8>, precision: u8, scale: u8, expected_value: Decimal) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_decimal_opt(precision, scale).unwrap();

        assert_eq!(Some(expected_value), parsed_value);
    }

    #[test]
    fn record_try_from_returns_err_for_truncated_header() {
        let err = Record::try_from(&[0u8][..]).expect_err("truncated record header should return Err");
        assert_eq!("record too short for header", err);
    }

    #[test]
    fn record_try_from_returns_err_for_truncated_variable_column_metadata() {
        let err = Record::try_from(&[0b0010_0000, 0x00, 0x04, 0x00, 0x01, 0x00, 0x01, 0x00][..])
            .expect_err("truncated variable-column metadata should return Err");
        assert_eq!("record too short for variable-length column lengths", err);
    }

    #[test]
    fn page_records_skips_malformed_slots_instead_of_panicking() {
        let mut bytes = [0u8; 8192];
        bytes[22..24].copy_from_slice(&1u16.to_le_bytes());
        bytes[8190..8192].copy_from_slice(&96u16.to_le_bytes());

        let page = Page::try_from(bytes).expect("synthetic page header should be valid");
        assert!(page.records().is_empty(), "malformed slot should be skipped");
    }

    #[test]
    fn page_records_skips_out_of_bounds_slots_instead_of_panicking() {
        let mut bytes = [0u8; 8192];
        bytes[22..24].copy_from_slice(&1u16.to_le_bytes());
        bytes[8190..8192].copy_from_slice(&9000u16.to_le_bytes());

        let page = Page::try_from(bytes).expect("synthetic page header should be valid");
        assert!(page.records().is_empty(), "out-of-bounds slot should be skipped");
    }

    #[test]
    fn page_records_skips_impossible_slot_directory_size_instead_of_panicking() {
        let mut bytes = [0u8; 8192];
        bytes[22..24].copy_from_slice(&5000u16.to_le_bytes());

        let page = Page::try_from(bytes).expect("synthetic page header should be valid");
        assert!(
            page.records().is_empty(),
            "slot directory larger than the page should be skipped"
        );
    }

    #[test]
    fn parse_i32_returns_err_for_truncated_fixed_region() {
        let bytes = vec![0u8, 0u8, 5u8, 0u8, 0xAA, 0u8, 0u8];
        let record = Record::try_from(&bytes[..]).unwrap();

        let err = record
            .parse_i32()
            .expect_err("should fail when fixed region has fewer than 4 bytes");
        assert_eq!("requested fixed-length bytes exceed record bounds", err);
    }

    #[rstest(
        bytes,
        expected_value,
        // Bytes copied from data/AWLT2005.mdf
        case(vec![0x30, 0x0, 0x2c, 0x0, 0x4, 0x0, 0x0, 0x0, 0x4, 0x0, 0x0, 0x0, 0x0, 0x1, 0x0, 0x0e, 0x0, 0x53, 0x20, 0x0, 0x0, 0x0, 0x0, 0x1, 0x6, 0x0, 0x0, 0x0, 0x15, 0xf6, 0xc2, 0x0, 0x4a, 0x98, 0x0, 0x0, 0x15, 0xf6, 0xc2, 0x0, 0x4a, 0x98, 0x0, 0x0, 0xb, 0x0, 0x0, 0xf8, 0x1, 0x0, 0x54, 0x0, 0x73, 0x0, 0x79, 0x0, 0x73, 0x0, 0x72, 0x0, 0x6f, 0x0, 0x77, 0x0, 0x73, 0x0, 0x65, 0x0, 0x74, 0x0, 0x63, 0x0, 0x6f, 0x0, 0x6c, 0x0, 0x75, 0x0, 0x6d, 0x0, 0x6e, 0x0, 0x73, 0x0], Some(String::from("sysrowsetcolumns"))),
        // Bytes copied from data/spg_verein_TST.mdf
        case(vec![48, 0, 48, 0, 233, 135, 194, 92, 1, 0, 0, 0, 0, 0, 0, 14, 0, 85, 32, 0, 0, 0, 0, 1, 108, 0, 0, 0, 112, 200, 220, 0, 230, 167, 0, 0, 177, 76, 220, 0, 160, 171, 0, 0, 0, 0, 0, 0, 12, 0, 0, 0, 1, 0, 80, 0, 116, 0, 98, 0, 108, 0, 95, 0, 77, 0, 105, 0, 116, 0, 103, 0, 108, 0, 105, 0, 101, 0, 100, 0, 108, 0, 105, 0, 101, 0, 100, 0], Some(String::from("tbl_Mitglied"))),
        case(vec![0b0010_0000, 0u8, 5u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8], None),
    )]
    fn parse_string(bytes: Vec<u8>, expected_value: Option<String>) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_string().unwrap();

        assert_eq!(expected_value, parsed_value);
    }

    #[test]
    fn parse_string_allows_zero_fixed_length_region_with_null_bitmap() {
        // SQL Server rows may carry only variable-length payload
        // after the 4-byte record header. This shape appears in
        // SmartPlant lookup tables such as T_Area / T_Plant.
        let bytes = vec![
            0b0011_0000, // primary record + null bitmap + variable columns
            0x00,
            0x04,
            0x00, // fixed-length size is header-only, so fixed region is empty
            0x01,
            0x00, // one column
            0x00, // column is not null
            0x01,
            0x00, // one variable-length column
            0x0f,
            0x00, // end offset from record start
            b'H',
            0x00,
            b'i',
            0x00,
        ];
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_string().unwrap();

        assert_eq!(Some(String::from("Hi")), parsed_value);
    }

    #[test]
    fn parse_string_returns_err_for_descending_variable_column_end_offset() {
        let bytes = vec![
            0b0010_0000, // primary record + variable columns
            0x00,
            0x04,
            0x00, // fixed-length size is header-only, so fixed region is empty
            0x00,
            0x00, // column count is irrelevant here because there is no null bitmap
            0x01,
            0x00, // one variable-length column
            0x00,
            0x00, // malformed end offset: smaller than the current read index
        ];
        let record = Record::try_from(&bytes[..]).unwrap();

        let err = record
            .parse_string()
            .expect_err("descending variable column end offset should return Err");

        assert_eq!("variable column end offset precedes current read position", err);
    }

    #[test]
    fn parse_string_with_length() {
        // Bytes copied from data/spg_verein_TST.mdf
        let bytes = vec![
            48, 0, 211, 0, 32, 0, 32, 0, 32, 0, 0, 0, 0, 0, 0, 74, 8, 11, 0, 0, 0, 0, 0, 114, 39,
            11, 8, 0, 0, 0, 0, 0, 136, 97, 240, 116, 2, 0, 0, 0, 208, 97, 240, 0, 0, 0, 0, 0, 229,
            28, 11, 116, 2, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 208, 7, 0, 0, 231,
            116, 2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 220, 3, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0,
            0, 132, 28, 0, 0, 1, 80, 45, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 57, 11, 0, 0, 209,
            177, 172, 13, 0, 0, 0, 116, 215, 136, 178, 53, 58, 11, 1, 0, 0, 0, 0, 116, 215, 136,
            178, 115, 61, 11, 32, 0, 32, 0, 32, 0, 192, 198, 132, 117, 2, 0, 0, 0, 32, 0, 32, 0,
            32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 116, 215, 136, 178, 53, 58, 11, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 232, 3, 0, 0, 0, 0, 0, 0, 102, 0, 0, 0, 0, 10, 16, 12, 8, 0, 0, 0, 0,
            8, 26, 57, 0, 106, 1, 116, 1, 116, 1, 126, 1, 142, 1, 142, 1, 166, 1, 176, 1, 200, 1,
            200, 1, 200, 1, 212, 1, 212, 1, 212, 1, 214, 1, 220, 1, 242, 1, 16, 2, 16, 2, 16, 2,
            50, 2, 52, 2, 54, 2, 54, 2, 54, 2, 54, 2, 54, 2, 54, 2, 86, 2, 110, 2, 132, 2, 154, 2,
            198, 2, 230, 2, 230, 2, 254, 2, 254, 2, 254, 2, 254, 2, 254, 2, 38, 3, 38, 3, 38, 3,
            48, 3, 54, 3, 66, 3, 82, 3, 82, 3, 106, 3, 116, 3, 126, 3, 168, 3, 168, 3, 186, 3, 206,
            3, 210, 3, 220, 3, 48, 0, 48, 0, 48, 0, 48, 0, 48, 0, 48, 0, 49, 0, 48, 0, 48, 0, 48,
            0, 72, 0, 101, 0, 114, 0, 114, 0, 110, 0, 70, 0, 114, 0, 97, 0, 110, 0, 107, 0, 66, 0,
            101, 0, 114, 0, 103, 0, 109, 0, 97, 0, 110, 0, 110, 0, 82, 0, 101, 0, 98, 0, 101, 0,
            110, 0, 114, 0, 105, 0, 110, 0, 103, 0, 32, 0, 53, 0, 54, 0, 51, 0, 56, 0, 49, 0, 48,
            0, 56, 0, 66, 0, 114, 0, 97, 0, 117, 0, 110, 0, 115, 0, 99, 0, 104, 0, 119, 0, 101, 0,
            105, 0, 103, 0, 49, 0, 49, 0, 50, 0, 50, 0, 51, 0, 51, 0, 109, 0, 49, 0, 53, 0, 48, 0,
            48, 0, 53, 0, 51, 0, 49, 0, 47, 0, 52, 0, 50, 0, 51, 0, 51, 0, 52, 0, 52, 0, 48, 0, 53,
            0, 51, 0, 49, 0, 47, 0, 50, 0, 50, 0, 55, 0, 55, 0, 56, 0, 56, 0, 57, 0, 57, 0, 49, 0,
            49, 0, 101, 0, 114, 0, 32, 0, 72, 0, 101, 0, 114, 0, 114, 0, 32, 0, 66, 0, 101, 0, 114,
            0, 103, 0, 109, 0, 97, 0, 110, 0, 110, 0, 44, 0, 48, 0, 114, 0, 48, 0, 48, 0, 49, 0,
            32, 0, 220, 0, 98, 0, 117, 0, 110, 0, 103, 0, 115, 0, 108, 0, 101, 0, 105, 0, 116, 0,
            101, 0, 114, 0, 48, 0, 48, 0, 50, 0, 32, 0, 76, 0, 105, 0, 122, 0, 101, 0, 110, 0, 122,
            0, 32, 0, 65, 0, 48, 0, 49, 0, 55, 0, 50, 0, 47, 0, 49, 0, 49, 0, 50, 0, 50, 0, 51, 0,
            51, 0, 48, 0, 49, 0, 55, 0, 50, 0, 47, 0, 52, 0, 52, 0, 53, 0, 53, 0, 54, 0, 54, 0,
            102, 0, 114, 0, 97, 0, 110, 0, 107, 0, 46, 0, 98, 0, 101, 0, 114, 0, 103, 0, 109, 0,
            97, 0, 110, 0, 110, 0, 64, 0, 116, 0, 101, 0, 115, 0, 116, 0, 46, 0, 100, 0, 101, 0,
            119, 0, 119, 0, 119, 0, 46, 0, 115, 0, 112, 0, 103, 0, 45, 0, 112, 0, 101, 0, 105, 0,
            110, 0, 101, 0, 46, 0, 100, 0, 101, 0, 102, 0, 117, 0, 115, 0, 115, 0, 98, 0, 97, 0,
            108, 0, 108, 0, 46, 0, 106, 0, 112, 0, 103, 0, 66, 0, 69, 0, 82, 0, 71, 0, 77, 0, 65,
            0, 78, 0, 78, 0, 32, 0, 32, 0, 32, 0, 32, 0, 32, 0, 32, 0, 32, 0, 70, 0, 82, 0, 65, 0,
            78, 0, 75, 0, 72, 0, 101, 0, 114, 0, 114, 0, 110, 0, 68, 0, 114, 0, 46, 0, 72, 0, 117,
            0, 98, 0, 101, 0, 114, 0, 116, 0, 66, 0, 101, 0, 114, 0, 103, 0, 109, 0, 97, 0, 110, 0,
            110, 0, 77, 0, 101, 0, 105, 0, 115, 0, 101, 0, 110, 0, 119, 0, 101, 0, 103, 0, 32, 0,
            49, 0, 53, 0, 51, 0, 49, 0, 50, 0, 50, 0, 56, 0, 80, 0, 101, 0, 105, 0, 110, 0, 101, 0,
            101, 0, 114, 0, 32, 0, 72, 0, 101, 0, 114, 0, 114, 0, 32, 0, 68, 0, 114, 0, 46, 0, 32,
            0, 66, 0, 101, 0, 114, 0, 103, 0, 109, 0, 97, 0, 110, 0, 110, 0, 44, 0, 83, 0, 101, 0,
            103, 0, 101, 0, 108, 0, 98, 0, 111, 0, 111, 0, 116, 0, 49, 0, 53, 0, 46, 0, 48, 0, 51,
            0, 46, 0, 50, 0, 48, 0, 48, 0, 53, 0, 49, 0, 48, 0, 50, 0, 56, 0, 53, 0, 48, 0, 48, 0,
        ];
        let record = Record::try_from(&bytes[..]).unwrap();

        let (id, record) = record.parse_string().unwrap();
        assert_eq!(Some(String::from("0000001000")), id);

        let (id, _record) = record.parse_string().unwrap();
        assert_eq!(Some(String::from("Herrn")), id);
    }

    #[rstest(
        bytes,
        expected_value,
        case(vec![0u8, 0u8, 12u8, 0u8, 0, 0, 0, 0, 249, 148, 0, 0, 0u8, 0u8], Some(Utc.with_ymd_and_hms(2004, 6, 1, 0, 0, 0).unwrap()))
    )]
    fn parse_datetime(bytes: Vec<u8>, expected_value: Option<DateTime<Utc>>) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_datetime_opt().unwrap();

        assert_eq!(expected_value, parsed_value);
    }

    #[rstest(
        bytes,
        expected_value,
        case(vec![0u8, 0u8, 20u8, 0u8, 215, 208, 221, 236, 178, 45, 77, 70, 178, 218, 137, 191, 252, 98, 118, 170, 0u8, 0u8], Uuid::from_u128_le(226583458013659211989771997646895829207u128))
    )]
    fn parse_uuid(bytes: Vec<u8>, expected_value: Uuid) {
        let record = Record::try_from(&bytes[..]).unwrap();

        let (parsed_value, _record) = record.parse_uuid().unwrap();

        assert_eq!(expected_value, parsed_value);
    }

    #[rstest(
        bytes,
        expected_value,
        case(vec![0b0010_0000, 0, 4, 0, 1, 0, 1, 0, 38, 0, 1, 5, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0, 148, 146, 34, 80, 208, 187, 100, 97, 111, 197, 84, 61, 232, 3, 0, 0], vec![1, 5, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0, 148, 146, 34, 80, 208, 187, 100, 97, 111, 197, 84, 61, 232, 3, 0, 0])
    )]
    fn parse_varbinary(bytes: Vec<u8>, expected_value: Vec<u8>) {
        let record = Record::try_from(&bytes[..]).unwrap();
        let (parsed_value, _record) = record.parse_binary().unwrap();

        assert_eq!(expected_value, parsed_value.unwrap());
    }
}
