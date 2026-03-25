//! A lightweight, high-performance localization library.
//!
//! `lang-lib` loads TOML language files, supports runtime locale switching,
//! configurable file paths, and automatic fallback chains. It is designed to
//! be dropped into any project without ceremony.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use lang_lib::{t, Lang};
//!
//! // Configure once at startup
//! Lang::set_path("locales");
//! Lang::load("en").unwrap();
//! Lang::load("es").unwrap();
//! Lang::set_locale("en");
//!
//! // Translate anywhere
//! let msg = t!("bad_password");
//! let msg_es = t!("bad_password", "es");
//! let msg_fb = t!("missing_key", fallback: "Default message");
//! ```
//!
//! # File Format
//!
//! Language files are plain TOML, one key per line:
//!
//! ```toml
//! bad_password = "Your password is incorrect."
//! not_found    = "The page you requested does not exist."
//! ```
//!
//! Files are resolved as `{path}/{locale}.toml`.

#![deny(warnings)]
#![deny(clippy::all)]
#![deny(missing_docs)]

mod error;
mod loader;
mod store;

pub use error::LangError;
pub use store::Lang;

/// Translates a key using the active locale.
///
/// Falls back through the fallback chain and finally returns the key itself
/// if no translation is found anywhere.
///
/// # Examples
///
/// ```rust,no_run
/// use lang_lib::t;
///
/// // Active locale
/// let msg = t!("greeting");
///
/// // Specific locale
/// let msg = t!("greeting", "es");
///
/// // Inline fallback
/// let msg = t!("unknown_key", fallback: "Hello");
/// ```
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::Lang::translate($key, None, None)
    };
    ($key:expr, $locale:expr) => {
        $crate::Lang::translate($key, Some($locale), None)
    };
    ($key:expr, fallback: $fallback:expr) => {
        $crate::Lang::translate($key, None, Some($fallback))
    };
    ($key:expr, $locale:expr, fallback: $fallback:expr) => {
        $crate::Lang::translate($key, Some($locale), Some($fallback))
    };
}
