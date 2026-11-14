//! The unit-bearing values the committed DWG package's dump states, as the
//! whole-package publish writes them, against the committed reference.
//!
//! The package is a later state of the plant than the reference, so the
//! two are compared object by object: every value written on an object the
//! reference also publishes is the reference's spelling, and every such
//! value of the reference is written. The package and the reference are
//! both in the repository; nothing here skips.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use common::Scratch;

const CLI: &str = env!("CARGO_BIN_EXE_pid_publish_xml");
const ATTRIBUTES: [&str; 4] = [
    "TotalInsulThick",
    "SlopedPipingAngle",
    "HeightRelativeToGrade",
    "ProcessVessel_VesselVolumetricCapacity",
];

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// The objects of a `_Data.xml` by `IObject UID`, each with the value of
/// every attribute of [`ATTRIBUTES`] it states, keyed `Interface.Attribute`.
fn unit_values(xml: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut objects: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut uid = String::new();
    for line in xml.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("<IObject UID=\"") {
            uid = rest[..rest.find('"').expect("a closed UID")].to_string();
            objects.entry(uid.clone()).or_default();
            continue;
        }
        let Some(interface) = line
            .strip_prefix('<')
            .and_then(|rest| rest.split([' ', '/']).next())
        else {
            continue;
        };
        for attribute in ATTRIBUTES {
            let key = format!(" {attribute}=\"");
            if let Some(start) = line.find(&key) {
                let rest = &line[start + key.len()..];
                let value = &rest[..rest.find('"').expect("a closed value")];
                objects
                    .get_mut(&uid)
                    .expect("an attribute inside an object")
                    .insert(format!("{interface}.{attribute}"), value.to_string());
            }
        }
    }
    objects
}

fn stated(
    objects: &BTreeMap<String, BTreeMap<String, String>>,
) -> BTreeMap<(String, String), String> {
    objects
        .iter()
        .flat_map(|(uid, values)| {
            values
                .iter()
                .map(move |(key, value)| ((uid.clone(), key.clone()), value.clone()))
        })
        .collect()
}

#[test]
fn the_dwg_package_writes_its_unit_values_as_the_reference_spells_them() {
    let out_dir = Scratch::unmade("unit-values");
    let output = Command::new(CLI)
        .arg(repo_path("test-file/backup-test/DWG-0202GP06-01_p"))
        .arg("--out-dir")
        .arg(out_dir.path())
        .output()
        .expect("spawn pid_publish_xml <DWG package>");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    let read = |name: &str| {
        std::fs::read_to_string(out_dir.join(format!("{name}_Data.xml")))
            .unwrap_or_else(|error| panic!("{name}: {error}"))
    };

    let generated = unit_values(&read("DWG-0202GP06-01"));
    let reference = unit_values(
        &std::fs::read_to_string(repo_path(
            "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml",
        ))
        .expect("the reference"),
    );
    let on_both = |objects: &BTreeMap<String, BTreeMap<String, String>>,
                   other: &BTreeMap<String, BTreeMap<String, String>>| {
        stated(objects)
            .into_iter()
            .filter(|((uid, _), _)| other.contains_key(uid))
            .collect::<BTreeMap<_, _>>()
    };
    let written = on_both(&generated, &reference);
    assert_eq!(written, on_both(&reference, &generated));
    let pinned: Vec<(&str, &str, &str)> = written
        .iter()
        .map(|((uid, key), value)| (uid.as_str(), key.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        pinned,
        [
            (
                "4C2F232018CE42E8B7FFCABD553BDBC4",
                "IInsulatedItem.TotalInsulThick",
                "50 mm"
            ),
            (
                "4C2F232018CE42E8B7FFCABD553BDBC4",
                "ISlopedPipingItem.SlopedPipingAngle",
                "2.9999910000486E-03 rad"
            ),
            (
                "61BA43D9FA854B8EBE6E0FACBCBC64BD",
                "IInsulatedItem.TotalInsulThick",
                "50 mm"
            ),
            (
                "7042E6B0A1A944E390A08A5A44771C46",
                "IInsulatedItem.TotalInsulThick",
                "50 mm"
            ),
            (
                "9CE3F6FAD0F949DDBE8630B0A0DB5993",
                "IInsulatedItem.TotalInsulThick",
                "50 mm"
            ),
            (
                "C7C3E09265174C91AC008F3CE02E9653",
                "IInsulatedItem.TotalInsulThick",
                "50 mm"
            ),
            (
                "ED053C1B662D45F887952F3C5968A094",
                "IInsulatedItem.TotalInsulThick",
                "50 mm"
            ),
            (
                "F1081777313D41ABB14B7B18FA210D83",
                "IPBSItem.HeightRelativeToGrade",
                "3 m"
            ),
            (
                "F1081777313D41ABB14B7B18FA210D83",
                "IProcessVessel.ProcessVessel_VesselVolumetricCapacity",
                "27 m^3"
            ),
        ]
    );
    // The drawing states no unit value on an object the reference lacks.
    assert_eq!(stated(&generated), written);

    // The package's other two drawings with such a value have no reference:
    // the dump's eight insulated runs and two sloped ones are 6 + 1 + 1 and
    // 1 + 1 over the three drawings.
    let elsewhere: Vec<(String, (String, String), String)> = ["DWG-0201GP06-01", "DWG-0104GP01-08"]
        .into_iter()
        .flat_map(|name| {
            stated(&unit_values(&read(name)))
                .into_iter()
                .map(move |(key, value)| (name.to_string(), key, value))
        })
        .collect();
    let elsewhere: Vec<(&str, &str, &str)> = elsewhere
        .iter()
        .map(|(name, (_, key), value)| (name.as_str(), key.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        elsewhere,
        [
            ("DWG-0201GP06-01", "IInsulatedItem.TotalInsulThick", "50 mm"),
            (
                "DWG-0201GP06-01",
                "ISlopedPipingItem.SlopedPipingAngle",
                "2.9999910000486E-03 rad"
            ),
            ("DWG-0104GP01-08", "IInsulatedItem.TotalInsulThick", "50 mm"),
        ]
    );
}
