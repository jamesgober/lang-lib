use lang_lib::{Lang, Translator, resolve_accept_language, resolve_accept_language_owned, t};
use std::io::Write;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Creates a temporary directory with TOML lang files and returns it.
/// The caller must keep the TempDir alive for the duration of the test.
fn setup_temp_locales(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (locale, content) in files {
        let path = dir.path().join(format!("{}.toml", locale));
        let mut f = std::fs::File::create(path).unwrap();
        write!(f, "{}", content).unwrap();
    }
    dir
}

fn test_guard() -> MutexGuard<'static, ()> {
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn reset_lang() {
    for locale in Lang::loaded() {
        Lang::unload(locale);
    }

    Lang::set_path("locales");
    Lang::set_locale("en");
    Lang::set_fallbacks(vec!["en".to_string()]);
}

// ---------------------------------------------------------------------------
// set_path and load
// ---------------------------------------------------------------------------

#[test]
fn test_load_valid_file() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "greeting = \"Hello\"\nfarewell = \"Goodbye\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    assert!(Lang::is_loaded("en"));
}

#[test]
fn test_load_missing_file_returns_error() {
    let _guard = test_guard();
    reset_lang();
    Lang::set_path("/nonexistent/path/that/does/not/exist");
    let result = Lang::load("xx");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("xx"));
}

#[test]
fn test_load_invalid_toml_returns_parse_error() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("bad", "this is not = valid = toml !!!")]);
    Lang::set_path(dir.path().to_str().unwrap());
    let result = Lang::load("bad");
    assert!(result.is_err());
}

#[test]
fn test_load_from_custom_path() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("fr", "hello = \"Bonjour\"")]);
    Lang::load_from("fr", dir.path().to_str().unwrap()).unwrap();
    assert!(Lang::is_loaded("fr"));
}

#[test]
fn test_load_rejects_path_traversal_locale() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "hello = \"Hello\"")]);
    Lang::set_path(dir.path().to_str().unwrap());

    let result = Lang::load("../en");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid"));
}

// ---------------------------------------------------------------------------
// Basic translation
// ---------------------------------------------------------------------------

#[test]
fn test_translate_active_locale() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "key = \"value\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    Lang::set_locale("en");
    assert_eq!(Lang::translate("key", None, None), "value");
}

#[test]
fn test_translate_specific_locale() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[
        ("en_specific", "msg = \"Hello\""),
        ("es_specific", "msg = \"Hola\""),
    ]);
    let path = dir.path().to_str().unwrap();
    Lang::load_from("en_specific", path).unwrap();
    Lang::load_from("es_specific", path).unwrap();
    assert_eq!(Lang::translate("msg", Some("es_specific"), None), "Hola");
    assert_eq!(Lang::translate("msg", Some("en_specific"), None), "Hello");
}

#[test]
fn test_resolve_accept_language_prefers_supported_primary_match() {
    assert_eq!(
        resolve_accept_language("es-ES,es;q=0.9,en;q=0.8", &["en", "es"], "en"),
        "es"
    );
}

#[test]
fn test_resolve_accept_language_honors_q_values() {
    assert_eq!(
        resolve_accept_language("es;q=0.4,en;q=0.9", &["en", "es"], "en"),
        "en"
    );
}

#[test]
fn test_resolve_accept_language_returns_default_when_no_match_exists() {
    assert_eq!(
        resolve_accept_language("fr-FR,fr;q=0.9", &["en", "es"], "en"),
        "en"
    );
}

#[test]
fn test_resolve_accept_language_prefers_exact_match_over_primary_match() {
    assert_eq!(
        resolve_accept_language("en-GB,en;q=0.8", &["en", "en-GB"], "en"),
        "en-GB"
    );
}

#[test]
fn test_resolve_accept_language_owned_accepts_runtime_locale_list() {
    let supported = vec!["en".to_string(), "es".to_string()];

    assert_eq!(
        resolve_accept_language_owned("es-MX,es;q=0.9,en;q=0.7", &supported, "en"),
        "es"
    );
}

#[test]
fn test_resolve_accept_language_owned_returns_owned_default() {
    let supported = vec!["en".to_string(), "es".to_string()];

    assert_eq!(
        resolve_accept_language_owned("fr-CA,fr;q=0.9", &supported, "en"),
        "en".to_string()
    );
}

#[test]
fn test_translator_uses_explicit_locale_without_touching_active_locale() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "title = \"Hello\""), ("es", "title = \"Hola\"")]);
    let path = dir.path().to_str().unwrap();

    Lang::load_from("en", path).unwrap();
    Lang::load_from("es", path).unwrap();
    Lang::set_locale("en");

    let translator = Lang::translator("es");

    assert_eq!(translator.locale(), "es");
    assert_eq!(translator.translate("title"), "Hola");
    assert_eq!(Lang::locale(), "en");
}

#[test]
fn test_translator_inline_fallback_is_used() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "title = \"Hello\"")]);
    let path = dir.path().to_str().unwrap();

    Lang::load_from("en", path).unwrap();
    let translator = Translator::new("en");

    assert_eq!(
        translator.translate_with_fallback("missing", "Default title"),
        "Default title"
    );
}

#[test]
fn test_missing_key_returns_key_itself() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "existing = \"exists\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    Lang::set_locale("en");
    Lang::set_fallbacks(vec![]);
    assert_eq!(
        Lang::translate("totally_missing", None, None),
        "totally_missing"
    );
}

// ---------------------------------------------------------------------------
// Fallback chain
// ---------------------------------------------------------------------------

#[test]
fn test_fallback_chain_used_when_key_missing() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[
        ("en_fb", "only_in_en = \"English only\""),
        ("es_fb", "other = \"other\""),
    ]);
    let path = dir.path().to_str().unwrap();
    Lang::load_from("en_fb", path).unwrap();
    Lang::load_from("es_fb", path).unwrap();
    Lang::set_locale("es_fb");
    Lang::set_fallbacks(vec!["en_fb".to_string()]);
    assert_eq!(Lang::translate("only_in_en", None, None), "English only");
}

#[test]
fn test_inline_fallback_used_last() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "real_key = \"real\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    Lang::set_locale("en");
    Lang::set_fallbacks(vec![]);
    let result = Lang::translate("ghost_key", None, Some("fallback text"));
    assert_eq!(result, "fallback text");
}

#[test]
fn test_fallback_does_not_override_found_translation() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[
        ("en_nofb", "shared = \"English\""),
        ("es_nofb", "shared = \"Español\""),
    ]);
    let path = dir.path().to_str().unwrap();
    Lang::load_from("en_nofb", path).unwrap();
    Lang::load_from("es_nofb", path).unwrap();
    Lang::set_locale("es_nofb");
    Lang::set_fallbacks(vec!["en_nofb".to_string()]);
    assert_eq!(Lang::translate("shared", None, None), "Español");
}

// ---------------------------------------------------------------------------
// t! macro
// ---------------------------------------------------------------------------

#[test]
fn test_t_macro_active_locale() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "hi = \"Hi there\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    Lang::set_locale("en");
    assert_eq!(t!("hi"), "Hi there");
}

#[test]
fn test_t_macro_explicit_locale() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("de", "hi = \"Hallo\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("de").unwrap();
    assert_eq!(t!("hi", "de"), "Hallo");
}

#[test]
fn test_t_macro_inline_fallback() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "other = \"something\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    Lang::set_locale("en");
    assert_eq!(t!("no_such_key", fallback: "default"), "default");
}

#[test]
fn test_t_macro_locale_and_fallback() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "other = \"something\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    assert_eq!(t!("missing", "en", fallback: "nope"), "nope");
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_non_string_toml_values_are_skipped() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "name = \"Alice\"\ncount = 42\nflag = true")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    Lang::set_locale("en");
    // string key loads fine
    assert_eq!(Lang::translate("name", None, None), "Alice");
    // integer and bool keys are skipped — returns key itself
    assert_eq!(Lang::translate("count", None, None), "count");
    assert_eq!(Lang::translate("flag", None, None), "flag");
}

#[test]
fn test_empty_lang_file_loads_without_error() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("empty", "")]);
    Lang::set_path(dir.path().to_str().unwrap());
    assert!(Lang::load("empty").is_ok());
}

#[test]
fn test_unload_removes_locale() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en", "x = \"y\"")]);
    Lang::set_path(dir.path().to_str().unwrap());
    Lang::load("en").unwrap();
    assert!(Lang::is_loaded("en"));
    Lang::unload("en");
    assert!(!Lang::is_loaded("en"));
}

#[test]
fn test_reload_replaces_translations() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("en_reload", "val = \"first\"")]);
    let path = dir.path().to_str().unwrap();
    Lang::load_from("en_reload", path).unwrap();
    assert_eq!(Lang::translate("val", Some("en_reload"), None), "first");

    // overwrite the file with new content and reload
    let file = dir.path().join("en_reload.toml");
    std::fs::write(file, "val = \"second\"").unwrap();
    Lang::load_from("en_reload", path).unwrap();
    assert_eq!(Lang::translate("val", Some("en_reload"), None), "second");
}

#[test]
fn test_loaded_locales_are_sorted() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[("z_sorted", "value = \"z\""), ("a_sorted", "value = \"a\"")]);
    let path = dir.path().to_str().unwrap();

    Lang::load_from("z_sorted", path).unwrap();
    Lang::load_from("a_sorted", path).unwrap();

    assert_eq!(
        Lang::loaded(),
        vec!["a_sorted".to_string(), "z_sorted".to_string()]
    );
}

#[test]
fn test_duplicate_fallbacks_do_not_change_resolution() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_temp_locales(&[
        ("en_dup", "hello = \"Hello\""),
        ("es_dup", "other = \"Other\""),
    ]);
    let path = dir.path().to_str().unwrap();

    Lang::load_from("en_dup", path).unwrap();
    Lang::load_from("es_dup", path).unwrap();
    Lang::set_locale("es_dup");
    Lang::set_fallbacks(vec![
        "en_dup".to_string(),
        "en_dup".to_string(),
        "en_dup".to_string(),
    ]);

    assert_eq!(Lang::translate("hello", None, None), "Hello");
}
