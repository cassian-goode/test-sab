use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::layout::{Abs};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use wasm_bindgen::prelude::*;

pub fn compile_to_svg(source: &str) -> Result<String, String> {
    let world = SimpleWorld::new(source);

    let warned = typst::compile(&world);

    match warned.output {
        Ok(document) => Ok(typst_svg::svg_merged(&document, Abs::zero())),
        Err(errors) => Err(format_diagnostics(&errors)),
    }
}

#[wasm_bindgen]
pub fn compile_to_svg_wasm(source: &str) -> Result<String, JsValue> {
    compile_to_svg(source).map_err(|err| JsValue::from_str(&err))
}

#[wasm_bindgen]
pub fn embedded_font_count_wasm() -> usize {
    let (_, fonts) = load_embedded_fonts();
    fonts.len()
}

#[wasm_bindgen]
pub fn compile_debug_wasm(source: &str) -> String {
    let world = SimpleWorld::new(source);

    let mut out = String::new();
    out.push_str(&format!("fonts loaded: {}\n", world.fonts.len()));

    match compile_to_svg(source) {
        Ok(svg) => {
            out.push_str("compile output: ok\n");
            out.push_str(&format!("svg length: {}\n", svg.len()));
            out.push_str("svg starts with:\n");
            out.push_str(&svg.chars().take(200).collect::<String>());
        }
        Err(err) => {
            out.push_str("compile output: err\n");
            out.push_str(&err);
        }
    }

    out
}

fn format_diagnostics(diags: &[SourceDiagnostic]) -> String {
    diags
        .iter()
        .map(|diag| diag.message.to_string())
        .collect::<Vec<_>>()
        .join("\n\n")
}

struct SimpleWorld {
    source: Source,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl SimpleWorld {
    fn new(main_source: &str) -> Self {
        let (book, fonts) = load_embedded_fonts();

        Self {
            source: Source::detached(main_source.to_owned()),
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
        }
    }
}

fn load_embedded_fonts() -> (FontBook, Vec<Font>) {
    let mut book = FontBook::new();
    let mut fonts = Vec::new();

    for data in typst_assets::fonts() {
        let bytes = Bytes::new(data);

        for font in Font::iter(bytes) {
            book.push(font.info().clone());
            fonts.push(font);
        }
    }

    (book, fonts)
}

impl World for SimpleWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }

    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::AccessDenied)
    }

    fn font(&self, id: usize) -> Option<Font> {
        self.fonts.get(id).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_DOC: &str = r#"
#set page(width: 200pt, height: 120pt, margin: 12pt)
#set text(size: 14pt)

Hello, Typst!
"#;

    #[test]
    fn compiles_simple_doc_to_svg() {
        let svg = compile_to_svg(SIMPLE_DOC).expect("expected Typst source to compile to SVG");
        assert!(svg.contains("<svg"));
    }
}