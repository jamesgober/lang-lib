use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::error::LangError;

/// Loads a TOML language file from `path/{locale}.toml` and returns a flat
/// `HashMap<String, String>` of all key-value pairs.
///
/// Only string values are accepted. Any non-string value in the TOML file is
/// silently skipped — this keeps the format simple and predictable.
pub fn load_file(path: &str, locale: &str) -> Result<HashMap<String, String>, LangError> {
    let file_path = format!("{}/{}.toml", path, locale);

    let raw = fs::read_to_string(Path::new(&file_path)).map_err(|e| LangError::Io {
        locale: locale.to_string(),
        cause: e.to_string(),
    })?;

    parse_toml(locale, &raw)
}

/// Parses a raw TOML string into a flat `HashMap<String, String>`.
///
/// Only top-level string values are extracted. Tables, arrays, integers, and
/// other types are skipped without error.
pub fn parse_toml(locale: &str, raw: &str) -> Result<HashMap<String, String>, LangError> {
    let table: toml::Table = raw.parse().map_err(|e: toml::de::Error| LangError::Parse {
        locale: locale.to_string(),
        cause: e.to_string(),
    })?;

    let mut map = HashMap::with_capacity(table.len());

    for (key, value) in table {
        if let toml::Value::String(s) = value {
            map.insert(key, s);
        }
    }

    Ok(map)
}
