//! Change events published when the translation store mutates.
//!
//! Available when the `registry` feature is enabled. Pair these types with
//! [`crate::Lang::on_change`] to receive notifications whenever a locale is
//! loaded, unloaded, or reloaded — either programmatically or by the
//! optional [`crate::Lang::watch`] file watcher (`hot-reload` feature).

/// The kind of change that produced a [`LangChangeEvent`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ChangeKind {
    /// A locale was loaded for the first time during this process.
    Loaded,
    /// A locale was reloaded (replaced) with fresh contents.
    Reloaded,
    /// A locale was removed from the lookup table.
    Unloaded,
    /// The file watcher detected a change but the file was missing or
    /// unreadable when reload was attempted.
    FileMissing,
    /// The file watcher detected a change but the file failed to parse as
    /// valid TOML.
    ParseFailed,
}

/// A change to the translation store.
///
/// `locale` is the affected locale identifier (a long-lived interned
/// reference). `kind` describes what happened.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LangChangeEvent {
    /// The locale identifier the change applies to.
    pub locale: &'static str,
    /// The kind of change.
    pub kind: ChangeKind,
}
