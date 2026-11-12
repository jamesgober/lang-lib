use std::borrow::Cow;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use arc_swap::{ArcSwap, Guard};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::LangError;
use crate::intern::{Text, intern, intern_unloaded_locale};
use crate::loader;
#[cfg(feature = "registry")]
use crate::{
    change::{ChangeKind, LangChangeEvent},
    registry::emit,
};

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------
//
// Translation values use a different storage strategy depending on whether
// the `hot-reload` feature is enabled:
//
// - **Default builds** store values as `Text`. Values from the first load
//   of a locale (and reloaded values while the interner's reload budget
//   lasts) are interned `&'static str` references, so hit-path returns are
//   `Cow::Borrowed(&'static str)`: pure pointer copies, no atomics, no
//   allocation. Reloaded values past the budget are `Arc<str>`, freed on
//   the next reload or unload, and a hit on one returns an owned copy. This
//   bounds the interner's growth under repeated `Lang::load` calls.
//
// - **`hot-reload` builds** store values as `Arc<str>`. Hit-path returns are
//   `Cow::Owned(arc.to_string())` — one allocation per call, but reloading
//   a locale drops the old `Arc<str>` instances cleanly (no leak). The
//   `Lang::translate_arc` opt-in returns `Arc<str>` directly for callers
//   that want zero-allocation reads under hot-reload at the cost of
//   refcount contention under same-key high concurrency.

/// Value-storage type used inside the per-locale map.
///
/// `Text` in default builds (interned where the budget allows, so most hits
/// are zero-alloc); `Arc<str>` in `hot-reload` builds (reclaims on reload).
#[cfg(not(feature = "hot-reload"))]
pub(crate) type StoredValue = Text;

/// Value-storage type used inside the per-locale map.
///
/// `Text` in default builds (interned where the budget allows, so most hits
/// are zero-alloc); `Arc<str>` in `hot-reload` builds (reclaims on reload).
#[cfg(feature = "hot-reload")]
pub(crate) type StoredValue = Arc<str>;

type LocaleMap = FxHashMap<Text, StoredValue>;

struct LangState {
    path: &'static str,
    active: &'static str,
    fallbacks: Arc<[&'static str]>,
    locales: FxHashMap<&'static str, Arc<LocaleMap>>,
}

impl LangState {
    fn initial() -> Self {
        Self {
            path: "locales",
            active: "en",
            fallbacks: Arc::from(["en"].as_slice()),
            locales: FxHashMap::default(),
        }
    }
}

impl Clone for LangState {
    fn clone(&self) -> Self {
        Self {
            path: self.path,
            active: self.active,
            fallbacks: Arc::clone(&self.fallbacks),
            locales: self.locales.clone(),
        }
    }
}

static STATE: OnceLock<ArcSwap<LangState>> = OnceLock::new();
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Locale names that have loaded successfully at least once in this
/// process. A later load of one of these is a reload, which interns new
/// strings only within the interner's reload budget.
static EVER_LOADED: OnceLock<Mutex<FxHashSet<&'static str>>> = OnceLock::new();

fn ever_loaded() -> std::sync::MutexGuard<'static, FxHashSet<&'static str>> {
    EVER_LOADED
        .get_or_init(|| Mutex::new(FxHashSet::default()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// The locale name used for names that are rejected or over the cap. It is
/// never a valid locale, so it is never loaded and every lookup through it
/// goes straight to the fallback chain.
const UNKNOWN_LOCALE: &str = "";

/// Maps a caller-supplied locale name to a `&'static str` without letting
/// outside input grow the interner without bound.
///
/// A loaded locale, the active locale, or a fallback reuses the stored
/// name. Any other name is interned only if it is shaped like a locale and
/// the cap on unloaded names has room; otherwise it maps to
/// [`UNKNOWN_LOCALE`].
fn resolve_locale(name: &str) -> &'static str {
    let state = snapshot();
    if let Some((&key, _)) = state.locales.get_key_value(name) {
        return key;
    }
    if state.active == name {
        return state.active;
    }
    if let Some(&fb) = state.fallbacks.iter().find(|&&fb| fb == name) {
        return fb;
    }
    drop(state);
    if !loader::is_locale_name(name) {
        return UNKNOWN_LOCALE;
    }
    intern_unloaded_locale(name).unwrap_or(UNKNOWN_LOCALE)
}

/// Returns the stored name of a loaded locale.
#[cfg(feature = "hot-reload")]
pub(crate) fn loaded_locale_key(name: &str) -> Option<&'static str> {
    snapshot().locales.get_key_value(name).map(|(&key, _)| key)
}

fn load_into(locale: &str, path: &str) -> Result<(), LangError> {
    let first_load = !ever_loaded().contains(locale);
    let map = loader::load_file(path, locale, first_load)?;
    let interned_locale = intern(locale);
    let _ = ever_loaded().insert(interned_locale);
    let arc_map: Arc<LocaleMap> = Arc::new(map);
    #[cfg(feature = "registry")]
    let was_present = snapshot().locales.contains_key(interned_locale);
    with_write(|state| {
        let _ = state.locales.insert(interned_locale, Arc::clone(&arc_map));
    });
    #[cfg(feature = "registry")]
    emit(LangChangeEvent {
        locale: interned_locale,
        kind: if was_present {
            ChangeKind::Reloaded
        } else {
            ChangeKind::Loaded
        },
    });
    Ok(())
}

fn state() -> &'static ArcSwap<LangState> {
    STATE.get_or_init(|| ArcSwap::new(Arc::new(LangState::initial())))
}

fn snapshot() -> Guard<Arc<LangState>> {
    state().load()
}

fn with_write<F>(mutate: F)
where
    F: FnOnce(&mut LangState),
{
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let current = state().load_full();
    let mut next = (*current).clone();
    mutate(&mut next);
    state().store(Arc::new(next));
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The main entry point for configuring and querying the localization system.
///
/// `Lang` manages process-global state behind a lock-free [`arc_swap::ArcSwap`]
/// snapshot. Configure it once during startup, load the locales your
/// application needs, and then use [`t!`](crate::t) or [`Lang::translate`]
/// wherever translated text is needed.
///
/// Concurrent calls to [`Lang::translate`] do not contend on any lock. Write
/// operations (`set_*`, [`Lang::load`], [`Lang::unload`]) briefly serialize
/// against each other but never block readers.
pub struct Lang;

/// A lightweight, request-scoped translation helper.
///
/// `Translator` stores a locale identifier and forwards lookups to the global
/// [`Lang`] store without mutating the process-wide active locale.
/// This makes it a good fit for web handlers, jobs, and other code paths
/// where locale is part of the input rather than part of global application
/// state.
///
/// Cloning a `Translator` is cheap — the locale is held as a `&'static str`,
/// so the operation is a pointer copy and no allocation occurs.
///
/// # Examples
///
/// ```rust,no_run
/// use lang_lib::{Lang, Translator};
///
/// Lang::load_from("en", "tests/fixtures/locales").unwrap();
/// let translator = Translator::new("en");
///
/// let title = translator.translate_with_fallback("welcome", "Welcome");
/// assert_eq!(title, "Welcome");
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Translator {
    locale: &'static str,
}

impl Translator {
    /// Creates a translator bound to a specific locale.
    ///
    /// The locale may come from outside input, such as a path segment or a
    /// cookie. A loaded locale, the active locale, or a fallback locale is
    /// used as is. Any other name is kept only if it is shaped like a locale
    /// identifier (see [`Lang::load`]) and fewer than 256 distinct unloaded
    /// names have been kept so far; otherwise the translator uses the empty
    /// locale, which is never loaded, so every lookup goes to the fallback
    /// chain. This keeps memory bounded however many distinct names arrive.
    ///
    /// For request handling, prefer resolving the locale against a fixed
    /// list first with [`crate::resolve_accept_language`].
    #[must_use]
    pub fn new(locale: impl AsRef<str>) -> Self {
        Self {
            locale: resolve_locale(locale.as_ref()),
        }
    }

    /// Returns the locale used by this translator.
    ///
    /// This is the empty string when [`Translator::new`] rejected the name.
    #[must_use]
    pub fn locale(&self) -> &'static str {
        self.locale
    }

    /// Translates a key using this translator's locale.
    ///
    /// The returned value borrows directly into the interned translation
    /// store on the hit path and into `key` on the complete-miss path. Both
    /// outcomes are zero-allocation.
    #[must_use]
    pub fn translate<'a>(&self, key: &'a str) -> Cow<'a, str> {
        Lang::translate(key, Some(self.locale), None)
    }

    /// Translates a key using this translator's locale and an inline fallback.
    ///
    /// The returned value borrows directly into the interned translation
    /// store on the hit path, into `fallback` when the lookup misses, and
    /// into `key` only if no fallback resolves either. All three outcomes
    /// are zero-allocation.
    #[must_use]
    pub fn translate_with_fallback<'a>(&self, key: &'a str, fallback: &'a str) -> Cow<'a, str> {
        Lang::translate(key, Some(self.locale), Some(fallback))
    }

    /// Translates a key, returning the value as an `Arc<str>`.
    ///
    /// Available when the `hot-reload` feature is enabled. Forwards to
    /// [`Lang::translate_arc`] with this translator's locale. See that
    /// method's documentation for the contention trade-off.
    #[cfg(feature = "hot-reload")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot-reload")))]
    #[must_use]
    pub fn translate_arc(&self, key: &str) -> Arc<str> {
        Lang::translate_arc(key, Some(self.locale), None)
    }

    /// Translates a key with an inline fallback, returning the value as an
    /// `Arc<str>`.
    ///
    /// Available when the `hot-reload` feature is enabled. Forwards to
    /// [`Lang::translate_arc`] with this translator's locale.
    #[cfg(feature = "hot-reload")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot-reload")))]
    #[must_use]
    pub fn translate_arc_with_fallback(&self, key: &str, fallback: &str) -> Arc<str> {
        Lang::translate_arc(key, Some(self.locale), Some(fallback))
    }
}

impl Lang {
    /// Sets the directory where language files are looked up.
    ///
    /// Defaults to `"locales"`. Call this before the first [`Lang::load`] if
    /// your project stores files elsewhere.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    /// Lang::set_path("assets/lang");
    /// ```
    pub fn set_path(path: impl AsRef<str>) {
        let interned = intern(path.as_ref());
        with_write(|state| state.path = interned);
    }

    /// Returns the current language file path.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// Lang::set_path("assets/locales");
    /// assert_eq!(Lang::path(), "assets/locales");
    /// ```
    #[must_use]
    pub fn path() -> &'static str {
        snapshot().path
    }

    /// Sets the active locale used when no locale is specified in `t!`.
    ///
    /// The locale does not need to be loaded before calling this, but
    /// translations will be empty until it is. Names follow the same rules
    /// as [`Translator::new`]: a name that is not shaped like a locale, or
    /// an unloaded name past the cap, sets the active locale to the empty
    /// string, so lookups go to the fallback chain.
    ///
    /// This method is a good fit for single-user applications, CLIs, and
    /// startup-time configuration. In request-driven servers, prefer passing
    /// an explicit locale to [`Lang::translate`] or [`t!`](crate::t) so one
    /// request does not change another request's active locale.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    /// Lang::set_locale("es");
    /// ```
    pub fn set_locale(locale: impl AsRef<str>) {
        let resolved = resolve_locale(locale.as_ref());
        with_write(|state| state.active = resolved);
    }

    /// Returns the currently active locale.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// Lang::set_locale("fr");
    /// assert_eq!(Lang::locale(), "fr");
    /// ```
    #[must_use]
    pub fn locale() -> &'static str {
        snapshot().active
    }

    /// Sets the fallback locale chain.
    ///
    /// When a key is not found in the requested locale, each fallback is
    /// checked in order. The last resort is the inline `fallback:` argument
    /// in `t!`, and if that is absent, the key itself is returned.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    /// Lang::set_fallbacks(vec!["en".to_string()]);
    /// ```
    #[allow(clippy::needless_pass_by_value, reason = "preserves 1.0.x signature")]
    pub fn set_fallbacks(chain: Vec<String>) {
        let interned: Vec<&'static str> = chain.iter().map(|s| intern(s)).collect();
        let arc: Arc<[&'static str]> = Arc::from(interned);
        with_write(|state| state.fallbacks = Arc::clone(&arc));
    }

    /// Loads a locale from disk.
    ///
    /// Reads `{path}/{locale}.toml` and stores all translations in memory.
    /// Calling this a second time for the same locale replaces the existing
    /// translations with a fresh load from disk.
    ///
    /// Locale names must be shaped like locale identifiers: 1 to 64 ASCII
    /// bytes, starting with a letter or digit, followed by letters, digits,
    /// `-`, `_`, `.`, or `@` (for example `en`, `en-US`, `pt_BR`, or
    /// `sr_RS@latin`). Anything else, including path separators, is
    /// rejected before any file access.
    ///
    /// Files larger than [`Lang::max_file_size`] (16 MiB by default) are
    /// rejected without being read.
    ///
    /// # Memory
    ///
    /// The first load of a locale keeps its keys and values for the life of
    /// the process, which is what makes default-build lookups allocation
    /// free. A later load of the same locale reuses strings that are already
    /// kept and keeps new ones only within a 1 MiB budget shared by all
    /// reloads in the process. Past the budget, new strings are freed when
    /// the locale is reloaded again or unloaded, and in the default build a
    /// hit on one of them returns an owned copy.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::InvalidLocale`] if the name is rejected,
    /// [`LangError::Io`] if the file cannot be read or is over the size
    /// limit, or [`LangError::Parse`] if the TOML is invalid. A parse error
    /// gives the line, column, and reason, never the file content.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    /// Lang::set_path("locales");
    /// Lang::load("en").unwrap();
    /// Lang::load("es").unwrap();
    /// ```
    pub fn load(locale: impl AsRef<str>) -> Result<(), LangError> {
        let path = snapshot().path;
        load_into(locale.as_ref(), path)
    }

    /// Loads a locale from a specific path, ignoring the global path setting.
    ///
    /// Useful when a project stores one locale separately from the others.
    /// Locale names follow the same validation rules as [`Lang::load`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Io`] or [`LangError::Parse`] on failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// Lang::load_from("en", "tests/fixtures/locales").unwrap();
    /// ```
    pub fn load_from(locale: impl AsRef<str>, path: &str) -> Result<(), LangError> {
        load_into(locale.as_ref(), path)
    }

    /// Sets the largest locale file, in bytes, that [`Lang::load`] and
    /// [`Lang::load_from`] will read. Defaults to 16 MiB.
    ///
    /// A larger file is rejected with [`LangError::Io`] before it is read,
    /// so a locale directory that others can write to cannot make a load
    /// exhaust memory.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use lang_lib::Lang;
    ///
    /// Lang::set_max_file_size(4 * 1024 * 1024);
    /// assert_eq!(Lang::max_file_size(), 4 * 1024 * 1024);
    /// # Lang::set_max_file_size(16 * 1024 * 1024);
    /// ```
    pub fn set_max_file_size(bytes: u64) {
        loader::set_max_file_size(bytes);
    }

    /// Returns the largest locale file, in bytes, that a load will read.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use lang_lib::Lang;
    ///
    /// assert!(Lang::max_file_size() > 0);
    /// ```
    #[must_use]
    pub fn max_file_size() -> u64 {
        loader::max_file_size()
    }

    /// Returns `true` if the locale has been loaded.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// Lang::load_from("en", "tests/fixtures/locales").unwrap();
    /// assert!(Lang::is_loaded("en"));
    /// ```
    #[must_use]
    pub fn is_loaded(locale: &str) -> bool {
        snapshot().locales.contains_key(locale)
    }

    /// Returns a sorted list of all loaded locale identifiers.
    ///
    /// Sorting keeps diagnostics and tests deterministic.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// Lang::load_from("es", "tests/fixtures/locales").unwrap();
    /// Lang::load_from("en", "tests/fixtures/locales").unwrap();
    /// assert_eq!(Lang::loaded(), vec!["en", "es"]);
    /// ```
    #[must_use]
    pub fn loaded() -> Vec<&'static str> {
        let mut locales: Vec<&'static str> = snapshot().locales.keys().copied().collect();
        locales.sort_unstable();
        locales
    }

    /// Unloads a locale and removes it from the lookup table.
    ///
    /// Unloading a locale does not change the active locale or fallback chain.
    /// If either of those still references the removed locale, translation
    /// will simply skip it.
    ///
    /// Strings kept from the locale's first load stay in the process-wide
    /// pool after unloading (see the memory notes on [`Lang::load`]);
    /// strings held past the reload budget are freed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// Lang::load_from("en", "tests/fixtures/locales").unwrap();
    /// Lang::unload("en");
    /// assert!(!Lang::is_loaded("en"));
    /// ```
    pub fn unload(locale: &str) {
        #[cfg(feature = "registry")]
        let present_key = snapshot()
            .locales
            .get_key_value(locale)
            .map(|(&key, _)| key);
        with_write(|state| {
            let _ = state.locales.remove(locale);
        });
        #[cfg(feature = "registry")]
        if let Some(key) = present_key {
            emit(LangChangeEvent {
                locale: key,
                kind: ChangeKind::Unloaded,
            });
        }
    }

    /// Creates a request-scoped [`Translator`] for the provided locale.
    ///
    /// This is a convenience wrapper around [`Translator::new`], with the
    /// same rules for names that are not loaded. It is most useful in server
    /// code where locale is resolved per request and passed through the
    /// handler stack.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// let translator = Lang::translator("es");
    /// assert_eq!(translator.locale(), "es");
    /// ```
    #[must_use]
    pub fn translator(locale: impl AsRef<str>) -> Translator {
        Translator::new(locale)
    }

    /// Registers a handler that fires whenever the translation store
    /// changes.
    ///
    /// Handlers fire inline on the thread that produced the change (the
    /// writer, or the watcher thread when `hot-reload` is enabled). The
    /// dispatch is lock-free and panic-isolating; a panic in one handler
    /// does not stop sibling handlers.
    ///
    /// Returns a [`registry_io::HandlerId`] that can be passed to
    /// [`Lang::off_change`] to deregister the handler.
    ///
    /// Available when the `registry` feature is enabled.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "registry")]
    /// # fn demo() {
    /// use lang_lib::Lang;
    ///
    /// let id = Lang::on_change(|event| {
    ///     println!("{:?} on {}", event.kind, event.locale);
    /// });
    /// let _ = Lang::off_change(id);
    /// # }
    /// ```
    #[cfg(feature = "registry")]
    #[cfg_attr(docsrs, doc(cfg(feature = "registry")))]
    pub fn on_change<F>(handler: F) -> registry_io::HandlerId
    where
        F: Fn(&LangChangeEvent) + Send + Sync + 'static,
    {
        crate::registry::registry().register(handler)
    }

    /// Deregisters a change-event handler previously installed via
    /// [`Lang::on_change`].
    ///
    /// Returns `true` if a handler with the given id was removed.
    ///
    /// Available when the `registry` feature is enabled.
    #[cfg(feature = "registry")]
    #[cfg_attr(docsrs, doc(cfg(feature = "registry")))]
    #[must_use = "off_change returns whether a handler was actually removed"]
    pub fn off_change(id: registry_io::HandlerId) -> bool {
        crate::registry::registry().unregister(id)
    }

    /// Starts watching the given directory for `<locale>.toml` changes.
    ///
    /// File modifications, atomic-rename writes, and creates are detected
    /// and debounced into a single per-file reload. Each successful reload
    /// fires a [`crate::LangChangeEvent`] through the shared registry
    /// (`registry` feature is implied by `hot-reload`).
    ///
    /// Only files for locales that are already loaded are reloaded. A new
    /// `<locale>.toml` that appears in the directory is ignored until the
    /// application loads that locale with [`Lang::load`] or
    /// [`Lang::load_from`].
    ///
    /// A change handler may call [`Lang::unwatch`] from the watcher thread;
    /// the watcher then stops after the current event.
    ///
    /// Only one watcher may be active per process. Call [`Lang::unwatch`]
    /// before starting a new one.
    ///
    /// Available when the `hot-reload` feature is enabled.
    ///
    /// # Errors
    ///
    /// Returns [`crate::WatchError::Io`] if the watcher cannot subscribe
    /// to filesystem events, or [`crate::WatchError::AlreadyRunning`] if
    /// a watcher is already active.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "hot-reload")]
    /// # fn demo() -> Result<(), lang_lib::WatchError> {
    /// use lang_lib::Lang;
    /// Lang::watch("locales")?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "hot-reload")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot-reload")))]
    pub fn watch(dir: impl AsRef<std::path::Path>) -> Result<(), crate::WatchError> {
        crate::watch::start(dir.as_ref().to_path_buf())
    }

    /// Stops the active filesystem watcher, if any.
    ///
    /// Idempotent: calling [`Lang::unwatch`] when no watcher is running is
    /// a no-op.
    ///
    /// Available when the `hot-reload` feature is enabled.
    #[cfg(feature = "hot-reload")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot-reload")))]
    pub fn unwatch() {
        crate::watch::stop();
    }

    /// Translates a key.
    ///
    /// Lookup order:
    /// 1. The requested locale (or active locale if `None`)
    /// 2. Each locale in the fallback chain, in order
    /// 3. The inline `fallback` string if provided
    /// 4. The key itself (never returns an empty string)
    ///
    /// The hot path is lock-free and zero-allocation: a hit returns
    /// [`Cow::Borrowed`] backed by the interned translation store (in the
    /// default build, except for values a reload added past the reload
    /// budget, which return [`Cow::Owned`]; see [`Lang::load`]); a miss
    /// with an inline fallback returns [`Cow::Borrowed`] of the user-supplied
    /// fallback; a complete miss returns [`Cow::Borrowed`] of the key.
    /// The returned value derefs to `&str` and works transparently with
    /// `format!`, `println!`, and equality against `&str`.
    ///
    /// This is the function called by the [`t!`](crate::t) macro. Prefer
    /// using the macro directly in application code.
    ///
    /// In concurrent server code, passing `Some(locale)` is usually the safest
    /// policy because it avoids mutating the process-wide active locale.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    ///
    /// Lang::load_from("en", "tests/fixtures/locales").unwrap();
    /// let text = Lang::translate("welcome", Some("en"), Some("Welcome"));
    /// assert_eq!(text, "Welcome");
    /// ```
    #[must_use]
    pub fn translate<'a>(
        key: &'a str,
        locale: Option<&'a str>,
        fallback: Option<&'a str>,
    ) -> Cow<'a, str> {
        let state = snapshot();
        let target: &str = locale.unwrap_or(state.active);

        if let Some(map) = state.locales.get(target) {
            if let Some(val) = map.get(key) {
                return stored_to_cow(val);
            }
        }

        // Fallback chains are a handful of entries, so checking the
        // already-visited prefix is cheaper than a set and never allocates.
        for (i, &fb_locale) in state.fallbacks.iter().enumerate() {
            if fb_locale == target || state.fallbacks[..i].contains(&fb_locale) {
                continue;
            }
            if let Some(map) = state.locales.get(fb_locale) {
                if let Some(val) = map.get(key) {
                    return stored_to_cow(val);
                }
            }
        }

        if let Some(fb) = fallback {
            return Cow::Borrowed(fb);
        }

        Cow::Borrowed(key)
    }

    /// Translates a key, returning the value as an `Arc<str>`.
    ///
    /// Available when the `hot-reload` feature is enabled. The hit path is
    /// zero-allocation (a refcount bump on the existing `Arc<str>`); the
    /// miss paths allocate a fresh `Arc<str>` from the caller-supplied
    /// fallback or key.
    ///
    /// Use this when you want to avoid the per-call `String` allocation
    /// imposed by `Lang::translate` in `hot-reload` builds. Be aware that
    /// high concurrency on the same translation key can produce refcount
    /// cache-line contention; if you suspect this is biting you, measure
    /// and switch back to [`Lang::translate`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "hot-reload")]
    /// # fn demo() {
    /// use lang_lib::Lang;
    ///
    /// let value = Lang::translate_arc("greeting", Some("en"), None);
    /// println!("{value}");
    /// # }
    /// ```
    #[cfg(feature = "hot-reload")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot-reload")))]
    #[must_use]
    pub fn translate_arc(key: &str, locale: Option<&str>, fallback: Option<&str>) -> Arc<str> {
        let state = snapshot();
        let target: &str = locale.unwrap_or(state.active);

        if let Some(map) = state.locales.get(target) {
            if let Some(val) = map.get(key) {
                return Arc::clone(val);
            }
        }

        // Fallback chains are a handful of entries, so checking the
        // already-visited prefix is cheaper than a set and never allocates.
        for (i, &fb_locale) in state.fallbacks.iter().enumerate() {
            if fb_locale == target || state.fallbacks[..i].contains(&fb_locale) {
                continue;
            }
            if let Some(map) = state.locales.get(fb_locale) {
                if let Some(val) = map.get(key) {
                    return Arc::clone(val);
                }
            }
        }

        Arc::from(fallback.unwrap_or(key))
    }
}

/// Coerces the per-feature `StoredValue` into a `Cow<'a, str>`.
///
/// Default build: a pure pointer copy for interned values (the borrow's
/// `'static` lifetime coerces to `'a`), and an owned copy for values a
/// reload added past the reload budget.
///
/// `hot-reload` build: allocates a new `String` from the `Arc<str>`'s
/// bytes. Avoids touching the `Arc`'s refcount, so there is no contention
/// when the same key is read from many threads simultaneously.
#[cfg(not(feature = "hot-reload"))]
#[inline]
fn stored_to_cow<'a>(val: &StoredValue) -> Cow<'a, str> {
    match val {
        Text::Static(s) => Cow::Borrowed(s),
        Text::Shared(s) => Cow::Owned(s.as_ref().to_owned()),
    }
}

#[cfg(feature = "hot-reload")]
#[inline]
fn stored_to_cow<'a>(val: &StoredValue) -> Cow<'a, str> {
    Cow::Owned(val.as_ref().to_owned())
}
