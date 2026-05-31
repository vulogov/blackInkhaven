//! `typst::World` implementation for the in-process compiler (1.2.5+).
//!
//! Phase 2+3+4 of the typst-as-library plan. Provides everything
//! `typst::compile()` needs to evaluate + lay out a Typst document
//! without spawning the external `typst` binary:
//!
//! * **Sources**: loaded on demand from the project's filesystem.
//!   When the user has `typst_compile.packages_enabled = true` and
//!   the compiler asks for an `@preview/<pkg>` import, we lean on
//!   `typst-kit`'s `PackageStorage` to fetch + unpack from
//!   `packages.typst.org` (cached on disk, see `package_cache_path`
//!   in the typst-kit defaults).
//! * **Fonts**: discovered via `typst-kit`'s `FontSearcher`. By
//!   default we ship the embedded Computer Modern + Linux Libertine
//!   set AND search system fonts; either can be disabled via the
//!   HJSON `typst_compile.bundle_fonts` / `use_system_fonts`
//!   knobs.
//! * **Library**: the standard typst stdlib (`typst-library`),
//!   built once and cached behind `LazyHash`.
//! * **Today**: the wall-clock date in the system's local timezone.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use typst::diag::{FileError, FileResult, PackageError};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// Runtime knobs the World cares about. Built from
/// `crate::config::TypstCompileConfig` by `WorldSettings::from_cfg`.
#[derive(Debug, Clone)]
pub struct WorldSettings {
    pub bundle_fonts: bool,
    pub use_system_fonts: bool,
    pub packages_enabled: bool,
}

impl WorldSettings {
    pub fn from_cfg(cfg: &crate::config::TypstCompileConfig) -> Self {
        Self {
            bundle_fonts: cfg.bundle_fonts,
            use_system_fonts: cfg.use_system_fonts,
            packages_enabled: cfg.packages_enabled,
        }
    }
}

/// Font cache keyed by (bundle_fonts, use_system_fonts). Two
/// settings combinations are common in one session — the editor
/// runs the in-process engine with the user's config, and an
/// `#[ignore]` smoke test elsewhere wants defaults. Caching by
/// the bool pair avoids re-searching the system for each.
static FONT_CACHE: OnceLock<Mutex<HashMap<(bool, bool), Arc<LoadedFonts>>>> = OnceLock::new();

struct LoadedFonts {
    book: LazyHash<FontBook>,
    fonts: Vec<typst_kit::fonts::FontSlot>,
}

fn loaded_fonts(settings: &WorldSettings) -> Arc<LoadedFonts> {
    let key = (settings.bundle_fonts, settings.use_system_fonts);
    let cache = FONT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    {
        // 1.2.15+ Phase S.5 — recover from a
        // poisoned mutex via `into_inner()` so a
        // panic in a previous font-load attempt
        // doesn't take down every subsequent
        // typst compile.  The cache's invariants
        // (HashMap<key, Arc<LoadedFonts>>) survive
        // partial mutation cleanly; nothing
        // user-visible is corrupted.
        if let Some(hit) = cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&key)
            .cloned()
        {
            return hit;
        }
    }
    let mut searcher = typst_kit::fonts::FontSearcher::new();
    searcher.include_system_fonts(settings.use_system_fonts);
    searcher.include_embedded_fonts(settings.bundle_fonts);
    let kit = searcher.search();
    let loaded = Arc::new(LoadedFonts {
        book: LazyHash::new(kit.book),
        fonts: kit.fonts,
    });
    cache
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(key, loaded.clone());
    loaded
}

/// Lazily-built stdlib instance. Shared across every World in this
/// process — the stdlib is immutable and Comemo caches keyed off
/// `&Library` benefit from the pointer staying stable.
static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();

fn library() -> &'static LazyHash<Library> {
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

/// Lazily-built package storage. Same singleton model as the
/// font cache — the cache directory + downloader are heavy enough
/// (TLS init, dirs lookup) that re-creating per compile would
/// stutter the spinner.
static PACKAGE_STORAGE: OnceLock<typst_kit::package::PackageStorage> = OnceLock::new();

fn package_storage() -> &'static typst_kit::package::PackageStorage {
    PACKAGE_STORAGE.get_or_init(|| {
        let downloader = typst_kit::download::Downloader::new(concat!(
            "inkhaven/",
            env!("CARGO_PKG_VERSION"),
        ));
        typst_kit::package::PackageStorage::new(None, None, downloader)
    })
}

/// Inkhaven's `typst::World` implementation.
pub struct InkhavenWorld {
    root: PathBuf,
    main: FileId,
    sources: Mutex<HashMap<FileId, Source>>,
    today: Option<Datetime>,
    settings: WorldSettings,
    fonts: Arc<LoadedFonts>,
    /// Optional in-memory body of the `main` source. When set, the
    /// World short-circuits `source(main)` to return this directly
    /// instead of reading from disk — used by `check_semantic` to
    /// avoid writing a tempfile on every editor idle / save check.
    main_override: Option<String>,
}

impl InkhavenWorld {
    /// Build a World rooted at `project_root` whose entry document
    /// is `main_typ`. `settings` controls fonts + package fetch.
    pub fn new(
        project_root: &Path,
        main_typ: &Path,
        settings: WorldSettings,
    ) -> Result<Self, String> {
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
        let fonts = loaded_fonts(&settings);
        Ok(Self {
            root,
            main,
            sources: Mutex::new(HashMap::new()),
            today: now_local(),
            settings,
            fonts,
            main_override: None,
        })
    }

    /// Build a World whose `main` source comes from an in-memory
    /// buffer rather than disk. Used for "check this paragraph
    /// without persisting it" flows (semantic diagnostics on
    /// every idle / save). The synthetic `main` lives at
    /// `<root>/__main__.typ`; any path is fine since the World
    /// intercepts reads of that id before they hit the
    /// filesystem.
    pub fn in_memory(root: PathBuf, body: String, settings: WorldSettings) -> Self {
        let main = FileId::new(None, VirtualPath::new("__main__.typ"));
        let fonts = loaded_fonts(&settings);
        Self {
            root,
            main,
            sources: Mutex::new(HashMap::new()),
            today: now_local(),
            settings,
            fonts,
            main_override: Some(body),
        }
    }

    /// Resolve a `FileId` to a concrete on-disk path. Handles
    /// both project-local files (rooted at `self.root`) and
    /// package-scoped imports (fetched via `package_storage` when
    /// `packages_enabled` is set; rejected otherwise).
    fn resolve_disk_path(&self, id: FileId) -> FileResult<PathBuf> {
        if let Some(pkg) = id.package() {
            return self.resolve_package_path(pkg, id);
        }
        id.vpath()
            .resolve(&self.root)
            .ok_or_else(|| {
                FileError::NotFound(PathBuf::from(id.vpath().as_rooted_path()))
            })
    }

    /// Fetch (or read from cache) the package and join the `id`'s
    /// vpath onto its on-disk root.
    fn resolve_package_path(
        &self,
        pkg: &PackageSpec,
        id: FileId,
    ) -> FileResult<PathBuf> {
        if !self.settings.packages_enabled {
            return Err(FileError::Package(PackageError::Other(Some(
                typst::ecow::eco_format!(
                    "package fetching is disabled \
                     (typst_compile.packages_enabled = false)"
                ),
            ))));
        }
        let storage = package_storage();
        let mut noop = NoProgress;
        let pkg_root = storage
            .prepare_package(pkg, &mut noop)
            .map_err(FileError::Package)?;
        id.vpath()
            .resolve(&pkg_root)
            .ok_or_else(|| {
                FileError::NotFound(PathBuf::from(id.vpath().as_rooted_path()))
            })
    }

    fn load_source(&self, id: FileId) -> FileResult<Source> {
        let path = self.resolve_disk_path(id)?;
        let bytes =
            std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        let text = String::from_utf8(bytes).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text))
    }
}

impl World for InkhavenWorld {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.fonts.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            if let Some(text) = self.main_override.as_ref() {
                return Ok(Source::new(id, text.clone()));
            }
        }
        // 1.2.15+ Phase S.5 — poisoned-lock recovery
        // for the per-World source cache.  Same shape
        // as `loaded_fonts`; the cache's invariants
        // tolerate partial state cleanly.
        if let Some(src) = self
            .sources
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id)
            .cloned()
        {
            return Ok(src);
        }
        let loaded = self.load_source(id)?;
        self.sources
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id, loaded.clone());
        Ok(loaded)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve_disk_path(id)?;
        let bytes =
            std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(bytes))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.fonts.get(index)?.get()
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

/// `Progress` impl that suppresses everything — the TUI spinner
/// already covers "the compile is running"; per-byte download
/// chatter would corrupt the alternate-screen back buffer.
struct NoProgress;

impl typst_kit::download::Progress for NoProgress {
    fn print_start(&mut self) {}
    fn print_progress(&mut self, _: &typst_kit::download::DownloadState) {}
    fn print_finish(&mut self, _: &typst_kit::download::DownloadState) {}
}
