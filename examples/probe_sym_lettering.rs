//! Does `I-P Converter.sym`'s own lettering resolve through the public API?
use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: probe_sym_lettering <sym>");
    let geometry = read_symbol_geometry(std::path::Path::new(&path)).expect("reads");
    for styled in &geometry.primitives {
        if let SymbolPrimitive::Text { text, .. } = &styled.primitive {
            println!("text={text:?} lettering={:?}", styled.lettering);
        }
    }
}
