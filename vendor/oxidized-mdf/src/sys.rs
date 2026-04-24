// SPDX-License-Identifier: GPL-3.0-or-later
// Original: oxidized-mdf by schrieveslaach (https://gitlab.com/schrieveslaach/oxidized-mdf)
// Modified: 2026-04-24 by happyrust
//   - Replaced panic!() / unwrap() / todo!() with ok_or + ? for panic-free TryFrom
//   - Added sysname, char, binary, numeric, smalldatetime, smallmoney, image,
//     text, ntext, date, timestamp column type variants

use crate::error::Error;
use crate::pages::{BootPage, PagePointer, Record};
use crate::PageReader;

pub(crate) struct BaseTableData {
    sysalloc_units: Vec<SysallocUnit>,
    sysrow_sets: Vec<SysrowSet>,
    sysschobjs: Vec<Sysschobj>,
    sysscalartypes: Vec<Sysscalartype>,
    syscolpars: Vec<Syscolpar>,
}

const SYSROWEST_AUID: i64 = 327680;
const SYSSCHOBJS_IDMAJOR: i32 = 34;
const SYSCOLPARS_IDMAJOR: i32 = 41;
const SYSSCALARTYPE_IDMAJOR: i32 = 50;

macro_rules! parse_page_records {
    ( $page_reader:expr, $page_pointer:expr, $t:ty ) => {{
        let mut parsed_records = Vec::new();

        let mut page_iter = $page_reader.read_pages_of_pointer($page_pointer);
        while let Some(Ok(page)) = page_iter.next() {
            parsed_records.extend(
                page.records()
                    .into_iter()
                    .map(<$t>::try_from)
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>(),
            );
        }

        parsed_records
    }};
}

macro_rules! parse_from_sysrow_set {
    ( $page_reader:expr, $sysrow_sets:expr, $sysalloc_units:expr, $t:ty ) => {{
        let rowset_id = $sysrow_sets
            .map(|row| row.rowsetid)
            .ok_or(Error::ParseError("sysrowset entry not found"))?;

        let page_pointer = $sysalloc_units
            .iter()
            .find(|unit| unit.auid == rowset_id && unit.r#type == 1)
            .and_then(|unit| PagePointer::try_from(&unit.pgfirst[..]).ok())
            .ok_or(Error::ParseError("sysallocunit page pointer not found"))?;

        let mut parsed_records = Vec::new();

        let mut page_iter = $page_reader.read_pages_of_pointer(page_pointer);
        while let Some(Ok(page)) = page_iter.next() {
            parsed_records.extend(
                page.records()
                    .into_iter()
                    .map(<$t>::try_from)
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>(),
            );
        }

        parsed_records
    }};
}

impl BaseTableData {
    pub(crate) fn parse(
        mut page_reader: &mut PageReader,
        boot_page: &BootPage,
    ) -> Result<Self, Error> {
        let sysalloc_units = parse_page_records!(
            &mut page_reader,
            boot_page.first_sys_indexes.clone(),
            SysallocUnit
        );

        let sysrowset_page_pointer = sysalloc_units
            .iter()
            .find(|unit| unit.auid == SYSROWEST_AUID)
            .and_then(|unit| PagePointer::try_from(&unit.pgfirst[..]).ok())
            .ok_or(Error::ParseError("sysrowset page pointer not found"))?;

        let sysrow_sets = parse_page_records!(&mut page_reader, sysrowset_page_pointer, SysrowSet);

        let sysschobjs = parse_from_sysrow_set!(
            &mut page_reader,
            &sysrow_sets
                .iter()
                .find(|row| row.idmajor == SYSSCHOBJS_IDMAJOR && row.idminor == 1),
            &sysalloc_units,
            Sysschobj
        );

        let sysscalartypes = parse_from_sysrow_set!(
            &mut page_reader,
            &sysrow_sets
                .iter()
                .find(|row| row.idmajor == SYSSCALARTYPE_IDMAJOR && row.idminor == 1),
            &sysalloc_units,
            Sysscalartype
        );

        let syscolpars = parse_from_sysrow_set!(
            &mut page_reader,
            &sysrow_sets
                .iter()
                .find(|row| row.idmajor == SYSCOLPARS_IDMAJOR && row.idminor == 1),
            &sysalloc_units,
            Syscolpar
        );

        Ok(Self {
            sysalloc_units,
            sysrow_sets,
            sysschobjs,
            sysscalartypes,
            syscolpars,
        })
    }

    fn objects_dollar(&self) -> impl Iterator<Item = &Sysschobj> {
        self.sysschobjs
            .iter()
            .filter(|o| o.nsclass == 0 && o.pclass == 1)
    }

    pub(crate) fn tables(&self) -> Vec<String> {
        self.objects_dollar()
            .filter(|o| o.r#type == "U")
            .map(|o| o.name.clone())
            .collect()
    }

    pub(crate) fn table<'a, 'b: 'a>(&'b self, table_name: &str) -> Option<Table<'a>> {
        self.objects_dollar()
            .find(|o| o.name == table_name)
            .map(|table| Table {
                objects_dollar: table,
                sysalloc_units: &self.sysalloc_units,
                sysrow_sets: &self.sysrow_sets,
                columns: {
                    let mut cols = self
                        .syscolpars
                        .iter()
                        .filter(|c| c.number == 0 && c.id == table.id && c.name.is_some())
                        .collect::<Vec<_>>();
                    cols.sort_by_key(|c| c.colid);
                    cols.into_iter().filter_map(|c| {
                        let r#type = self
                            .sysscalartypes
                            .iter()
                            .find(|st| st.xtype == c.xtype)
                            .map(|st| &st.name)?;

                        Some(Column {
                            name: c.name.as_deref().unwrap_or(""),
                            r#type,
                            max_length: c.length,
                            precision: c.prec as u8,
                            scale: c.scale as u8,
                        })
                    }).collect()
                },
            })
    }
}

#[derive(Debug)]
pub(crate) struct Table<'a> {
    objects_dollar: &'a Sysschobj,
    sysalloc_units: &'a Vec<SysallocUnit>,
    sysrow_sets: &'a Vec<SysrowSet>,
    pub(crate) columns: Vec<Column<'a>>,
}

impl<'a> Table<'a> {
    pub(crate) fn page_pointers(&self) -> Vec<PagePointer> {
        let mut partitions = self
            .sysrow_sets
            .iter()
            .filter(|sysrow| sysrow.idmajor == self.objects_dollar.id)
            .collect::<Vec<_>>();

        partitions.sort_by_key(|p| p.numpart);

        let mut page_pointers = Vec::new();

        for partition in partitions {
            if let Some(unit) = self
                .sysalloc_units
                .iter()
                .find(|unit| unit.ownerid == partition.rowsetid && unit.r#type == 1)
            {
                if let Ok(page_pointer) = PagePointer::try_from(&unit.pgfirst[..]) {
                    page_pointers.push(page_pointer);
                }
            }
        }

        page_pointers
    }
}

#[derive(Debug)]
pub(crate) struct Column<'a> {
    pub(crate) name: &'a str,
    pub(crate) r#type: &'a str,
    pub(crate) max_length: i16,
    pub(crate) precision: u8,
    pub(crate) scale: u8,
}

#[derive(Debug)]
struct SysallocUnit {
    auid: i64,
    r#type: i8,
    ownerid: i64,
    status: i32,
    fgid: i16,
    pgfirst: Vec<u8>,
    pgroot: Vec<u8>,
    pgfirstiam: Vec<u8>,
    pcused: i64,
    pcdata: i64,
    pcreserved: i64,
    // TODO dbfragid: i32,
}

impl<'a> TryFrom<Record<'a>> for SysallocUnit {
    type Error = &'static str;

    fn try_from(record: Record<'a>) -> Result<Self, Self::Error> {
        let (auid, record) = record.parse_i64()?;
        let (r#type, record) = record.parse_i8()?;
        let (ownerid, record) = record.parse_i64()?;
        let (status, record) = record.parse_i32()?;
        let (fgid, record) = record.parse_i16()?;

        let (pgfirst, record) = record.parse_bytes(6)?;
        let (pgroot, record) = record.parse_bytes(6)?;
        let (pgfirstiam, record) = record.parse_bytes(6)?;

        let (pcused, record) = record.parse_i64()?;
        let (pcdata, record) = record.parse_i64()?;
        let (pcreserved, _record) = record.parse_i64()?;
        // TODO let (dbfragid, _record) = record.parse_i32()?;

        Ok(Self {
            auid,
            r#type,
            ownerid,
            status,
            fgid,
            pgfirst: pgfirst.to_vec(),
            pgroot: pgroot.to_vec(),
            pgfirstiam: pgfirstiam.to_vec(),
            pcused,
            pcdata,
            pcreserved,
            // TODO dbfragid,
        })
    }
}

#[derive(Debug)]
struct SysrowSet {
    rowsetid: i64,
    ownertype: i8,
    idmajor: i32,
    idminor: i32,
    numpart: i32,
    status: i32,
    fgidfs: i16,
    rcrows: i64,
    // TODO cmprlevel: i8,
    // TODO fillfact: i8,
    // TODO maxnullbit: i16,
    // TODO maxleaf: i32,
    // TODO maxint: i16,
    // TODO minleaf: i16,
    // TODO minint: i16,
    // TODO rsguid: varbinary,
    // TODO lockres: varbinary,
    // TODO dbfragid: i32
}

impl<'a> TryFrom<Record<'a>> for SysrowSet {
    type Error = &'static str;

    fn try_from(record: Record<'a>) -> Result<Self, Self::Error> {
        let (rowsetid, record) = record.parse_i64()?;
        let (ownertype, record) = record.parse_i8()?;
        let (idmajor, record) = record.parse_i32()?;
        let (idminor, record) = record.parse_i32()?;
        let (numpart, record) = record.parse_i32()?;
        let (status, record) = record.parse_i32()?;
        let (fgidfs, record) = record.parse_i16()?;
        let (rcrows, _record) = record.parse_i64()?;
        // TODO let (cmprlevel, record) = record.parse_i8()?;
        // TODO let (fillfact, _record) = record.parse_i8()?;

        // TODO let (maxnullbit, record) = record.parse_i16()?;
        // TODO let (maxleaf, record) = record.parse_i32()?;
        // TODO let (maxint, record) = record.parse_i16()?;
        // TODO let (minleaf, record) = record.parse_i16()?;
        // TODO let (minint, _record) = record.parse_i16()?;

        Ok(Self {
            rowsetid,
            ownertype,
            idmajor,
            idminor,
            numpart,
            status,
            fgidfs,
            rcrows,
            // TODO cmprlevel,
            // TODO fillfact,
        })
    }
}

#[derive(Debug)]
struct Sysschobj {
    id: i32,
    name: String,
    nsid: i32,
    nsclass: i8,
    status: i32,
    r#type: String,
    pid: i32,
    pclass: i8,
    // intprop: i32,
    // TODO created: datetime,
    // TODO modified: datetime,
}

impl<'a> TryFrom<Record<'a>> for Sysschobj {
    type Error = &'static str;

    fn try_from(record: Record<'a>) -> Result<Self, Self::Error> {
        let (id, record) = record.parse_i32()?;
        let (name, record) = record.parse_string()?;
        let (nsid, record) = record.parse_i32()?;
        let (nsclass, record) = record.parse_i8()?;
        let (status, record) = record.parse_i32()?;
        let (r#type, record) = record.parse_string_from_fixed_bytes(2)?;
        let r#type = r#type.trim().to_string();
        let (pid, record) = record.parse_i32()?;
        let (pclass, _record) = record.parse_i8()?;

        Ok(Self {
            id,
            name: name.ok_or("sysschobj name is null")?,
            nsid,
            nsclass,
            status,
            r#type,
            pid,
            pclass,
        })
    }
}

#[derive(Debug)]
struct Sysscalartype {
    id: i32,
    schid: i32,
    name: String,
    xtype: i8,
    length: i16,
    prec: i8,
    scale: i8,
    collationid: i32,
    status: i32,
    // created: datetime
    // modified: datetime
    // dflt: int
    //chk: int
}

impl<'a> TryFrom<Record<'a>> for Sysscalartype {
    type Error = &'static str;

    fn try_from(record: Record<'a>) -> Result<Self, Self::Error> {
        let (id, record) = record.parse_i32()?;
        let (schid, record) = record.parse_i32()?;
        let (name, record) = record.parse_string()?;
        let (xtype, record) = record.parse_i8()?;
        let (length, record) = record.parse_i16()?;
        let (prec, record) = record.parse_i8()?;
        let (scale, record) = record.parse_i8()?;
        let (collationid, record) = record.parse_i32()?;
        let (status, _record) = record.parse_i32()?;

        Ok(Self {
            id,
            schid,
            name: name.ok_or("sysscalartype name is null")?,
            xtype,
            length,
            prec,
            scale,
            collationid,
            status,
        })
    }
}

#[derive(Debug)]
struct Syscolpar {
    id: i32,
    number: i16,
    colid: i32,
    name: Option<String>,
    xtype: i8,
    utype: i32,
    length: i16,
    prec: i8,
    scale: i8,
    collationid: i32,
    status: i32,
    maxinrow: i16,
    xmlns: i32,
    dflt: i32,
    chk: i32,
    // idtval: varbinary
}

impl<'a> TryFrom<Record<'a>> for Syscolpar {
    type Error = &'static str;

    fn try_from(record: Record<'a>) -> Result<Self, Self::Error> {
        let (id, record) = record.parse_i32()?;
        let (number, record) = record.parse_i16()?;
        let (colid, record) = record.parse_i32()?;
        let (name, record) = if record.has_variable_length_columns() {
            let (name, record) = record.parse_string()?;
            (name, record)
        } else {
            (None, record)
        };
        let (xtype, record) = record.parse_i8()?;
        let (utype, record) = record.parse_i32()?;
        let (length, record) = record.parse_i16()?;
        let (prec, record) = record.parse_i8()?;
        let (scale, record) = record.parse_i8()?;
        let (collationid, record) = record.parse_i32()?;
        let (status, record) = record.parse_i32()?;
        let (maxinrow, record) = record.parse_i16()?;
        let (xmlns, record) = record.parse_i32()?;
        let (dflt, record) = record.parse_i32()?;
        let (chk, _record) = record.parse_i32()?;

        Ok(Self {
            id,
            number,
            colid,
            name,
            xtype,
            utype,
            length,
            prec,
            scale,
            collationid,
            status,
            maxinrow,
            xmlns,
            dflt,
            chk,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, MdfDatabase};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_read_boot_page_records() -> Result<(), Error> {
        let db = MdfDatabase::open("data/AWLT2005.mdf")?;
        let auids = db
            .base_table_data
            .sysalloc_units
            .iter()
            .map(|su| su.auid)
            .collect::<Vec<_>>();

        assert_eq!(
            auids,
            vec![
                262144,
                327680,
                458752,
                524288,
                851968,
                983040, // TODO check if the record is also shown by an MDF viewer
                281474978349056,
                281474978414592,
                281474978480128,
                281474978611200,
                281474978938880,
                281474979397632,
                281474979594240,
                281474979987456,
                281474980052992,
                281474980249600,
                281474980315136,
                281474980642816,
                281474980904960,
                281474980970496,
                281474981560320,
                281474981625856,
                281474981691392,
                562949956763648,
                562949958270976,
                562949958336512,
                844424931901440,
                1125899909070848,
                71776119065149440,
                72057594037993472,
                72057594038059008,
                72057594038190080,
                72057594038255616,
                72057594038386688,
                72057594038452224,
                72057594038583296,
                72057594038648832,
                72057594038779904,
                72057594038845440,
                72057594038976512,
                72057594039107584,
                72057594039173120,
                72057594039304192,
                72057594039435264,
                72057594039566336,
                72057594039697408,
                72057594039762944,
                72057594039959552,
                72057594040025088,
                72057594040156160,
                72057594040287232,
                72057594040418304,
                72057594040483840,
                72057594040614912,
                72057594040680448,
                72057594040811520,
                72057594040877056,
                72057594041008128,
                72057594041073664,
                72057594041204736,
                72057594041335808,
                72057594041466880,
                72057594041532416,
                72057594041597952,
                72057594041663488,
                72057594041729024,
                72057594041794560,
                72057594041860096,
                72057594041925632,
                72057594041991168,
                72057594042056704,
                72057594042122240,
                72057594042187776,
                72057594042253312,
                72057594042318848,
                72057594042384384,
                72057594042449920,
                72057594042515456,
                72057594042580992,
                72057594042646528,
                72057594042712064,
                72057594042777600,
                72057594042843136,
                72057594042908672,
                72057594042974208,
                72057594043039744,
                72057594043105280,
                72057594043170816,
                72057594043236352,
                72057594043432960,
                72057594043498496,
                72057594043564032,
                72057594043957248,
                72057594044284928,
                72057594044612608,
                72057594044678144,
                72057594044743680,
                72057594044809216,
                72057594044874752,
                72057594044940288,
                72057594045005824,
                72057594045071360,
                72057594045136896,
                72057594045202432,
                72057594045267968,
                72057594045333504,
                72057594045399040,
                72057594045464576,
                72057594045530112,
                72057594045595648,
                72057594045661184,
                72057594045726720,
                72057594045792256,
                72057594045857792,
                72057594045923328,
                72057594045988864,
                72057594046054400,
                72057594046119936,
                72057594046185472,
                72057594046251008,
                72057594046316544,
                72057594046382080,
                72057594046447616,
                72057594046513152,
                72057594046578688,
                72057594046644224,
                72057594046709760,
                72057594046775296,
                72057594046840832,
                72057594046906368,
                72057594046971904,
                72057594047037440,
            ]
        );
        Ok(())
    }
}
