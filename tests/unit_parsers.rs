use pid_parse::parsers::cluster_header::{self, CLUSTER_MAGIC};
use pid_parse::parsers::magic;
use pid_parse::parsers::xml_util;

// ─── xml_util::collect_simple_tags ───

#[test]
fn collect_simple_tags_basic() {
    let xml = "<Root><Name>hello</Name><Value>42</Value></Root>";
    let tags = xml_util::collect_simple_tags(xml);
    assert_eq!(tags.get("Name").map(String::as_str), Some("hello"));
    assert_eq!(tags.get("Value").map(String::as_str), Some("42"));
}

#[test]
fn collect_simple_tags_skips_nested() {
    let xml = "<Outer><Inner><Deep>x</Deep></Inner></Outer>";
    let tags = xml_util::collect_simple_tags(xml);
    // "Inner" has value containing '<', should be skipped
    assert!(!tags.contains_key("Inner"));
    // "Deep" is a simple tag
    assert_eq!(tags.get("Deep").map(String::as_str), Some("x"));
}

#[test]
fn collect_simple_tags_empty_value() {
    let xml = "<Tag></Tag>";
    let tags = xml_util::collect_simple_tags(xml);
    assert_eq!(tags.get("Tag").map(String::as_str), Some(""));
}

#[test]
fn collect_simple_tags_whitespace_trimmed() {
    let xml = "<Tag>  spaced  </Tag>";
    let tags = xml_util::collect_simple_tags(xml);
    assert_eq!(tags.get("Tag").map(String::as_str), Some("spaced"));
}

#[test]
fn collect_simple_tags_sp_prefixed() {
    let xml = "<SP_RulesUID>abc-123</SP_RulesUID><SP_FormatsUID>def</SP_FormatsUID>";
    let tags = xml_util::collect_simple_tags(xml);
    assert_eq!(tags.get("SP_RulesUID").map(String::as_str), Some("abc-123"));
    assert_eq!(tags.get("SP_FormatsUID").map(String::as_str), Some("def"));
}

// ─── cluster_header::parse_header ───

fn make_header_bytes(
    magic: u32,
    rec_count: u32,
    stream_type: u16,
    body_len: u32,
    flags: u16,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend_from_slice(&magic.to_le_bytes());
    buf.extend_from_slice(&rec_count.to_le_bytes());
    buf.extend_from_slice(&stream_type.to_le_bytes());
    buf.extend_from_slice(&body_len.to_le_bytes());
    buf.extend_from_slice(&flags.to_le_bytes());
    buf
}

#[test]
fn parse_header_valid() {
    let data = make_header_bytes(CLUSTER_MAGIC, 10, 0x0003, 1024, 0x0001);
    let hdr = cluster_header::parse_header(&data).expect("should parse valid header");
    assert_eq!(hdr.magic, CLUSTER_MAGIC);
    assert_eq!(hdr.record_count, 10);
    assert_eq!(hdr.stream_type, 0x0003);
    assert_eq!(hdr.body_len, 1024);
    assert_eq!(hdr.flags, 0x0001);
}

#[test]
fn parse_header_too_short_15_bytes() {
    let data = make_header_bytes(CLUSTER_MAGIC, 10, 0x0003, 1024, 0x0001);
    // Only 15 bytes — must return None (boundary bug fix verification)
    assert!(cluster_header::parse_header(&data[..15]).is_none());
}

#[test]
fn parse_header_exactly_16_bytes() {
    let data = make_header_bytes(CLUSTER_MAGIC, 5, 0x0001, 512, 0x0000);
    assert!(cluster_header::parse_header(&data[..16]).is_some());
}

#[test]
fn parse_header_wrong_magic() {
    let data = make_header_bytes(0xDEADBEEF, 1, 0, 0, 0);
    assert!(cluster_header::parse_header(&data).is_none());
}

#[test]
fn parse_header_empty() {
    assert!(cluster_header::parse_header(&[]).is_none());
}

// ─── cluster_header::parse_string_table ───

fn make_string_entry(index: u32, text: &str) -> Vec<u8> {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let byte_len = (utf16.len() * 2) as u32;
    let mut buf = Vec::new();
    buf.extend_from_slice(&index.to_le_bytes());
    buf.extend_from_slice(&byte_len.to_le_bytes());
    for w in &utf16 {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    buf
}

fn make_sentinel() -> Vec<u8> {
    // index=0, byte_len=0
    vec![0, 0, 0, 0, 0, 0, 0, 0]
}

#[test]
fn parse_string_table_basic() {
    let mut data = Vec::new();
    data.extend(make_string_entry(1, "Hello"));
    data.extend(make_string_entry(2, "World"));
    data.extend(make_sentinel());

    let (table, _end) = cluster_header::parse_string_table(&data, 0);
    assert_eq!(table.len(), 2);
    assert_eq!(table[0].index, 1);
    assert_eq!(table[0].value, "Hello");
    assert_eq!(table[1].index, 2);
    assert_eq!(table[1].value, "World");
}

#[test]
fn parse_string_table_empty_string_not_sentinel() {
    // Non-zero index with byte_len=0 should be kept as empty string, not treated as sentinel
    let mut data = Vec::new();
    data.extend(make_string_entry(1, "First"));
    // index=5, byte_len=0 — empty string, NOT sentinel
    data.extend_from_slice(&5u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend(make_string_entry(3, "Third"));
    data.extend(make_sentinel());

    let (table, _end) = cluster_header::parse_string_table(&data, 0);
    assert_eq!(
        table.len(),
        3,
        "should parse 3 entries (empty string not sentinel)"
    );
    assert_eq!(table[0].value, "First");
    assert_eq!(table[1].index, 5);
    assert_eq!(table[1].value, "");
    assert_eq!(table[2].value, "Third");
}

#[test]
fn parse_string_table_immediate_sentinel() {
    let data = make_sentinel();
    let (table, _end) = cluster_header::parse_string_table(&data, 0);
    assert!(table.is_empty(), "sentinel at start → empty table");
}

#[test]
fn parse_string_table_truncated() {
    // Entry claims 100 bytes but data ends early
    let mut data = Vec::new();
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&100u32.to_le_bytes());
    data.extend_from_slice(&[0x41, 0x00]); // only 2 bytes of payload

    let (table, _end) = cluster_header::parse_string_table(&data, 0);
    assert!(table.is_empty(), "truncated entry should not parse");
}

// ─── magic::magic_tag / describe_magic ───

#[test]
fn magic_tag_renders_known_streams() {
    // PSMroots: 'root'   (on-disk bytes: 72 6F 6F 74 -> u32 LE = 0x746F6F72)
    assert_eq!(magic::magic_tag(0x746F6F72).as_deref(), Some("root"));
    // PSMclustertable: 'clst'
    assert_eq!(magic::magic_tag(0x74736C63).as_deref(), Some("clst"));
    // PSMsegmenttable: 'stab'
    assert_eq!(magic::magic_tag(0x62617473).as_deref(), Some("stab"));
    // DocVersion3: 'Smar'
    assert_eq!(magic::magic_tag(0x72616D53).as_deref(), Some("Smar"));
}

#[test]
fn magic_tag_rejects_non_printable() {
    // Cluster header magic contains 0xF5 0x90 — not printable ASCII
    assert!(magic::magic_tag(0x6C90F544).is_none());
    assert!(magic::magic_tag(0x00000005).is_none());
}

#[test]
fn describe_magic_for_known_and_unknown() {
    assert!(magic::describe_magic(0x6C90F544).contains("cluster"));
    assert!(magic::describe_magic(0x746F6F72).contains("root"));
    assert!(magic::describe_magic(0x72616D53).contains("SmartPlant"));
    assert_eq!(magic::describe_magic(0x12345678), "");
}

// ─── Sheet probe integration (real file) ───

#[test]
fn sheet_stream_reuses_cluster_header() {
    // `test-file/` is gitignored; the sample is available to contributors
    // with SmartPlant access but not in CI. Skip cleanly when the fixture
    // is missing, matching the pattern in writer_real_files.rs /
    // parse_real_files.rs.
    let fixture = "test-file/DWG-0201GP06-01.pid";
    if !std::path::Path::new(fixture).exists() {
        eprintln!("skipping: fixture {} not found", fixture);
        return;
    }
    let parser = pid_parse::PidParser::new();
    let doc = parser.parse_file(fixture).expect("parse fixture");
    assert!(!doc.sheet_streams.is_empty(), "expected at least one sheet");
    let sheet = &doc.sheet_streams[0];
    let magic = sheet.magic_u32_le.expect("sheet stream must have magic");
    assert_eq!(
        magic, CLUSTER_MAGIC,
        "sheet stream shares cluster magic 0x{:08X}, got 0x{:08X}",
        CLUSTER_MAGIC, magic
    );
    let hdr = sheet.header.as_ref().expect("sheet header must parse");
    assert_eq!(hdr.magic, CLUSTER_MAGIC);
    assert!(hdr.record_count > 0, "sheet header should report records");
    let ps = sheet.probe_summary.as_ref().expect("probe summary exists");
    // Sheet streams do not use the 0x89 marker format of DA, so marker_count
    // should be zero; we only verify the probe produced a plausible start/scan.
    assert_eq!(
        ps.marker_count, 0,
        "sheet streams are not expected to use 0x89 DA markers"
    );
    assert!(
        ps.body_start_offset >= 8 && ps.body_start_offset < sheet.size as usize,
        "body_start={} should be within stream",
        ps.body_start_offset
    );
    assert!(ps.bytes_scanned > 0);
}
