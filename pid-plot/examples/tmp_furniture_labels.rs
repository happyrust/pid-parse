//! Which paper primitives on a page are furniture, and where the ones a
//! reference disagrees with sit.
//!
//! ```text
//! cargo run -p pid-plot --example tmp_furniture_labels -- "<drawing.pid>" "<corpus>"
//! ```

use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pid = PathBuf::from(std::env::args().nth(1).expect("usage: <pid> <corpus>"));
    let corpus = PathBuf::from(std::env::args().nth(2).expect("usage: <pid> <corpus>"));
    let model = lay_out_drawing(&pid, &corpus)?;
    if let Some(template) = &model.template {
        println!(
            "template {} offset {:?} scale {}",
            template.path, template.offset_mm, template.scale
        );
    }
    let mut furniture = 0usize;
    for primitive in &model.primitives {
        let PaperShape::Text {
            origin_mm,
            height_mm,
            value,
            anchor,
            line,
            ..
        } = &primitive.shape
        else {
            continue;
        };
        let wanted = value.contains("中国石油工程建设有限公司") || value.trim() == "CPECC";
        if primitive.furniture {
            furniture += 1;
        }
        if wanted {
            println!(
                "furniture={} {value:?} at [{:.3}, {:.3}] h={:?} anchor={anchor:?} line={line:?}",
                primitive.furniture, origin_mm[0], origin_mm[1], height_mm
            );
        }
    }
    println!(
        "{furniture} furniture text primitives of {}",
        model.primitives.len()
    );
    Ok(())
}
