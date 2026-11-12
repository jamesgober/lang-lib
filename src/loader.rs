use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use rustc_hash::FxHashMap;

use crate::error::LangError;
use crate::intern::Text;
use crate::store::StoredValue;

/// Default limit on the size of one locale file: 16 MiB.
pub(crate) const DEFAULT_MAX_FILE_SIZE: u64 = 16 * 1024 * 1024;

static MAX_FILE_SIZE: AtomicU64 = AtomicU64::new(DEFAULT_MAX_FILE_SIZE);

/// Longest accepted locale name, in bytes.
const MAX_LOCALE_LEN: usize = 64;

pub(crate) fn max_file_size() -> u64 {
    MAX_FILE_SIZE.load(Ordering::Relaxed)
}

pub(crate) fn set_max_file_size(bytes: u64) {
    MAX_FILE_SIZE.store(bytes, Ordering::Relaxed);
}

/// Returns `true` if `locale` is shaped like a locale identifier.
///
/// Accepted: 1 to 64 ASCII bytes, starting with a letter or digit, followed
/// by letters, digits, `-`, `_`, `.`, or `@`. That covers BCP 47 tags
/// (`en`, `en-US`, `zh-Hant-TW`), underscore forms (`pt_BR`), and POSIX
/// names (`sr_RS@latin`, `en_US.UTF-8`). Path separators, drive prefixes,
/// spaces, and control characters are rejected, so a name can never point
/// outside the locale directory.
pub(crate) fn is_locale_name(locale: &str) -> bool {
    let bytes = locale.as_bytes();
    match bytes.split_first() {
        Some((first, rest)) => {
            bytes.len() <= MAX_LOCALE_LEN
                && first.is_ascii_alphanumeric()
                && rest
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'@'))
        }
        None => false,
    }
}

fn validate_locale(locale: &str) -> Result<(), LangError> {
    if is_locale_name(locale) {
        Ok(())
    } else {
        Err(LangError::InvalidLocale {
            locale: locale.to_string(),
        })
    }
}

fn io_error(locale: &str, cause: &impl std::fmt::Display) -> LangError {
    LangError::Io {
        locale: locale.to_string(),
        cause: cause.to_string(),
    }
}

fn read_limited(file_path: &Path, locale: &str) -> Result<String, LangError> {
    let limit = max_file_size();
    let too_large = || {
        io_error(
            locale,
            &format!("file is larger than the {limit}-byte limit set by Lang::set_max_file_size"),
        )
    };
    let file = File::open(file_path).map_err(|e| io_error(locale, &e))?;
    let len = file.metadata().map_err(|e| io_error(locale, &e))?.len();
    if len > limit {
        return Err(too_large());
    }
    let mut raw = String::with_capacity(usize::try_from(len).unwrap_or(0));
    let _ = file
        .take(limit.saturating_add(1))
        .read_to_string(&mut raw)
        .map_err(|e| io_error(locale, &e))?;
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > limit {
        return Err(too_large());
    }
    Ok(raw)
}

/// Loads a TOML language file from `path/{locale}.toml` and returns a flat
/// map of keys to translation values.
///
/// `first_load` is `true` when the locale has never been loaded in this
/// process; see [`Text::new`] for what that changes.
///
/// Only string values are accepted. Any non-string value in the TOML file is
/// silently skipped, which keeps the format simple and predictable.
pub(crate) fn load_file(
    path: &str,
    locale: &str,
    first_load: bool,
) -> Result<FxHashMap<Text, StoredValue>, LangError> {
    validate_locale(locale)?;

    let file_path = Path::new(path).join(format!("{locale}.toml"));
    let raw = read_limited(&file_path, locale)?;

    parse_toml(locale, &raw, first_load)
}

/// Describes a TOML parse error by position and message, without the
/// snippet of file content that the TOML crate's `Display` output quotes.
fn describe_parse_error(raw: &str, e: &toml::de::Error) -> String {
    let message = e.message().lines().next().unwrap_or("invalid TOML");
    match e.span() {
        Some(span) => {
            let offset = span.start.min(raw.len());
            let before = raw.get(..offset).unwrap_or("");
            let line = before.matches('\n').count() + 1;
            let column = before
                .rsplit('\n')
                .next()
                .map_or(0, |tail| tail.chars().count())
                + 1;
            format!("line {line}, column {column}: {message}")
        }
        None => message.to_string(),
    }
}

/// Parses a raw TOML string into a flat map of keys to translation values.
///
/// Only top-level string values are extracted. Tables, arrays, integers, and
/// other types are skipped without error.
pub(crate) fn parse_toml(
    locale: &str,
    raw: &str,
    first_load: bool,
) -> Result<FxHashMap<Text, StoredValue>, LangError> {
    let table: toml::Table = raw.parse().map_err(|e: toml::de::Error| LangError::Parse {
        locale: locale.to_string(),
        cause: describe_parse_error(raw, &e),
    })?;

    let mut map: FxHashMap<Text, StoredValue> =
        FxHashMap::with_capacity_and_hasher(table.len(), rustc_hash::FxBuildHasher);

    for (key, value) in table {
        if let toml::Value::String(s) = value {
            let _ = map.insert(
                Text::new(&key, first_load),
                value_from_string(&s, first_load),
            );
        }
    }

    Ok(map)
}

/// Converts a parsed TOML string into the default build's `StoredValue`.
///
/// Interned on a first load (or while the reload budget lasts), so hits
/// return a borrowed `&'static str`; reference counted otherwise.
#[cfg(not(feature = "hot-reload"))]
#[inline]
fn value_from_string(s: &str, first_load: bool) -> StoredValue {
    Text::new(s, first_load)
}

/// Converts a parsed TOML string into the `hot-reload` build's
/// `StoredValue`: a fresh `Arc<str>` that is dropped when the locale is
/// reloaded or unloaded.
#[cfg(feature = "hot-reload")]
#[inline]
fn value_from_string(s: &str, _first_load: bool) -> StoredValue {
    std::sync::Arc::from(s)
}

#[cfg(test)]
mod tests {
    use super::{is_locale_name, parse_toml};
    use crate::error::LangError;

    #[test]
    fn locale_names_accept_common_forms() {
        for name in [
            "en",
            "en-US",
            "pt_BR",
            "zh-Hant-TW",
            "sr_RS@latin",
            "en_US.UTF-8",
            "x1",
        ] {
            assert!(is_locale_name(name), "{name} should be accepted");
        }
    }

    #[test]
    fn locale_names_reject_paths_and_odd_characters() {
        let long = "a".repeat(65);
        for name in [
            "",
            "en/",
            "en\\",
            "../en",
            "..",
            ".",
            ".hidden",
            "/etc",
            "C:",
            "C:en",
            "en us",
            "en\n",
            "-en",
            "\u{e9}n",
            long.as_str(),
        ] {
            assert!(!is_locale_name(name), "{name:?} should be rejected");
        }
    }

    #[test]
    fn parse_error_reports_position_without_file_content() {
        let result = parse_toml(
            "secret",
            "ok = \"fine\"\npassword = hunter2-not-quoted\n",
            true,
        );
        match result {
            Err(LangError::Parse { cause, .. }) => {
                assert!(!cause.contains("hunter2"), "cause echoes content: {cause}");
                assert!(cause.starts_with("line 2, column "), "cause: {cause}");
            }
            Err(other) => panic!("expected a parse error, got {other:?}"),
            Ok(_) => panic!("invalid TOML parsed"),
        }
    }
}
