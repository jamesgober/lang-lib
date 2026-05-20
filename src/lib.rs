//! A lightweight, high-performance localization library.
//!
//! `lang-lib` loads TOML language files, supports runtime locale switching,
//! configurable file paths, and automatic fallback chains. It is designed to
//! be dropped into any project without ceremony.
//!
//! # What This Crate Does Well
//!
//! - Keeps the runtime model simple: load files once, then translate by key.
//! - Uses plain TOML so translators and developers can inspect files easily.
//! - Works across platforms by resolving file paths with native path handling.
//! - Fails predictably with typed errors when files are missing or invalid.
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
//! # Small Tutorial
//!
//! 1. Create a directory for locale files.
//! 2. Add one TOML file per locale.
//! 3. Load the locales your application needs at startup.
//! 4. Set the active locale for the current process.
//! 5. Use [`t!`] anywhere you need translated text.
//!
//! Example layout:
//!
//! ```text
//! your-app/
//! |- Cargo.toml
//! |- src/
//! |  \- main.rs
//! \- locales/
//!    |- en.toml
//!    \- es.toml
//! ```
//!
//!
//! # File Format
//!
//! Language files are plain TOML, one key per line:
//!
//! ```toml
//! bad_password = "Your password is incorrect."
//! welcome_user = "Welcome back"
//! not_found    = "The page you requested does not exist."
//! ```
//!
//! Files are resolved as `{path}/{locale}.toml`.
//!
//! # Behavior Notes
//!
//! Lookup order is deterministic:
//!
//! 1. Requested locale, or the active locale when none is provided
//! 2. Each configured fallback locale in order
//! 3. The inline fallback passed to [`t!`]
//! 4. The key itself
//!
//! Non-string TOML values are ignored on purpose. That keeps translation data
//! flat and avoids surprising coercions at runtime.
//!
//! # Examples
//!
//! See the runnable example in `examples/basic.rs` for a complete setup using
//! real locale files.
//! See `examples/server.rs` for a server-oriented pattern that resolves a
//! locale per request and passes it explicitly during translation.
//! See `examples/axum_server.rs` for the same pattern inside a real HTTP
//! handler using `axum`.
//! See `examples/actix_server.rs` for the same pattern using `actix-web`.
//! Locale names must be a single file stem such as `en`, `en-US`, or `pt_BR`.

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::missing_safety_doc)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod error;
mod intern;
mod loader;
mod request;
mod store;

pub use error::LangError;
pub use request::{resolve_accept_language, resolve_accept_language_owned};
pub use store::{Lang, Translator};

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
