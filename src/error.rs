use std::fmt;

/// Errors produced by `lang-lib`.
#[derive(Debug)]
pub enum LangError {
    /// The language file could not be read from disk.
    Io {
        /// The locale that failed to load.
        locale: String,
        /// The underlying I/O error message.
        cause: String,
    },
    /// The language file was not valid TOML or contained non-string values.
    Parse {
        /// The locale whose file could not be parsed.
        locale: String,
        /// A description of the parse failure.
        cause: String,
    },
    /// A locale was requested that has never been loaded.
    NotLoaded {
        /// The locale that was not found.
        locale: String,
    },
    /// A locale identifier was rejected before any file access occurred.
    InvalidLocale {
        /// The locale that was rejected.
        locale: String,
    },
}

impl fmt::Display for LangError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LangError::Io { locale, cause } => {
                write!(f, "failed to read language file for '{locale}': {cause}")
            }
            LangError::Parse { locale, cause } => {
                write!(f, "failed to parse language file for '{locale}': {cause}")
            }
            LangError::NotLoaded { locale } => {
                write!(f, "locale '{locale}' has not been loaded")
            }
            LangError::InvalidLocale { locale } => {
                write!(
                    f,
                    "locale '{locale}' is invalid; expected a single locale name without path separators"
                )
            }
        }
    }
}

impl std::error::Error for LangError {}
