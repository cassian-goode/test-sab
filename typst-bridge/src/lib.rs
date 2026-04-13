use std::sync::RwLock;

use comemo::Track;
use serde::Serialize;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::ecow::EcoString;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::{Page, PagedDocument};
use typst_library::layout::Point;
use typst_library::model::LateLinkResolver;
use typst_utils::hash128;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct CompileDocumentOutput {
    page_count: usize,
}

#[derive(Serialize)]
struct ChangedPageOutput {
    index: usize,
    svg: String,
}

#[derive(Serialize)]
struct RenderChangedPagesOutput {
    changed_pages: Vec<ChangedPageOutput>,
    cache_hit_count: usize,
    cache_miss_count: usize,
}

#[derive(Clone)]
struct CachedPage {
    hash: u128
}

#[wasm_bindgen]
pub struct CompilerSession {
    world: SimpleWorld,
    document: Option<PagedDocument>,
    page_cache: Vec<Option<CachedPage>>,
}

#[wasm_bindgen]
impl CompilerSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Keep the expensive, mostly-static compiler state alive for the whole session.
        Self {
            world: SimpleWorld::new("#set text(size: 14pt)\n"),
            document: None,
            page_cache: Vec::new(),
        }
    }

    #[wasm_bindgen]
    pub fn compile_document(&mut self, source: &str) -> Result<JsValue, JsValue> {
        self.world.set_main_source(source);

        // Compile once and store the final paged document in the session.
        let document = compile_document_internal(&self.world)
            .map_err(|err| JsValue::from_str(&err))?;
        let page_count = document.pages().len();

        self.document = Some(document);

        // Keep the cache aligned to the current page count.
        self.page_cache.truncate(page_count);
        if self.page_cache.len() < page_count {
            self.page_cache.resize_with(page_count, || None);
        }

        serde_wasm_bindgen::to_value(&CompileDocumentOutput { page_count })
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    #[wasm_bindgen]
    pub fn render_changed_pages(&mut self) -> Result<JsValue, JsValue> {
        let page_count = self
            .document
            .as_ref()
            .ok_or_else(|| JsValue::from_str("no compiled document"))?
            .pages()
            .len();

        if self.page_cache.len() > page_count {
            self.page_cache.truncate(page_count);
        } else if self.page_cache.len() < page_count {
            self.page_cache.resize_with(page_count, || None);
        }

        let mut changed_pages = Vec::new();
        let mut cache_hit_count = 0usize;
        let mut cache_miss_count = 0usize;

        // Change: Hash every page, but only render and return pages whose hash changed.
        for index in 0..page_count {
            let (page_hash, maybe_svg) = {
                let document = self
                    .document
                    .as_ref()
                    .ok_or_else(|| JsValue::from_str("no compiled document"))?;

                let page = document
                    .pages()
                    .get(index)
                    .ok_or_else(|| JsValue::from_str("page index out of range"))?;

                let page_hash = hash128(page);

                let cache_hit = self
                    .page_cache
                    .get(index)
                    .and_then(|entry| entry.as_ref())
                    .is_some_and(|cached| cached.hash == page_hash);

                if cache_hit {
                    (page_hash, None)
                } else {
                    let svg = render_page_svg_in_bundle(document, page)
                        .map_err(|err| JsValue::from_str(&err))?;
                    (page_hash, Some(svg))
                }
            };

            match maybe_svg {
                Some(svg) => {
                    self.page_cache[index] = Some(CachedPage {
                        hash: page_hash
                    });

                    changed_pages.push(ChangedPageOutput { index, svg });
                    cache_miss_count += 1;
                }
                None => {
                    cache_hit_count += 1;
                }
            }
        }

        serde_wasm_bindgen::to_value(&RenderChangedPagesOutput {
            changed_pages,
            cache_hit_count,
            cache_miss_count,
        })
        .map_err(|err| JsValue::from_str(&err.to_string()))
    }
}

fn compile_document_internal(world: &SimpleWorld) -> Result<PagedDocument, String> {
    let warned = typst::compile::<PagedDocument>(world);

    match warned.output {
        Ok(document) => Ok(document),
        Err(errors) => Err(format_diagnostics(&errors)),
    }
}

fn render_page_svg_in_bundle(document: &PagedDocument, page: &Page) -> Result<String, String> {
    // We are not emitting extra anchors in this preview experiment.
    let anchors: Vec<(Point, EcoString)> = Vec::new();

    let resolver = make_bundle_link_resolver(document);

    let svg = typst_svg::svg_in_bundle(page, &anchors, resolver.track());

    Ok(svg)
}

fn make_bundle_link_resolver(document: &PagedDocument) -> LateLinkResolver<'_> {
    // Single-document preview mode: resolve links relative to the current document.
    LateLinkResolver::new(None, document.introspector().as_ref())
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
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_DOC: &str = r#"
#set text(size: 14pt)

Hello, Typst!
"#;

    #[test]
    fn compiles_document() {
        let world = SimpleWorld::new(SIMPLE_DOC);
        let document = compile_document_internal(&world).expect("expected document to compile");
        assert_eq!(document.pages().len(), 1);
    }

    #[test]
    fn reuses_same_world_identity() {
        let world = SimpleWorld::new("Hello");
        let first_id = world.main();

        world.set_main_source("Hello again");
        let second_id = world.main();

        assert_eq!(first_id, second_id);
    }

    #[test]
    fn page_hash_changes_when_page_changes() {
        let world_a = SimpleWorld::new("#set text(size: 14pt)\n\nHello");
        let doc_a = compile_document_internal(&world_a).expect("doc A should compile");

        let world_b = SimpleWorld::new("#set text(size: 14pt)\n\nHello!");
        let doc_b = compile_document_internal(&world_b).expect("doc B should compile");

        let hash_a = hash128(&doc_a.pages()[0]);
        let hash_b = hash128(&doc_b.pages()[0]);

        assert_ne!(hash_a, hash_b);
    }
}