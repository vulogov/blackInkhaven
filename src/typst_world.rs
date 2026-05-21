//! `typst::World` implementation for the in-process compiler (1.2.5+).
//!
//! Phase 2+3+4 of the typst-as-library plan. Provides everything
//! `typst::compile()` needs to evaluate + lay out a Typst document
//! without spawning the external `typst` binary:
//!
//! * **Sources**: loaded on demand from the project's filesystem
//!   (root = `.scriv`-style project root, same place
//!   `inkhaven init` placed everything). `@preview/<pkg>` package
//!   imports are NOT yet resolved — the network-fetching path is a
//!   future addition; for now an `@preview/...` import returns a
//!   `FileError::Package(PackageError::NotFound(...))` and the user
//!   sees that diagnostic next to the import line.
//! * **Fonts**: discovered via `typst-kit`'s `FontSearcher`. System
//!   fonts only — we explicitly skip the `embed-fonts` feature
//!   (would add ~10 MB to the binary for Computer Modern). If the
//!   user's system has no fonts at all, `typst::compile()` will
//!   surface a `font not found` diagnostic; we don't try to paper
//!   over it.
//! * **Library**: the standard typst stdlib (`typst-library`),
//!   built once and cached behind `LazyHash`.
//! * **Today**: the wall-clock date in the system's local timezone.
//!
//! The World is intentionally simple — caching the source list is
//! enough for the assemble-then-compile flow inkhaven uses today.
//! Phase 5 (live preview) will want a more sophisticated invalidation
//! story so re-compiles only re-parse the source that changed; for
//! now every `Ctrl+B B` / `Ctrl+B O` builds a fresh World.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// Global font cache. Searching the system for fonts is a one-time
/// startup cost (~50–500 ms depending on platform); we do it lazily
/// the first time a World is built and reuse the result across
/// every subsequent compile.
static FONTS: OnceLock<LoadedFonts> = OnceLock::new();

struct LoadedFonts {
    book: LazyHash<FontBook>,
    fonts: Vec<typst_kit::fonts::FontSlot>,
}

fn loaded_fonts() -> &'static LoadedFonts {
    FONTS.get_or_init(|| {
        let kit = typst_kit::fonts::FontSearcher::new()
            .include_system_fonts(true)
            .search();
        LoadedFonts {
            book: LazyHash::new(kit.book),
            fonts: kit.fonts,
        }
    })
}

/// Lazily-built stdlib instance. Shared across every World in this
/// process — the stdlib is immutable and Comemo caches keyed off
/// `&Library` benefit from the pointer staying stable.
static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();

fn library() -> &'static LazyHash<Library> {
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

/// Inkhaven's `typst::World` implementation.
///
/// Lifetime: build a fresh one per compile. The Mutex-guarded
/// `sources` cache is private to one World instance — if you reuse
/// a World across compiles you'd want explicit invalidation. The
/// canonical pattern in this codebase is the call sites in
/// `typst_inprocess` which build, compile once, and drop.
pub struct InkhavenWorld {
    /// Absolute project root. All `VirtualPath`s the compiler asks
    /// about are resolved relative to this.
    root: PathBuf,
    /// `FileId` of the entry source the compiler should treat as
    /// the document root.
    main: FileId,
    /// Cached `Source` instances keyed by `FileId`. Built lazily
    /// the first time `source(id)` fires for a given id.
    sources: Mutex<HashMap<FileId, Source>>,
    /// Wall-clock at World creation. Snapshotted once so a
    /// long-running compile sees a stable `today`.
    today: Option<Datetime>,
}

impl InkhavenWorld {
    /// Build a World rooted at `project_root` whose entry document
    /// is `main_typ` (must live somewhere under `project_root`).
    ///
    /// Errors only if `main_typ` is outside the project root —
    /// font / source loading is lazy and surfaces through the
    /// `FileResult` paths the compiler already handles.
    pub fn new(project_root: &Path, main_typ: &Path) -> Result<Self, String> {
        let root = project_root.to_path_buf();
        let main_abs = main_typ
            .canonicalize()
            .unwrap_or_else(|_| main_typ.to_path_buf());
        let root_abs = root.canonicalize().unwrap_or_else(|_| root.clone());
        let main_rel = main_abs
            .strip_prefix(&root_abs)
            .map_err(|_| {
                format!(
                    "main `.typ` ({}) is not inside project root ({})",
                    main_typ.display(),
                    project_root.display(),
                )
            })?
            .to_path_buf();
        let vpath = VirtualPath::new(main_rel);
        let main = FileId::new(None, vpath);
        Ok(Self {
            root,
            main,
            sources: Mutex::new(HashMap::new()),
            today: now_local(),
        })
    }

    /// Resolve a `FileId` to a concrete on-disk absolute path. Returns
    /// `None` for package-scoped ids (`@preview/<pkg>`) — those need
    /// the package downloader we haven't pulled in yet.
    fn resolve_disk_path(&self, id: FileId) -> Option<PathBuf> {
        if id.package().is_some() {
            return None;
        }
        Some(id.vpath().resolve(&self.root)?)
    }

    fn load_source(&self, id: FileId) -> FileResult<Source> {
        if id.package().is_some() {
            return Err(FileError::Package(typst::diag::PackageError::NotFound(
                id.package().unwrap().clone(),
            )));
        }
        let path = self
            .resolve_disk_path(id)
            .ok_or_else(|| FileError::NotFound(PathBuf::from(id.vpath().as_rooted_path())))?;
        let bytes = std::fs::read(&path).map_err(|err| {
            FileError::from_io(err, &path)
        })?;
        let text = String::from_utf8(bytes)
            .map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text))
    }
}

impl World for InkhavenWorld {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &loaded_fonts().book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(src) = self.sources.lock().unwrap().get(&id).cloned() {
            return Ok(src);
        }
        let loaded = self.load_source(id)?;
        self.sources.lock().unwrap().insert(id, loaded.clone());
        Ok(loaded)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self
            .resolve_disk_path(id)
            .ok_or_else(|| {
                if let Some(pkg) = id.package() {
                    FileError::Package(typst::diag::PackageError::NotFound(pkg.clone()))
                } else {
                    FileError::NotFound(PathBuf::from(id.vpath().as_rooted_path()))
                }
            })?;
        let bytes = std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(bytes))
    }

    fn font(&self, index: usize) -> Option<Font> {
        loaded_fonts().fonts.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        self.today
    }
}

/// Best-effort wall-clock for `World::today`. Matches the local
/// timezone, same convention the external `typst` CLI uses.
fn now_local() -> Option<Datetime> {
    use chrono::Datelike;
    let now = chrono::Local::now();
    Datetime::from_ymd(now.year(), now.month() as u8, now.day() as u8)
}
