//! Report what `fontcull`'s subsetter does to one face.
//!
//! `fonts::prepare_fonts` falls back to the whole program whenever the
//! subset fails or does not validate, which embeds a 18 MiB SimSun instead of
//! the few hundred glyphs a drawing letters. This probe names the step that
//! refuses: parse, plan, subset, or pdfcraft's validate.
//!
//! `cargo run -p pid-plot --example probe_font_subset -- <face.ttf> [text]`

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

use fontcull_klippa::{Plan, SubsetFlags, subset_font};
use fontcull_read_fonts::collections::IntSet;
use fontcull_skrifa::{FontRef, GlyphId, Tag};
use fontcull_write_fonts::types::NameId;

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: probe_font_subset <face.ttf> [text]");
        std::process::exit(2);
    };
    let text = args.next().unwrap_or_else(|| "PT 1001101A 阀".to_string());
    let out = args.next();
    let path = PathBuf::from(path);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
        eprintln!("read {}: {error}", path.display());
        std::process::exit(1);
    });
    println!(
        "{}: {} bytes, validate {:?}",
        path.display(),
        bytes.len(),
        pdfcraft_fonts::embedding::validate(&bytes).map(|()| "ok")
    );
    let font = match FontRef::new(&bytes) {
        Ok(font) => font,
        Err(error) => {
            eprintln!("parse: {error:?}");
            std::process::exit(1);
        }
    };
    let chars: HashSet<char> = ('\u{20}'..='\u{7e}').chain(text.chars()).collect();
    let mut unicodes: IntSet<u32> = IntSet::empty();
    for ch in &chars {
        unicodes.insert(u32::from(*ch));
    }
    let empty_gids: IntSet<GlyphId> = IntSet::empty();
    let empty_tags: IntSet<Tag> = [Tag::new(b"EBDT"), Tag::new(b"EBLC"), Tag::new(b"EBSC")]
        .into_iter()
        .collect();
    let name_ids: IntSet<NameId> = IntSet::all();
    let langs: IntSet<u16> = IntSet::all();
    let layout_scripts: IntSet<Tag> = IntSet::all();
    let layout_features: IntSet<Tag> =
        IntSet::from_iter(fontcull_klippa::DEFAULT_LAYOUT_FEATURES.iter().copied());
    let plan = Plan::new(
        &empty_gids,
        &unicodes,
        &font,
        SubsetFlags::default(),
        &empty_tags,
        &layout_scripts,
        &layout_features,
        &name_ids,
        &langs,
    );
    match subset_font(&font, &plan) {
        Ok(subset) => {
            println!(
                "subset: {} bytes, validate {:?}",
                subset.len(),
                pdfcraft_fonts::embedding::validate(&subset).map(|()| "ok")
            );
            println!(
                "pdfcraft names: {:?}",
                pdfcraft_fonts::embedding::names(&subset)
            );
            let missing: String = text
                .chars()
                .filter(|ch| {
                    let mut buf = [0u8; 4];
                    !pdfcraft_fonts::embedding::covers(&subset, ch.encode_utf8(&mut buf))
                })
                .collect();
            println!("uncovered characters: {missing:?}");
            if let Some(out) = out {
                std::fs::write(&out, &subset)
                    .unwrap_or_else(|error| panic!("write {out}: {error}"));
                println!("wrote {out}");
            }
        }
        Err(error) => println!("subset failed: {error:?}"),
    }
}
