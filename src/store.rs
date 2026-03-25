use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

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
            path:      "locales".to_string(),
            active:    "en".to_string(),
            fallbacks: vec!["en".to_string()],
            locales:   HashMap::new(),
        }
    }
}

static STATE: OnceLock<RwLock<LangState>> = OnceLock::new();

fn state() -> &'static RwLock<LangState> {
    STATE.get_or_init(|| RwLock::new(LangState::new()))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The main entry point for configuring and querying the localization system.
pub struct Lang;

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
        state().write().unwrap().path = path.into();
    }

    /// Returns the current language file path.
    pub fn path() -> String {
        state().read().unwrap().path.clone()
    }

    /// Sets the active locale used when no locale is specified in `t!`.
    ///
    /// The locale does not need to be loaded before calling this, but
    /// translations will be empty until it is.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lang_lib::Lang;
    /// Lang::set_locale("es");
    /// ```
    pub fn set_locale(locale: impl Into<String>) {
        state().write().unwrap().active = locale.into();
    }

    /// Returns the currently active locale.
    pub fn locale() -> String {
        state().read().unwrap().active.clone()
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
        state().write().unwrap().fallbacks = chain;
    }

    /// Loads a locale from disk.
    ///
    /// Reads `{path}/{locale}.toml` and stores all translations in memory.
    /// Calling this a second time for the same locale replaces the existing
    /// translations with a fresh load from disk.
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
        let path = state().read().unwrap().path.clone();
        let map = loader::load_file(&path, &locale)?;
        state().write().unwrap().locales.insert(locale, map);
        Ok(())
    }

    /// Loads a locale from a specific path, ignoring the global path setting.
    ///
    /// Useful when a project stores one locale separately from the others.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Io`] or [`LangError::Parse`] on failure.
    pub fn load_from(locale: impl Into<String>, path: &str) -> Result<(), LangError> {
        let locale = locale.into();
        let map = loader::load_file(path, &locale)?;
        state().write().unwrap().locales.insert(locale, map);
        Ok(())
    }

    /// Returns `true` if the locale has been loaded.
    pub fn is_loaded(locale: &str) -> bool {
        state().read().unwrap().locales.contains_key(locale)
    }

    /// Returns a list of all loaded locale identifiers.
    pub fn loaded() -> Vec<String> {
        state().read().unwrap().locales.keys().cloned().collect()
    }

    /// Unloads a locale and frees its memory.
    pub fn unload(locale: &str) {
        state().write().unwrap().locales.remove(locale);
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
    pub fn translate(key: &str, locale: Option<&str>, fallback: Option<&str>) -> String {
        let state = state().read().unwrap();

        let target = locale.unwrap_or(&state.active);

        // Check requested locale first
        if let Some(map) = state.locales.get(target) {
            if let Some(val) = map.get(key) {
                return val.clone();
            }
        }

        // Walk the fallback chain
        for fb_locale in &state.fallbacks {
            if fb_locale == target {
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
