use std::fs;
use std::path::{Component, Path};

use rustc_hash::FxHashMap;

use crate::error::LangError;
use crate::intern::intern;
use crate::store::StoredValue;

fn validate_locale(locale: &str) -> Result<(), LangError> {
    let mut components = Path::new(locale).components();

    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(LangError::InvalidLocale {
            locale: locale.to_string(),
        }),
    }
}

/// Loads a TOML language file from `path/{locale}.toml` and returns a flat
/// map of interned keys to translation values.
///
/// Value storage depends on the active feature set: `&'static str` (interner)
/// in default builds, `Arc<str>` in `hot-reload` builds. See [`StoredValue`].
///
/// Only string values are accepted. Any non-string value in the TOML file is
/// silently skipped — this keeps the format simple and predictable.
pub(crate) fn load_file(
    path: &str,
    locale: &str,
) -> Result<FxHashMap<&'static str, StoredValue>, LangError> {
    validate_locale(locale)?;

    let file_path = Path::new(path).join(format!("{locale}.toml"));

    let raw = fs::read_to_string(&file_path).map_err(|e| LangError::Io {
        locale: locale.to_string(),
        cause: e.to_string(),
    })?;

    parse_toml(locale, &raw)
}

/// Parses a raw TOML string into a flat map of interned keys to translation
/// values.
///
/// Value storage depends on the active feature set: `&'static str` (interner)
/// in default builds, `Arc<str>` in `hot-reload` builds.
///
/// Only top-level string values are extracted. Tables, arrays, integers, and
/// other types are skipped without error.
pub(crate) fn parse_toml(
    locale: &str,
    raw: &str,
) -> Result<FxHashMap<&'static str, StoredValue>, LangError> {
    let table: toml::Table = raw.parse().map_err(|e: toml::de::Error| LangError::Parse {
        locale: locale.to_string(),
        cause: e.to_string(),
    })?;

    let mut map: FxHashMap<&'static str, StoredValue> =
        FxHashMap::with_capacity_and_hasher(table.len(), rustc_hash::FxBuildHasher);

    for (key, value) in table {
        if let toml::Value::String(s) = value {
            let _ = map.insert(intern(&key), value_from_string(s));
        }
    }

    Ok(map)
}

/// Converts a parsed TOML string into the active `StoredValue` representation.
///
/// In the default build this interns the string into the leak-based pool;
/// in `hot-reload` builds it wraps the bytes in a fresh `Arc<str>` that will
/// be dropped when the locale is reloaded or unloaded.
#[cfg(not(feature = "hot-reload"))]
#[inline]
fn value_from_string(s: String) -> StoredValue {
    intern(&s)
}

/// Converts a parsed TOML string into the active `StoredValue` representation.
///
/// In the default build this interns the string into the leak-based pool;
/// in `hot-reload` builds it wraps the bytes in a fresh `Arc<str>` that will
/// be dropped when the locale is reloaded or unloaded.
#[cfg(feature = "hot-reload")]
#[inline]
fn value_from_string(s: String) -> StoredValue {
    std::sync::Arc::from(s)
}
