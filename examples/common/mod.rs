use std::path::PathBuf;

use lang_lib::{Lang, resolve_accept_language};

pub const DEFAULT_LOCALE: &str = "en";
pub const SUPPORTED_LOCALES: &[&str] = &["en", "es"];

pub fn configure_i18n() -> Result<(), lang_lib::LangError> {
    let locale_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/locales");

    Lang::set_path(locale_dir.to_string_lossy().into_owned());
    Lang::load("en")?;
    Lang::load("es")?;
    Lang::set_fallbacks(vec![DEFAULT_LOCALE.to_string()]);
    Lang::set_locale(DEFAULT_LOCALE);

    Ok(())
}

pub fn resolve_request_locale(header: &str, default_locale: &'static str) -> &'static str {
    resolve_accept_language(header, SUPPORTED_LOCALES, default_locale)
}
