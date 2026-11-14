//! Values with a unit, written from the SI columns of an Oracle `exp` dump.
//!
//! The dump keeps a unit-bearing attribute twice: as the text it was typed
//! in (`T_PIPERUN.INSULTHICK` `50mm`, `SLOPE` `0.3 %`) and as its SI value
//! (`SP_INSULTHICKSI` 0.05, `SP_SLOPESI` 0.0029999910000486). The reference
//! publishes the SI value in one unit per attribute, whatever the text was
//! typed in: the slope typed in per cent is published in radians.
//! `SPPIDDataMap.xml` ties each of these attributes to the list of units it
//! may take (`SPMU_1001` Thickness, `SPMU_2` Angle, `SPMU_1` Distance,
//! `SPMU_56` Volume) and says nothing of the unit it is published in, and
//! the Thickness list has no metre. The units are the reference's, measured
//! on DWG-0202GP06-01, the one reference on disk that carries any of them:
//!
//! | Column | Interface and attribute | Unit |
//! |---|---|---|
//! | `T_PIPERUN.SP_INSULTHICKSI` | `IInsulatedItem.TotalInsulThick` | `mm` |
//! | `T_PIPERUN.SP_SLOPESI` | `ISlopedPipingItem.SlopedPipingAngle` | `rad` |
//! | `T_EQUIPMENT.SP_INSULTHICKSI` | `IInsulatedItem.TotalInsulThick` | `mm` |
//! | `T_EQUIPMENT.SP_HEIGHTSI` | `IPBSItem.HeightRelativeToGrade` | `m` |
//! | `T_VESSEL.SP_VOLUMERATINGSI` | `IProcessVessel.ProcessVessel_VesselVolumetricCapacity` | `m^3` |
//!
//! A value goes on the published item whose UID is the row's `SP_ID`, as
//! `{number} {unit}` with the number spelled by [`automation_text`], and
//! only when the item's class carries the interface in the `.pid` line's
//! per-class interface table: no reference places `IInsulatedItem` on an
//! equipment class, so an equipment row's insulation is counted as left
//! out rather than written where its element would fall.

use std::collections::HashMap;
use std::fmt::Write;

use super::catalog;
use super::oracle_exp::{backup_uid_key, ExpError, ExpTables, OracleNumber};
use super::pid_load::{PidPublishAttribute, PidPublishDrawing, PidValueSource};

/// The unit a value is published in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishUnit {
    /// `mm`, a thousandth of the SI metre.
    Millimetre,
    /// `m`.
    Metre,
    /// `rad`.
    Radian,
    /// `m^3`.
    CubicMetre,
}

impl PublishUnit {
    /// The unit as the reference spells it after the number.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Millimetre => "mm",
            Self::Metre => "m",
            Self::Radian => "rad",
            Self::CubicMetre => "m^3",
        }
    }

    /// The power of ten an SI value is multiplied by to be in this unit.
    fn places(self) -> i32 {
        match self {
            Self::Millimetre => 3,
            Self::Metre | Self::Radian | Self::CubicMetre => 0,
        }
    }

    /// An SI value in this unit, as the reference writes it: `50 mm` for
    /// 0.05, `2.9999910000486E-03 rad` for 0.0029999910000486. The unit is
    /// changed by moving the decimal point, so no digit is lost on the way.
    #[must_use]
    pub fn spell(self, si: &OracleNumber) -> String {
        let exponent = si.exponent().saturating_add(self.places());
        format!(
            "{} {}",
            decimal_text(si.is_negative(), si.significant_digits(), exponent),
            self.symbol()
        )
    }
}

/// `number` spelled as OLE Automation spells a `Double` as text
/// (`VarBstrFromR8`, which `CStr` calls), worked from its decimal digits.
///
/// Fifteen significant digits, rounded half away from zero, trailing zeros
/// dropped. Positional when the number is below 10^15 and its rounded
/// digits end within fifteen places after the point; otherwise one digit,
/// the rest after a point, `E`, a sign and at least two exponent digits:
/// `50`, `0.05`, `0.029999910000486`, `2.9999910000486E-03`, `1E+15`,
/// `1E-16`, `-2.5`, `0`. The rules are `oleaut32`'s own, read off `CStr`
/// under `cscript`: there `999999999999999.5` reads `1000000000000000` and
/// `1E+15` reads `1E+15`, so the integer side is judged before rounding and
/// the fraction side after it.
///
/// `oleaut32` rounds the binary double and this the decimal digits; the two
/// can differ only on a number of more than fifteen digits whose sixteenth
/// digit on are exactly half a unit of the fifteenth.
#[must_use]
pub fn automation_text(number: &OracleNumber) -> String {
    decimal_text(
        number.is_negative(),
        number.significant_digits(),
        number.exponent(),
    )
}

/// [`automation_text`] of the number whose significant `digits` start at
/// the power of ten `exponent`.
fn decimal_text(negative: bool, digits: &str, exponent: i32) -> String {
    const SIGNIFICANT: usize = 15;
    if digits.bytes().all(|byte| byte == b'0') {
        return "0".to_string();
    }
    let positional_integer = exponent < 15;
    let (digits, exponent) = round_digits(digits, exponent, SIGNIFICANT);
    let fraction = i64::try_from(digits.len()).unwrap_or(i64::MAX) - 1 - i64::from(exponent);
    let mut text = String::with_capacity(digits.len() + 8);
    if negative {
        text.push('-');
    }
    if positional_integer && fraction <= 15 {
        match usize::try_from(exponent) {
            Err(_) => {
                text.push_str("0.");
                text.extend(std::iter::repeat_n(
                    '0',
                    exponent.unsigned_abs() as usize - 1,
                ));
                text.push_str(&digits);
            }
            Ok(whole) if whole + 1 < digits.len() => {
                text.push_str(&digits[..=whole]);
                text.push('.');
                text.push_str(&digits[whole + 1..]);
            }
            Ok(whole) => {
                text.push_str(&digits);
                text.extend(std::iter::repeat_n('0', whole + 1 - digits.len()));
            }
        }
    } else {
        text.push_str(&digits[..1]);
        if digits.len() > 1 {
            text.push('.');
            text.push_str(&digits[1..]);
        }
        let sign = if exponent < 0 { '-' } else { '+' };
        let _ = write!(text, "E{sign}{:02}", exponent.unsigned_abs());
    }
    text
}

/// `digits` × 10^`exponent` rounded half away from zero to at most
/// `significant` digits, trailing zeros dropped, with the power of ten of
/// its first digit after a carry.
fn round_digits(digits: &str, exponent: i32, significant: usize) -> (String, i32) {
    let digits = digits.trim_start_matches('0');
    if digits.len() <= significant {
        return (digits.trim_end_matches('0').to_string(), exponent);
    }
    let mut kept: Vec<u8> = digits.as_bytes()[..significant].to_vec();
    let mut exponent = exponent;
    if digits.as_bytes()[significant] >= b'5' {
        let mut index = kept.len();
        loop {
            if index == 0 {
                kept.insert(0, b'1');
                exponent = exponent.saturating_add(1);
                break;
            }
            index -= 1;
            if kept[index] == b'9' {
                kept[index] = b'0';
            } else {
                kept[index] += 1;
                break;
            }
        }
    }
    let text = String::from_utf8(kept).unwrap_or_default();
    (text.trim_end_matches('0').to_string(), exponent)
}

/// What [`attach_unit_values`] did with the SI values of a drawing's items.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitValueCounts {
    /// Values written on a published item.
    pub written: usize,
    /// Values of a published item whose class carries no element for them.
    pub left_out: usize,
}

/// One SI column and the attribute it is published as.
struct UnitColumn {
    table: &'static str,
    column: &'static str,
    interface: &'static str,
    attribute: &'static str,
    unit: PublishUnit,
}

const RUN_INSULATION: UnitColumn = UnitColumn {
    table: "T_PIPERUN",
    column: "SP_INSULTHICKSI",
    interface: "IInsulatedItem",
    attribute: "TotalInsulThick",
    unit: PublishUnit::Millimetre,
};
const RUN_SLOPE: UnitColumn = UnitColumn {
    table: "T_PIPERUN",
    column: "SP_SLOPESI",
    interface: "ISlopedPipingItem",
    attribute: "SlopedPipingAngle",
    unit: PublishUnit::Radian,
};
const EQUIPMENT_INSULATION: UnitColumn = UnitColumn {
    table: "T_EQUIPMENT",
    column: "SP_INSULTHICKSI",
    interface: "IInsulatedItem",
    attribute: "TotalInsulThick",
    unit: PublishUnit::Millimetre,
};
const EQUIPMENT_HEIGHT: UnitColumn = UnitColumn {
    table: "T_EQUIPMENT",
    column: "SP_HEIGHTSI",
    interface: "IPBSItem",
    attribute: "HeightRelativeToGrade",
    unit: PublishUnit::Metre,
};
const VESSEL_VOLUME: UnitColumn = UnitColumn {
    table: "T_VESSEL",
    column: "SP_VOLUMERATINGSI",
    interface: "IProcessVessel",
    attribute: "ProcessVessel_VesselVolumetricCapacity",
    unit: PublishUnit::CubicMetre,
};

/// The SI cells of the dump, by the folded `SP_ID` of their row.
type UnitCells = HashMap<String, Vec<(&'static UnitColumn, OracleNumber)>>;

/// Write the unit-bearing values the dump states on the drawing's published
/// items: insulation thickness and slope of a pipe run, insulation and
/// height of an equipment, volume of a vessel.
///
/// # Errors
///
/// Returns [`ExpError::Table`] when `T_PIPERUN`, `T_EQUIPMENT` or
/// `T_VESSEL` is not the shape [`ExpTables::pipe_runs`],
/// [`ExpTables::equipment_rows`] and [`ExpTables::vessel_rows`] read.
pub fn attach_unit_values(
    drawing: &mut PidPublishDrawing,
    tables: &ExpTables,
) -> Result<UnitValueCounts, ExpError> {
    let mut cells = UnitCells::new();
    let mut add = |id_key: &str, column: &'static UnitColumn, value: Option<OracleNumber>| {
        if let Some(value) = value {
            cells
                .entry(id_key.to_string())
                .or_default()
                .push((column, value));
        }
    };
    for run in tables.pipe_runs()? {
        add(&run.id_key, &RUN_INSULATION, run.insulation_thickness_si);
        add(&run.id_key, &RUN_SLOPE, run.slope_si);
    }
    for row in tables.equipment_rows()? {
        add(
            &row.id_key,
            &EQUIPMENT_INSULATION,
            row.insulation_thickness_si,
        );
        add(&row.id_key, &EQUIPMENT_HEIGHT, row.height_si);
    }
    for row in tables.vessel_rows()? {
        add(&row.id_key, &VESSEL_VOLUME, row.volume_rating_si);
    }
    Ok(attach_cells(drawing, &cells))
}

fn attach_cells(drawing: &mut PidPublishDrawing, cells: &UnitCells) -> UnitValueCounts {
    let mut counts = UnitValueCounts::default();
    for item in &mut drawing.items {
        let Some(class) = item.class else {
            continue;
        };
        let Some(found) = cells.get(&backup_uid_key(&item.uid)) else {
            continue;
        };
        let interfaces = catalog::class_interfaces(class).unwrap_or_default();
        for (column, si) in found {
            if !interfaces
                .iter()
                .any(|slot| slot.interface == column.interface)
            {
                counts.left_out += 1;
                continue;
            }
            if item.attributes.iter().any(|attribute| {
                attribute.interface == column.interface && attribute.name == column.attribute
            }) {
                continue;
            }
            item.attributes.push(PidPublishAttribute {
                interface: column.interface,
                name: column.attribute,
                value: column.unit.spell(si),
                source: PidValueSource::BackupRow {
                    table: column.table,
                    column: column.column,
                },
            });
            counts.written += 1;
        }
        item.attributes
            .sort_by(|a, b| (a.interface, a.name).cmp(&(b.interface, b.name)));
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::pid_load::{PidPublishItem, PidPublishStats, PipelineSynthesis};
    use crate::publish::pid_writer::write_pid_data_xml;

    /// 0.0029999910000486, the slope of three per mille in radians, as the
    /// committed DWG dump stores it: 29|99|99|10|00|04|86 × 100^-2.
    const SLOPE: [u8; 8] = [0xbf, 0x1e, 0x64, 0x64, 0x0b, 0x01, 0x05, 0x57];

    fn number(raw: &[u8]) -> OracleNumber {
        OracleNumber::from_oracle_bytes(raw).expect("a number")
    }

    #[test]
    fn the_four_reference_spellings() {
        assert_eq!(
            PublishUnit::Millimetre.spell(&number(&[0xc0, 0x06])),
            "50 mm"
        );
        assert_eq!(
            PublishUnit::Radian.spell(&number(&SLOPE)),
            "2.9999910000486E-03 rad"
        );
        assert_eq!(PublishUnit::Metre.spell(&OracleNumber::from(3)), "3 m");
        assert_eq!(
            PublishUnit::CubicMetre.spell(&OracleNumber::from(27)),
            "27 m^3"
        );
    }

    // Each pair is what `CStr` prints for the double under `cscript` on
    // Windows: the digits and the power of ten of its first digit, and the
    // text.
    #[test]
    fn a_number_is_spelled_as_oleaut32_spells_its_double() {
        let cases: &[(&str, i32, &str)] = &[
            ("29999910000486", -3, "2.9999910000486E-03"),
            ("29999910000486", -2, "0.029999910000486"),
            ("29999910000486", -1, "0.29999910000486"),
            ("29999910000486", 0, "2.9999910000486"),
            ("5", -2, "0.05"),
            ("5", 1, "50"),
            ("75", -1, "0.75"),
            ("245", 0, "2.45"),
            ("125", 1, "12.5"),
            ("12345678", 3, "1234.5678"),
            ("1", 5, "100000"),
            ("123", -3, "0.00123"),
            ("123", -5, "0.0000123"),
            ("123456789", -4, "0.000123456789"),
            ("12345678901234", -2, "0.012345678901234"),
            ("12345678901234", -3, "1.2345678901234E-03"),
            ("12345678901234", -4, "1.2345678901234E-04"),
            ("123456789012345", -1, "0.123456789012345"),
            ("123456789012345", -2, "1.23456789012345E-02"),
            ("123456789012345", -3, "1.23456789012345E-03"),
            ("123456789012345", -5, "1.23456789012345E-05"),
            ("999999999999999", -3, "9.99999999999999E-03"),
            ("123456789012345", 14, "123456789012345"),
            ("123456781234567", 7, "12345678.1234567"),
            ("1", -7, "0.0000001"),
            ("1", -10, "0.0000000001"),
            ("1", -15, "0.000000000000001"),
            ("1", -16, "1E-16"),
            ("15", -15, "1.5E-15"),
            ("12", -13, "0.00000000000012"),
            ("5", -6, "0.000005"),
            ("1", 15, "1E+15"),
            ("1", 16, "1E+16"),
            ("15", 20, "1.5E+20"),
            ("1", -100, "1E-100"),
            ("1", 100, "1E+100"),
            // Past fifteen digits: rounded, and the carry can reach a
            // power of ten.
            ("1234567890123456", 15, "1.23456789012346E+15"),
            ("1234567890123456", 14, "123456789012346"),
            ("1234567890123456", -1, "0.123456789012346"),
            ("12345678901234567", -4, "1.23456789012346E-04"),
            ("9999999999999995", 14, "1000000000000000"),
            ("9999999999999999", 14, "1000000000000000"),
            ("9999999999999999", 15, "1E+16"),
            ("9999999999999996", -2, "0.1"),
            ("9999999999999996", -3, "0.01"),
            ("9999999999999996", -4, "0.001"),
            ("99999999999999996", -15, "0.00000000000001"),
            // A carry that leaves trailing zeros drops them.
            ("1234567890123496", 0, "1.2345678901235"),
            ("0", 0, "0"),
        ];
        for &(digits, exponent, text) in cases {
            assert_eq!(
                decimal_text(false, digits, exponent),
                text,
                "{digits} × 10^{exponent}"
            );
        }
        assert_eq!(
            decimal_text(true, "29999910000486", -3),
            "-2.9999910000486E-03"
        );
        assert_eq!(decimal_text(true, "25", 0), "-2.5");
        assert_eq!(decimal_text(true, "5", -2), "-0.05");
        assert_eq!(decimal_text(true, "1", 16), "-1E+16");
    }

    #[test]
    fn a_number_of_the_dump_is_spelled_from_its_own_digits() {
        assert_eq!(automation_text(&number(&SLOPE)), "2.9999910000486E-03");
        assert_eq!(automation_text(&number(&[0xc0, 0x06])), "0.05");
        assert_eq!(automation_text(&number(&[0x3f, 0x33, 0x66])), "-0.5");
        assert_eq!(automation_text(&number(&[0x80])), "0");
        assert_eq!(automation_text(&OracleNumber::from(1200)), "1200");
        assert_eq!(automation_text(&number(&[0xff, 0x02])), "1E+124");
        assert_eq!(automation_text(&number(&[0x81, 0x02])), "1E-128");
    }

    #[test]
    fn a_unit_is_changed_by_moving_the_point() {
        let mm = |raw: &[u8]| PublishUnit::Millimetre.spell(&number(raw));
        assert_eq!(mm(&[0xc1, 0x02, 0x33]), "1500 mm");
        assert_eq!(mm(&[0x3f, 0x60, 0x66]), "-50 mm");
        assert_eq!(mm(&[0x80]), "0 mm");
        assert_eq!(mm(&SLOPE), "2.9999910000486 mm");
        assert_eq!(mm(&[0xbe, 0x0d, 0x1f]), "0.0123 mm");
        assert_eq!(mm(&[0xb7, 0x02]), "1E-17 mm");
        assert_eq!(PublishUnit::Metre.spell(&number(&[0xc0, 0x06])), "0.05 m");
    }

    fn item(uid: &str, class: &'static str) -> PidPublishItem {
        PidPublishItem {
            uid: uid.to_string(),
            model_item_type: String::new(),
            class: Some(class),
            attributes: Vec::new(),
        }
    }

    fn drawing(items: Vec<PidPublishItem>) -> PidPublishDrawing {
        PidPublishDrawing {
            drawing_uid: "D1".to_string(),
            drawing_name: "D".to_string(),
            items,
            process_points: Vec::new(),
            branch_points: Vec::new(),
            ports: Vec::new(),
            connectivity_ends: Vec::new(),
            exp_relations: Vec::new(),
            opc_points: Vec::new(),
            exp: None,
            pipelines: Vec::new(),
            pipelines_attempted: false,
            pipeline_synthesis: PipelineSynthesis::default(),
            representations: Vec::new(),
            stats: PidPublishStats::default(),
        }
    }

    #[test]
    fn the_writer_puts_each_value_in_its_element() {
        let mut drawing = drawing(vec![
            item("4C2F232018CE42E8B7FFCABD553BDBC4", "PIDPipingConnector"),
            item("F1081777313D41ABB14B7B18FA210D83", "PIDProcessVessel"),
            item("A0", "PIDMechanicalEquipment"),
            item("B0", "PIDPipingConnector"),
        ]);
        let mut cells = UnitCells::new();
        cells.insert(
            "4C2F232018CE42E8B7FFCABD553BDBC4".to_string(),
            vec![
                (&RUN_SLOPE, number(&SLOPE)),
                (&RUN_INSULATION, number(&[0xc0, 0x06])),
            ],
        );
        cells.insert(
            "F1081777313D41ABB14B7B18FA210D83".to_string(),
            vec![
                (&EQUIPMENT_HEIGHT, OracleNumber::from(3)),
                (&VESSEL_VOLUME, OracleNumber::from(27)),
            ],
        );
        cells.insert(
            "A0".to_string(),
            vec![
                (&EQUIPMENT_INSULATION, number(&[0xc0, 0x06])),
                (&EQUIPMENT_HEIGHT, OracleNumber::from(2)),
            ],
        );
        let counts = attach_cells(&mut drawing, &cells);
        assert_eq!(
            counts,
            UnitValueCounts {
                written: 5,
                left_out: 1
            }
        );
        assert_eq!(
            drawing.items[0].attributes[0].source,
            PidValueSource::BackupRow {
                table: "T_PIPERUN",
                column: "SP_INSULTHICKSI"
            }
        );
        let xml = write_pid_data_xml(&drawing, "P01");
        for line in [
            r#"      <ISlopedPipingItem SlopedPipingAngle="2.9999910000486E-03 rad"/>"#,
            r#"      <IInsulatedItem TotalInsulThick="50 mm"/>"#,
            r#"      <IPBSItem HeightRelativeToGrade="3 m"/>"#,
            r#"      <IProcessVessel ProcessVessel_VesselVolumetricCapacity="27 m^3"/>"#,
            r#"      <IPBSItem HeightRelativeToGrade="2 m"/>"#,
        ] {
            assert!(xml.lines().any(|written| written == line), "{line}\n{xml}");
        }
        // The connector's two elements stand where its class's row puts
        // them, and a run with no SI value writes neither.
        let connector = xml
            .split("<PIDPipingConnector>")
            .nth(1)
            .expect("the first connector");
        let sloped = connector.find("<ISlopedPipingItem").expect("sloped");
        let insulated = connector.find("<IInsulatedItem").expect("insulated");
        let jacketed = connector.find("<IJacketedItem").expect("jacketed");
        assert!(sloped < insulated && insulated < jacketed, "{connector}");
        let bare = xml
            .split("<PIDPipingConnector>")
            .nth(2)
            .expect("the second connector");
        assert!(!bare.contains("<ISlopedPipingItem"), "{bare}");
        assert!(!bare.contains("<IInsulatedItem"), "{bare}");
        assert_eq!(xml.matches("TotalInsulThick").count(), 1, "{xml}");
    }

    #[test]
    fn a_value_the_item_already_states_is_kept() {
        let mut connector = item("C1", "PIDPipingConnector");
        connector.attributes.push(PidPublishAttribute {
            interface: "IInsulatedItem",
            name: "TotalInsulThick",
            value: "40 mm".to_string(),
            source: PidValueSource::Label { row: 1 },
        });
        let mut drawing = drawing(vec![connector]);
        let mut cells = UnitCells::new();
        cells.insert(
            "C1".to_string(),
            vec![(&RUN_INSULATION, number(&[0xc0, 0x06]))],
        );
        assert_eq!(attach_cells(&mut drawing, &cells).written, 0);
        assert_eq!(drawing.items[0].attributes.len(), 1);
        assert_eq!(drawing.items[0].attributes[0].value, "40 mm");
    }
}
