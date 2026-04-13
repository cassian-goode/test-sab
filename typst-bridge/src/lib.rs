use std::sync::RwLock;

use serde::Serialize;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct CompileOutput {
    pages: Vec<String>,
}

#[wasm_bindgen]
pub struct CompilerSession {
    world: SimpleWorld,
}

#[wasm_bindgen]
impl CompilerSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Change: Build the expensive, mostly-static compiler state once.
        // Fonts, font book, library, and the main file identity all live for the
        // whole session instead of being rebuilt every compile.
        Self {
            world: SimpleWorld::new(
                "#set page(width: 300pt, height: 180pt, margin: 16pt)\n",
            ),
        }
    }

    pub fn compile(&mut self, source: &str) -> Result<JsValue, JsValue> {
        // Change: Reuse the same world and only replace the main source text.
        self.world.set_main_source(source);

        let pages = compile_pages(&self.world).map_err(|err| JsValue::from_str(&err))?;

        serde_wasm_bindgen::to_value(&CompileOutput { pages })
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }
}

fn compile_pages(world: &SimpleWorld) -> Result<Vec<String>, String> {
    let warned = typst::compile::<PagedDocument>(world);

    match warned.output {
        Ok(document) => {
            // Change: Render each page separately so the UI can display distinct
            // sheets instead of one giant merged SVG.
            let pages = document.pages().iter().map(typst_svg::svg).collect::<Vec<_>>();
            Ok(pages)
        }
        Err(errors) => Err(format_diagnostics(&errors)),
    }
}

fn format_diagnostics(diags: &[SourceDiagnostic]) -> String {
    diags
        .iter()
        .map(|diag| diag.message.to_string())
        .collect::<Vec<_>>()
        .join("\n\n")
}

struct SimpleWorld {
    main_id: FileId,
    source: RwLock<Source>,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl SimpleWorld {
    fn new(initial_source: &str) -> Self {
        let (book, fonts) = load_embedded_fonts();

        // Change: Create one stable main file id up front and keep it forever.
        // This mirrors the idea of a persistent "main file" inside an editor session.
        let main_id = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("main.typ").unwrap(),
        )
        .intern();

        let source = Source::new(main_id, initial_source.to_owned());

        Self {
            main_id,
            source: RwLock::new(source),
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
        }
    }

    fn set_main_source(&self, new_source: &str) {
        // Change: Update the existing source in place instead of constructing a
        // brand new world on every compile.
        let mut source = self.source.write().unwrap();
        source.replace(new_source);
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
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.source.read().unwrap().clone())
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
        // Still intentionally ignored for now.
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
    fn compiles_simple_doc_to_separate_pages() {
        let world = SimpleWorld::new(SIMPLE_DOC);
        let pages = compile_pages(&world).expect("expected Typst source to compile to SVG pages");

        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("<svg"));
    }

    #[test]
    fn reuses_same_world_for_multiple_compiles() {
        let world = SimpleWorld::new("Hello");
        let first_id = world.main();

        world.set_main_source("Hello again");
        let second_id = world.main();

        assert_eq!(first_id, second_id);
    }
}