//! Which .igr does each fixture link? And does each document's
//! Sheet6 carry decodable boundary records?

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
        let links: Vec<&String> = parsed
            .jsites
            .iter()
            .flat_map(|s| s.ole_links.iter())
            .filter(|l| l.to_lowercase().contains(".igr"))
            .collect();
        let bounds: usize = parsed
            .sheet_streams
            .iter()
            .filter_map(|s| s.geometry.as_ref())
            .map(|g| g.decoded_igboundaries.len())
            .sum();
        println!("{tag}: igr_links={links:?} own_boundaries={bounds}");
    }
    Ok(())
}
