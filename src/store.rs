use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::error::LangError;
use crate::loader;

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

struct LangState {
    /// Base directory where language files are stored.
    path: String,
    /// Active locale used by `t!("key")` with no explicit locale argument.
    active: String,
    /// Fallback locale chain. Checked in order when a key is missing.
    fallbacks: Vec<String>,
    /// All loaded locales. Each entry maps translation keys to their strings.
    locales: HashMap<String, HashMap<String, String>>,
}

impl LangState {
    fn new() -> Self {
        Self {
            path: "locales".to_string(),
            active: "en".to_string(),
            fallbacks: vec!["en".to_string()],
            locales: HashMap::new(),
        }
    }
}

static STATE: OnceLock<RwLock<LangState>> = OnceLock::new();

fn state() -> &'static RwLock<LangState> {
    STATE.get_or_init(|| RwLock::new(LangState::new()))
}

fn read_state() -> RwLockReadGuard<'static, LangState> {
    state()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write_state() -> RwLockWriteGuard<'static, LangState> {
    state()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The main entry point for configuring and querying the localization system.
///
/// `Lang` manages process-global state behind a read/write lock. Configure it
/// once during startup, load the locales your application needs, and then use
/// [`t!`](crate::t) or [`Lang::translate`] wherever translated text is needed.
pub struct Lang;

/// A lightweight, request-scoped translation helper.
///
/// `Translator` stores a locale identifier and forwards lookups to the global
/// [`Lang`] store without mutating the process-wide active locale. This makes
/// it a good fit for web handlers, jobs, and other code paths where locale is
/// part of the input rather than part of global application state.
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Translator {
    locale: String,
}

impl Translator {
    /// Creates a translator bound to a specific locale.
    pub fn new(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
        }
    }

    /// Returns the locale used by this translator.
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Translates a key using this translator's locale.
    pub fn translate(&self, key: &str) -> String {
        Lang::translate(key, Some(self.locale.as_str()), None)
    }

    /// Translates a key using this translator's locale and an inline fallback.
    pub fn translate_with_fallback(&self, key: &str, fallback: &str) -> String {
        Lang::translate(key, Some(self.locale.as_str()), Some(fallback))
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
    pub fn set_path(path: impl Into<String>) {
        write_state().path = path.into();
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
    pub fn path() -> String {
        read_state().path.clone()
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
    pub fn set_locale(locale: impl Into<String>) {
        write_state().active = locale.into();
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
    pub fn locale() -> String {
        read_state().active.clone()
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
    pub fn set_fallbacks(chain: Vec<String>) {
        write_state().fallbacks = chain;
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
    pub fn load(locale: impl Into<String>) -> Result<(), LangError> {
        let locale = locale.into();
        let path = read_state().path.clone();
        let map = loader::load_file(&path, &locale)?;
        write_state().locales.insert(locale, map);
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
    pub fn load_from(locale: impl Into<String>, path: &str) -> Result<(), LangError> {
        let locale = locale.into();
        let map = loader::load_file(path, &locale)?;
        write_state().locales.insert(locale, map);
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
    pub fn is_loaded(locale: &str) -> bool {
        read_state().locales.contains_key(locale)
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
    /// assert_eq!(Lang::loaded(), vec!["en".to_string(), "es".to_string()]);
    /// ```
    pub fn loaded() -> Vec<String> {
        let mut locales: Vec<_> = read_state().locales.keys().cloned().collect();
        locales.sort_unstable();
        locales
    }

    /// Unloads a locale and frees its memory.
    ///
    /// Unloading a locale does not change the active locale or fallback chain.
    /// If either of those still references the removed locale, translation will
    /// simply skip it.
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
        write_state().locales.remove(locale);
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
    pub fn translator(locale: impl Into<String>) -> Translator {
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
    pub fn translate(key: &str, locale: Option<&str>, fallback: Option<&str>) -> String {
        let state = read_state();

        let target = locale.unwrap_or(&state.active);

        // Check requested locale first
        if let Some(map) = state.locales.get(target) {
            if let Some(val) = map.get(key) {
                return val.clone();
            }
        }

        // Walk the fallback chain
        let mut seen = HashSet::with_capacity(state.fallbacks.len());
        for fb_locale in &state.fallbacks {
            if fb_locale == target || !seen.insert(fb_locale.as_str()) {
                continue;
            }
            if let Some(map) = state.locales.get(fb_locale.as_str()) {
                if let Some(val) = map.get(key) {
                    return val.clone();
                }
            }
        }

        // Inline fallback
        if let Some(fb) = fallback {
            return fb.to_string();
        }

        // Last resort: return the key itself
        key.to_string()
    }
}
