//! What does the W view filter set say about ConsistencyChecks?

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in [
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
    ] {
        let doc = PidParser::new().parse_file(path)?;
        eprintln!("=== {path}");
        for (spath, sets) in &doc.view_filter_sets {
            for vf in sets {
                for l in &vf.layers {
                    if l.name.contains("Consistency") || l.name.contains("Check") {
                        eprintln!(
                            "  {spath} vf oid={} layer={:?} num={} displayed={:?} locatable={:?}",
                            vf.oid, l.name, l.layer_number, l.displayed, l.locatable
                        );
                    }
                }
            }
        }
    }
    Ok(())
}
