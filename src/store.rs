use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use arc_swap::{ArcSwap, Guard};
use rustc_hash::FxHashMap;

use crate::error::LangError;
use crate::intern::intern;
use crate::loader;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

type LocaleMap = FxHashMap<&'static str, &'static str>;

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
/// `Translator` stores an interned locale identifier and forwards lookups to
/// the global [`Lang`] store without mutating the process-wide active locale.
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
    #[must_use]
    pub fn new(locale: impl AsRef<str>) -> Self {
        Self {
            locale: intern(locale.as_ref()),
        }
    }

    /// Returns the locale used by this translator.
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
    /// translations will be empty until it is.
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
        let interned = intern(locale.as_ref());
        with_write(|state| state.active = interned);
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
    /// Locale names must be single file stems such as `en`, `en-US`, or
    /// `pt_BR`. Path separators and relative path components are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Io`] if the file cannot be read, or
    /// [`LangError::Parse`] if the TOML is invalid.
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
        let locale = locale.as_ref();
        let path = snapshot().path;
        let map = loader::load_file(path, locale)?;
        let interned_locale = intern(locale);
        let arc_map: Arc<LocaleMap> = Arc::new(map);
        with_write(|state| {
            let _ = state.locales.insert(interned_locale, Arc::clone(&arc_map));
        });
        Ok(())
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
        let locale = locale.as_ref();
        let map = loader::load_file(path, locale)?;
        let interned_locale = intern(locale);
        let arc_map: Arc<LocaleMap> = Arc::new(map);
        with_write(|state| {
            let _ = state.locales.insert(interned_locale, Arc::clone(&arc_map));
        });
        Ok(())
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
    /// Note: in `1.1.x`, translation strings are interned into a process-wide
    /// pool, so unloading a locale removes it from the lookup table but does
    /// not reclaim the interned bytes themselves. The `1.2.x` hot-reload
    /// milestone revisits this so long-running reloaders do not grow the
    /// interner without bound.
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
        with_write(|state| {
            let _ = state.locales.remove(locale);
        });
    }

    /// Creates a request-scoped [`Translator`] for the provided locale.
    ///
    /// This is a convenience wrapper around [`Translator::new`]. It is most
    /// useful in server code where locale is resolved per request and passed
    /// through the handler stack.
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

    /// Translates a key.
    ///
    /// Lookup order:
    /// 1. The requested locale (or active locale if `None`)
    /// 2. Each locale in the fallback chain, in order
    /// 3. The inline `fallback` string if provided
    /// 4. The key itself (never returns an empty string)
    ///
    /// The hot path is lock-free and zero-allocation: a hit returns
    /// [`Cow::Borrowed`] backed by the interned translation store; a miss
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
            if let Some(&val) = map.get(key) {
                return Cow::Borrowed(val);
            }
        }

        let mut seen = HashSet::with_capacity(state.fallbacks.len());
        for &fb_locale in state.fallbacks.iter() {
            if fb_locale == target || !seen.insert(fb_locale) {
                continue;
            }
            if let Some(map) = state.locales.get(fb_locale) {
                if let Some(&val) = map.get(key) {
                    return Cow::Borrowed(val);
                }
            }
        }

        if let Some(fb) = fallback {
            return Cow::Borrowed(fb);
        }

        Cow::Borrowed(key)
    }
}
