//! Count igBoundary2d records by sub_type_word in the .pid sheets —
//! did the template-variant branch admit new records on drawing sheets?

use pid_parse::{ParseOptions, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for (tag, path) in [
        (
            "W",
            r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        ),
        (
            "A",
            r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
        ),
        (
            "B",
            r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        ),
        (
            "LT",
            r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
        ),
        (
            "HE",
            r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
        ),
    ] {
        let parsed = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
        let mut split = std::collections::BTreeMap::new();
        for s in &parsed.sheet_streams {
            if let Some(g) = &s.geometry {
                for b in &g.decoded_igboundaries {
                    *split.entry((s.path.clone(), b.sub_type_word)).or_insert(0) += 1;
                }
            }
        }
        println!("{tag}: {split:?}");
    }
    Ok(())
}
