//! List the boxed `Text` records one drawing carries, with the paragraph
//! alignment its resolved style states, so a run's start can be read against
//! the box edge it names.
//!
//! `probe_boxed_text_alignment <drawing.pid> [substring]` prints every record
//! whose `text_box_width` is set: value, insertion in millimetres, anchor
//! code, box width, resolved alignment, font and letter height.

use std::path::PathBuf;

use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: <drawing.pid> [substring]");
    let filter = args.next();
    let doc = PidParser::new().parse_file(PathBuf::from(&path))?;
    let styles = pid_parse::style_link::text_styles_for_document(&doc);
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let mut boxes = 0usize;
    for entity in &geometry.entities {
        let PidGraphicKind::Text {
            insertion,
            value,
            height,
            anchor_code,
            text_box_width,
            ..
        } = &entity.kind
        else {
            continue;
        };
        let Some(width) = text_box_width.filter(|width| *width > 0.0) else {
            continue;
        };
        boxes += 1;
        if filter
            .as_deref()
            .is_some_and(|needle| !value.contains(needle))
        {
            continue;
        }
        let styled = entity.source.stream_path.as_ref().and_then(|stream| {
            entity
                .graphic_oid
                .and_then(|oid| styles.get(&(stream.clone(), oid)))
                .and_then(|style| style.effective())
        });
        let insertion_mm = [insertion.x * 1000.0, insertion.y * 1000.0];
        println!(
            "{value:?}\n   insertion_mm ({:.3},{:.3}) code {:#04x} box {:.3} align {:?} font {:?} height {:.3} oid {:?} stream {}",
            insertion_mm[0],
            insertion_mm[1],
            anchor_code,
            width * 1000.0,
            styled.as_ref().and_then(|style| style.alignment),
            styled.as_ref().and_then(|style| style.font_name.clone()),
            height * 1000.0,
            entity.graphic_oid,
            entity.source.stream_path.as_deref().unwrap_or("?"),
        );
    }
    eprintln!("boxed records: {boxes}");
    Ok(())
}
