//! 演示 `pid_parse::byte_audit` 框架的程序化用法。
//!
//! 这个示例不读真实 `.pid` 文件 —— 它用合成的几条 stream 构造一个
//! 极小的 [`PidPackage`]，然后：
//!
//! 1. 用 [`byte_audit::aggregate::byte_audit_report`] 跑出一份字节
//!    审计报告，打印 per-stream / overall 统计。
//! 2. 把 baseline 报告小幅修改一下（模拟覆盖率回归），再调
//!    [`byte_audit::compare::compare_byte_audit_reports`] 对比，演示
//!    `is_clean()` / `regressions` / `improvements` 三种语义。
//! 3. 把当前报告序列化为 JSON，演示 `--byte-audit --json` 等同的
//!    程序化导出。
//!
//! Usage:
//!     cargo run --example byte_audit_demo
//!
//! 期望输出（节选，字段顺序按 `BTreeMap` 路径排序决定）：
//!
//! ```text
//! == byte_audit_demo: 4 synthetic streams ==
//!   /DocVersion2                              : 21/21 consumed (parse_doc_version2)
//!   /MysteryStream                            : 6/6 leftover (no parser)
//!   /PSMsegmenttable                          : 10/10 consumed (parse_psm_segment_table)
//!   /TaggedTxtData/Drawing                    : 55/55 consumed (parse_drawing_xml)
//!
//!   overall: 86/92 bytes consumed (93.48%)
//!   unregistered streams (1): ["/MysteryStream"]
//!
//! -- baseline vs current comparison --
//! is_clean = false (regressions: 2, improvements: 0)
//!   REGRESSION OverallCoverageDecreased on None: 0.934783 -> 0.826087
//!   REGRESSION StreamConsumedBytesDecreased on Some("/PSMsegmenttable"): 10 -> 0
//! ```
//!
//! 真实使用场景：CI 把 baseline JSON 提交到仓库，新 commit 跑出
//! current JSON，调 `compare_byte_audit_reports` 把 regression /
//! improvement 列表 surface 给 reviewer。本 demo 不依赖任何 fixture，
//! 适合作为下游集成 byte-audit framework 时的入门样板。

use std::collections::BTreeMap;
use std::error::Error;

use pid_parse::byte_audit::aggregate::byte_audit_report;
use pid_parse::byte_audit::compare::compare_byte_audit_reports;
use pid_parse::{PidDocument, PidPackage, RawStream};

fn main() -> Result<(), Box<dyn Error>> {
    println!("== byte_audit_demo: 4 synthetic streams ==");

    let baseline_pkg = build_demo_package();
    let baseline = byte_audit_report(&baseline_pkg);

    print_per_stream(&baseline);
    print_overall(&baseline);

    println!("\n-- JSON serialization --");
    let json = serde_json::to_string_pretty(&baseline)?;
    println!("baseline report serialised: {} bytes", json.len());

    println!("\n-- baseline vs current comparison --");
    let current_pkg = build_regressed_package();
    let current = byte_audit_report(&current_pkg);
    let comparison = compare_byte_audit_reports(&baseline, &current);

    println!(
        "is_clean = {} (regressions: {}, improvements: {})",
        comparison.is_clean(),
        comparison.regressions.len(),
        comparison.improvements.len(),
    );
    for r in &comparison.regressions {
        println!(
            "  REGRESSION {:?} on {:?}: {} -> {}",
            r.kind, r.path, r.baseline_value, r.current_value
        );
    }
    for i in &comparison.improvements {
        println!(
            "  IMPROVEMENT {:?} on {:?}: {} -> {}",
            i.kind, i.path, i.baseline_value, i.current_value
        );
    }

    Ok(())
}

fn print_per_stream(report: &pid_parse::ByteAuditReport) {
    for (path, summary) in &report.per_stream {
        let status = match &summary.parser_name {
            Some(name) => format!("{}/{} consumed ({})", summary.consumed_bytes, summary.total_bytes, name),
            None => format!("{}/{} leftover (no parser)", summary.leftover_bytes, summary.total_bytes),
        };
        println!("  {:42}: {status}", path);
    }
}

fn print_overall(report: &pid_parse::ByteAuditReport) {
    let pct = report.overall_coverage_ratio * 100.0;
    println!(
        "\n  overall: {}/{} bytes consumed ({pct:.2}%)",
        report.overall_consumed, report.total_file_bytes,
    );
    println!(
        "  unregistered streams ({}): {:?}",
        report.unregistered_paths.len(),
        report.unregistered_paths,
    );
}

/// 构造一个含 4 条已知 stream 的 baseline 包：
/// - `/PSMsegmenttable`：`stab` magic + count=2 + 2 字节 payload，会被
///   `parse_psm_segment_table_with_trace` 完全消费。
/// - `/DocVersion2`：12 字节 prefix + 一条 op record，会被
///   `parse_doc_version2_with_trace` 完全消费。
/// - `/TaggedTxtData/Drawing`：合法 UTF-8 XML，会被 XML 整 stream
///   消费。
/// - `/MysteryStream`：6 字节随机数据，没有 parser 注册 →
///   `parser_name = None` + 全部 leftover。
fn build_demo_package() -> PidPackage {
    let entries: &[(&str, Vec<u8>)] = &[
        ("/PSMsegmenttable", make_psm_segment_bytes(2)),
        ("/DocVersion2", make_doc_version2_bytes()),
        (
            "/TaggedTxtData/Drawing",
            br#"<Drawing><DrawingNumber>D-001</DrawingNumber></Drawing>"#.to_vec(),
        ),
        ("/MysteryStream", vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]),
    ];
    pkg_with_streams(entries)
}

/// 构造一个 "current" 包：故意把 `/PSMsegmenttable` 的 magic 破坏掉，
/// 让 trace 跑下来不再 fully consumed → 模拟 stream-level coverage
/// regression（`StreamConsumedBytesDecreased`）。其余 stream 与
/// baseline 完全一致。
fn build_regressed_package() -> PidPackage {
    let mut bad_segment = make_psm_segment_bytes(2);
    bad_segment[0] = 0x00;
    bad_segment[1] = 0x00;
    bad_segment[2] = 0x00;
    bad_segment[3] = 0x00;

    let entries: &[(&str, Vec<u8>)] = &[
        ("/PSMsegmenttable", bad_segment),
        ("/DocVersion2", make_doc_version2_bytes()),
        (
            "/TaggedTxtData/Drawing",
            br#"<Drawing><DrawingNumber>D-001</DrawingNumber></Drawing>"#.to_vec(),
        ),
        ("/MysteryStream", vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]),
    ];
    pkg_with_streams(entries)
}

fn pkg_with_streams(entries: &[(&str, Vec<u8>)]) -> PidPackage {
    let mut streams = BTreeMap::new();
    for (path, data) in entries {
        streams.insert(
            (*path).to_string(),
            RawStream {
                path: (*path).to_string(),
                data: data.clone(),
                modified: false,
            },
        );
    }
    PidPackage::new(None, streams, PidDocument::default())
}

fn make_psm_segment_bytes(count: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0x6261_7473u32.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    data.resize(data.len() + count as usize, 0x01);
    data
}

fn make_doc_version2_bytes() -> Vec<u8> {
    let mut data = vec![0u8; 12];
    data[0..4].copy_from_slice(&0x0001_0034u32.to_le_bytes());
    data.extend_from_slice(&[0x82, 0x00, 0x00, 0x09, 0x00, 0x90, 0x00, 0x00, 0x00]);
    data
}
