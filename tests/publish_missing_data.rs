//! 缺数据的五条规则：#71 的 S10 / S12 / S15 / S16 / S17。
//!
//! 每一条断言的是**今天代码的行为**，不是参考发布件的样子。数据到位之后
//! 规则要改，这几条就该跟着红——它们是「改规则」那一步的入口，不是回归门禁。
//! 缺口与改法写在 `docs/analysis/2026-11-15-what-the-export-lines-still-wait-on.md`。
//!
//! 只用入库夹具：`test-file/backup-test/DWG-0202GP06-01_p`（Oracle `exp` 包）
//! 与 `test-file/export-test/publish-data/DWG-0202GP06-01`（同图参考 `_Data.xml`
//! 与 `.pid`）。没有一条读 `D:\work\…`，也没有软跳过分支。

mod common;

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::process::Command;

use pid_parse::publish::model::{PublishDrawing, PublishObject, PublishStyle};
use pid_parse::publish::oracle_exp::ExpTables;
use pid_parse::publish::pipeline::{pipeline_name, pipeline_uid};

use common::Scratch;

const CLI: &str = env!("CARGO_BIN_EXE_pid_publish_xml");
const DWG_PACKAGE: &str = "test-file/backup-test/DWG-0202GP06-01_p";
const DWG_DIR: &str = "test-file/export-test/publish-data/DWG-0202GP06-01";
const DWG_DRAWING_UID: &str = "9D1F3F232C47409D8C54EFB22FC3682A";

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// 整包发布一遍，返回输出目录的持有者（测试结束时删掉）。
fn publish_package() -> Scratch {
    let out_dir = Scratch::unmade("publish-missing-data");
    let output = Command::new(CLI)
        .arg(repo_path(DWG_PACKAGE))
        .arg("--out-dir")
        .arg(out_dir.path())
        .output()
        .expect("spawn pid_publish_xml <DWG package>");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    out_dir
}

fn reference_data_xml() -> String {
    std::fs::read_to_string(repo_path(DWG_DIR).join("DWG-0202GP06-01_Data.xml"))
        .expect("the committed DWG reference")
}

fn dump_bytes() -> Vec<u8> {
    std::fs::read(repo_path(DWG_PACKAGE).join("Export.dmp")).expect("the committed dump")
}

/// 一份 `_Data.xml` 里每个类块的 `IObject UID`，按类分组。
fn block_uids(xml: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut blocks: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut class = String::new();
    for line in xml.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix('<') {
            if let Some(name) = rest.strip_suffix('>') {
                if !name.is_empty()
                    && !name.starts_with('/')
                    && !name.starts_with('?')
                    && !name.contains(' ')
                {
                    class = name.to_string();
                }
            }
        }
        if let Some(rest) = line.strip_prefix("<IObject UID=\"") {
            let uid = rest[..rest.find('"').expect("a closed UID")].to_string();
            blocks.entry(class.clone()).or_default().push(uid);
        }
    }
    blocks
}

/// 一份 `_Data.xml` 里 `PIDPipeline` 块的 `(UID, Name)`。
fn pipeline_uids_and_names(xml: &str) -> Vec<(String, String)> {
    let mut pipelines = Vec::new();
    let mut in_pipeline = false;
    for line in xml.lines().map(str::trim) {
        if line == "<PIDPipeline>" {
            in_pipeline = true;
            continue;
        }
        if line == "</PIDPipeline>" {
            in_pipeline = false;
            continue;
        }
        if !in_pipeline {
            continue;
        }
        let Some(rest) = line.strip_prefix("<IObject UID=\"") else {
            continue;
        };
        let uid = rest[..rest.find('"').expect("a closed UID")].to_string();
        let name = rest
            .find(" Name=\"")
            .map(|at| {
                let rest = &rest[at + 7..];
                rest[..rest.find('"').expect("a closed Name")].to_string()
            })
            .unwrap_or_default();
        pipelines.push((uid, name));
    }
    pipelines
}

/// S10 · 管线 UID 由 plant + 名字合成，不取库里的 `T_PIPELINE.SP_ID`。
///
/// 现在：`pipeline_uid()`（`src/publish/pipeline.rs:205`）是 UUID v5 ——
/// 命名空间 `pid-parse/pipeline` 混 plant，再混管线名；`attach_pipelines`
/// （同文件 `:99`）只拿它当 UID，dump 的 `T_PIPELINE` 一行都不读。
///
/// 数据到位后（DWG 同版发布件）：这条要改成断言每条管线的 UID 等于参考里
/// 那条管线的 UID（`2D010BCE…` 一类），今天这两个集合不相交，一改就红。
#[test]
fn s10_a_pipeline_uid_is_composed_from_the_plant_and_the_name_not_from_the_dump() {
    let out_dir = publish_package();
    let generated = std::fs::read_to_string(out_dir.join("DWG-0202GP06-01_Data.xml"))
        .expect("the published drawing");
    let reference = reference_data_xml();

    let ours = pipeline_uids_and_names(&generated);
    let theirs: Vec<String> = pipeline_uids_and_names(&reference)
        .into_iter()
        .map(|(uid, _)| uid)
        .collect();
    assert_eq!(ours.len(), 6, "the package's dump yields six pipelines");
    assert_eq!(theirs.len(), 5, "the reference states five pipelines");

    // 今天写出的每一条 UID 都等于 `pipeline_uid("P01", 它自己那个名字)`。
    for (uid, name) in &ours {
        assert_eq!(uid.len(), 32, "{uid}");
        assert!(
            uid.chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_lowercase()),
            "{uid} is 32 uppercase hex digits"
        );
        assert_eq!(
            *uid,
            pipeline_uid("P01", name),
            "the UID of `{name}` is composed from the plant and the name"
        );
    }

    // 与参考的管线 UID 一个都不重合：库里那条 `T_PIPELINE.SP_ID` 没有被读。
    let composed: BTreeSet<&String> = ours.iter().map(|(uid, _)| uid).collect();
    for uid in &theirs {
        assert_eq!(uid.len(), 32, "{uid}");
        assert!(
            !composed.contains(uid),
            "{uid} is a reference pipeline UID and no synthesized one equals it"
        );
    }
}

/// S12 · 本图 stockpile 里的 OPC 不发。
///
/// 现在：`opc_points()`（`src/publish/oracle_exp.rs:2276`）按
/// `T_REPRESENTATION` 取本图的 OPC，UID 取配对里较小的那个；`attach_dump`
/// （同文件 `:1568`）再按「表示在 `.pid` 里」筛一次，
/// 于是只有一条表示不在 `.pid` 的 OPC 被丢掉。
///
/// 数据到位后（第二例 stockpile OPC 的发布件）：这条要改成断言它被发出来、
/// UID 取哪一端、带哪些接口；今天的断言就会失败。
#[test]
fn s12_a_stockpiled_opc_is_read_from_the_dump_and_then_dropped_for_having_no_representation() {
    let dump = dump_bytes();
    let tables = ExpTables::read(&dump).expect("the committed dump reads");
    let drawing =
        pid_parse::publish::load_publish_from_pid(&repo_path(DWG_DIR).join("DWG-0202GP06-01.pid"))
            .expect("the committed .pid loads");
    assert_eq!(drawing.drawing_uid, DWG_DRAWING_UID);

    let points = tables.opc_points(&drawing.drawing_uid).expect("opc points");
    assert_eq!(points.len(), 5, "the dump places five OPCs on this drawing");

    // `.pid` 画了 39 个表示，其中四个 OPC 的表示在里面。
    let drawn: HashSet<&str> = drawing
        .representations
        .iter()
        .map(|representation| representation.uid.as_str())
        .collect();
    let kept: Vec<&str> = points
        .iter()
        .filter(|point| drawn.contains(point.representation_uid.as_str()))
        .map(|point| point.uid.as_str())
        .collect();
    let dropped: Vec<&str> = points
        .iter()
        .filter(|point| !drawn.contains(point.representation_uid.as_str()))
        .map(|point| point.uid.as_str())
        .collect();
    assert_eq!(kept.len(), 4, "four OPCs are drawn by the .pid");
    assert_eq!(
        dropped,
        vec!["799B9008DB6848F28127C78AC745C5A8"],
        "the one in this drawing's stockpile: no graphic oid, so the .pid has no representation"
    );

    // 参考发了它；我方没发。
    let reference = reference_data_xml();
    let reference_branch_points: Vec<String> = block_uids(&reference)
        .remove("PIDBranchPoint")
        .expect("the reference carries branch points");
    assert!(
        reference_branch_points
            .iter()
            .any(|uid| uid == "799B9008DB6848F28127C78AC745C5A8"),
        "the reference publishes the stockpiled OPC: {reference_branch_points:?}"
    );

    let out_dir = publish_package();
    let generated = std::fs::read_to_string(out_dir.join("DWG-0202GP06-01_Data.xml"))
        .expect("the published drawing");
    let our_branch_points: Vec<String> = block_uids(&generated)
        .remove("PIDBranchPoint")
        .expect("the drawing publishes branch points");
    assert_eq!(
        our_branch_points.len(),
        4,
        "we publish the four drawn ones only: {our_branch_points:?}"
    );
    assert!(
        !generated.contains("799B9008DB6848F28127C78AC745C5A8"),
        "the stockpiled OPC is not in our document"
    );
}

/// S15 · 数据库线：带正文的注释写成空 `<INote/>`，有 `ToFromText` 的 OPC 缺
/// `IOffpageConnector`。
///
/// 现在：`xml_writer::write_item_note`（`src/publish/xml_writer.rs:1509`）
/// 取 `NoteText` 字段，没有就取 `T_ModelItem.Description`；库里正文在
/// `T_NOTE.BODY`，`mdf_load` 暂存的 24 张表里没有 `T_NOTE`，于是正文为空。
/// `catalog::run_branch_point`（`src/publish/catalog.rs:196`）走
/// `write_pid_branch_point`（`src/publish/xml_writer.rs:1969`），它从不写
/// `IOffpageConnector`，而 `T_OPC` 也不在暂存的表里。
///
/// 数据到位后（一份带注释正文与 OPC 的 MDF / MTF 备份）：这条要改成断言
/// `<INote NoteText="…"/>` 带正文、`<IOffpageConnector ToFromText="…"/>` 写出来；
/// 今天的断言就会失败。
#[test]
fn s15_a_database_note_has_no_body_and_a_database_branch_point_has_no_offpage_connector() {
    let mut drawing = PublishDrawing::new("DWG-TEST", "DWG-TEST");
    drawing.style = PublishStyle::Dwg;
    drawing.objects = vec![
        PublishObject {
            uid: "NOTE0000000000000000000000000001".into(),
            item_type_name: "Note".into(),
            fields: [("Body".to_string(), "量液孔".to_string())]
                .into_iter()
                .collect(),
            ..PublishObject::default()
        },
        PublishObject {
            uid: "BRANCHPOINT00000000000000000001".into(),
            item_type_name: "BranchPoint".into(),
            fields: [("ToFromText".to_string(), "DWG-1".to_string())]
                .into_iter()
                .collect(),
            ..PublishObject::default()
        },
    ];
    let xml = pid_parse::publish::write_data_xml(&drawing, "P01").expect("the document renders");

    // 注释：`Body` 不是这个写入器读的字段，正文丢了。
    assert!(
        xml.contains("<INote/>"),
        "a note whose body sits in a `Body` field renders bare: \n{xml}"
    );
    assert!(
        !xml.contains("NoteText"),
        "no note text reaches the element: \n{xml}"
    );
    assert!(!xml.contains("量液孔"), "the body is not written: \n{xml}");

    // OPC：`ToFromText` 在对象字段里，写出器不读它。
    assert!(
        !xml.contains("IOffpageConnector"),
        "the database line's branch point carries no IOffpageConnector: \n{xml}"
    );
    assert!(
        !xml.contains("DWG-1"),
        "the ToFromText is not written: \n{xml}"
    );
}

/// S16 · 管线名回退拼名时流体那一格是空的：19 号代码表只读
/// `CODELIST_SHORT_TEXT`。
///
/// 现在：`attach_pipelines`（`src/publish/pipeline.rs:51`）调
/// `codelist_short_text(19)`（`src/publish/oracle_exp.rs:791`），它只读
/// `CODELIST_SHORT_TEXT` 一列；出厂自带的代号只填 `CODELIST_TEXT`，于是
/// `fluid_slot` 认为这个代号不在表里，名字里少一格并计入 `fluid_code_missing`。
///
/// 数据到位后（一条走回退拼名、代号又只有 `CODELIST_TEXT` 的管段与它的
/// 发布件）：这条要改成断言名字里带那个代号；今天的断言就会失败。
#[test]
fn s16_a_fluid_code_that_only_states_codelist_text_is_left_out_of_a_spelled_pipeline_name() {
    let dump = dump_bytes();
    let tables = ExpTables::read(&dump).expect("the committed dump reads");
    let fluids = tables.codelist_short_text(19).expect("codelist 19");

    // 19 号表：短文本只有 55 条，两个出厂代号（493 = OD、20 = AI）不在里面。
    assert_eq!(fluids.len(), 55, "the short texts codelist 19 states");
    assert!(!fluids.contains_key(&493), "493 = OD has no short text");
    assert!(!fluids.contains_key(&20), "20 = AI has no short text");

    // 回退拼名时流体那一格因此是空的：`pipeline_name` 拿到 `None`。
    let run = pid_parse::publish::oracle_exp::PipeRunRow {
        id: "RUN".into(),
        id_key: "RUN".into(),
        nominal_diameter: Some(50),
        fluid_code: Some(493),
        materials_class: "1.6AR12".into(),
        tag_sequence: "0401".into(),
        fluid_system: Some(8),
        insulation_spec: "WE50mm".into(),
        item_tag: None,
        insulation_thickness_si: None,
        slope_si: None,
    };
    assert_eq!(
        pipeline_name(&run, None),
        "50 mm-0401-1.6AR12-WE50mm",
        "the fluid slot is gone when the code has no short text"
    );
    // 有短文本时代号在名字里，说明少的那一格就是这一列。
    assert_eq!(
        pipeline_name(&run, Some("OD")),
        "50 mm-OD-0401-1.6AR12-WE50mm"
    );

    // 本包的 dump 上，这条规则把五条管段计成「代号缺失」。
    let drawing =
        pid_parse::publish::load_publish_from_pid(&repo_path(DWG_DIR).join("DWG-0202GP06-01.pid"))
            .expect("the committed .pid loads");
    let mut attached = drawing.clone();
    pid_parse::publish::pipeline::attach_pipelines(&mut attached, &tables, "P01", None)
        .expect("pipelines");
    assert_eq!(
        attached.pipeline_synthesis.fluid_code_missing, 5,
        "five connectors of the package state a code codelist 19 has no short text for"
    );
}

/// S17 · 库里画在图上、`.pid` 没画的非管件对象不发。
///
/// 现在：`dump_only_piping_components`（`src/publish/oracle_exp.rs:678`）只
/// 认 `T_MODELITEM.ITEMTYPENAME = "PipingComp"`（同文件 `:692`）的行，
/// `attach_dump_only_components`（同文件 `:1398`）只把它的返回值挂上去；
/// 仪表、注释、管嘴、机械设备、容器都没有这条规则。
///
/// 这一条钉的是那个投影的**范围**：入库包上它只回来 `PipingComp`，
/// 别的类连看都不看。C井场 的 21 个对象要等原厂发布件，所以这里不是
/// 端到端断言。
///
/// 数据到位后（C井场 的发布件）：这条要改成断言那 21 个对象被发出来、
/// 且类别与参考一致；今天的断言就会失败。
#[test]
fn s17_only_a_piping_component_the_database_draws_is_added_and_no_other_class_is_looked_at() {
    let dump = dump_bytes();
    let tables = ExpTables::read(&dump).expect("the committed dump reads");

    // 空 `drawn` 集合：这个投影在本图上一共看到两个表示，两个都是管件。
    let all = tables
        .dump_only_piping_components(DWG_DRAWING_UID, &HashSet::new())
        .expect("dump-only components");
    assert_eq!(all.len(), 2, "the dump places two piping components here");
    let types: BTreeSet<&str> = all
        .iter()
        .map(|(item, _)| item.model_item_type.as_str())
        .collect();
    assert_eq!(
        types,
        BTreeSet::from(["PipingComp"]),
        "the projection never returns another item type"
    );
    for (item, _) in &all {
        assert_eq!(item.class, Some("PIDPipingComponent"));
    }

    // `.pid` 画了这两个表示，所以整包发布一个也不加：C井场 的 21 个对象
    // 在这里没有任何对应的路径。
    let drawing =
        pid_parse::publish::load_publish_from_pid(&repo_path(DWG_DIR).join("DWG-0202GP06-01.pid"))
            .expect("the committed .pid loads");
    let drawn: HashSet<String> = drawing
        .representations
        .iter()
        .map(|representation| pid_parse::publish::oracle_exp::backup_uid_key(&representation.uid))
        .collect();
    let added = tables
        .dump_only_piping_components(DWG_DRAWING_UID, &drawn)
        .expect("dump-only components");
    assert!(
        added.is_empty(),
        "both are drawn by the .pid, so nothing is added: {added:?}"
    );
}
