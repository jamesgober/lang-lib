//! Append-only string interner backing zero-allocation translation reads.
//!
//! Translation keys, values, locale identifiers, and the base path are
//! interned into a global pool that hands out `&'static str` references.
//! Once interned a string lives for the program's lifetime, which lets the
//! translate hot path return `Cow::Borrowed(&'static str)` with zero
//! allocation.
//!
//! The pool deduplicates: re-interning the same byte sequence returns the
//! same `&'static str`, so reloading a locale file does not produce
//! duplicate copies of its strings. Memory growth is therefore bounded by
//! the count of *unique* strings ever seen — typically a few thousand for
//! a fully populated multi-locale application.
//!
//! Trade-off: [`crate::Lang::unload`] removes the locale's lookup map but
//! does **not** reclaim the interned string bytes. This is the intended
//! design for `1.x`; the `1.2.0` hot-reload milestone will revisit so that
//! large rolling rewrites do not grow the interner without bound.

use std::sync::{Mutex, OnceLock, PoisonError};

use rustc_hash::FxHashSet;

static INTERNER: OnceLock<Mutex<FxHashSet<&'static str>>> = OnceLock::new();

/// Returns a `&'static str` for `s`, leaking the bytes on first sight.
///
/// Subsequent calls with the same `s` return the existing reference without
/// allocating a second copy. The function is fully thread-safe and is the
/// canonical way to obtain stable references for translation data.
pub(crate) fn intern(s: &str) -> &'static str {
    let interner = INTERNER.get_or_init(|| Mutex::new(FxHashSet::default()));
    let mut set = interner.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    let _ = set.insert(leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::intern;

    #[test]
    fn interning_returns_static_reference() {
        let s = intern("hello");
        assert_eq!(s, "hello");
    }

    #[test]
    fn interning_deduplicates_identical_strings() {
        let a = intern("dedupe_target");
        let b = intern("dedupe_target");
        assert!(std::ptr::eq(a.as_ptr(), b.as_ptr()));
    }

    #[test]
    fn interning_distinct_strings_returns_distinct_pointers() {
        let a = intern("alpha_unique");
        let b = intern("beta_unique");
        assert!(!std::ptr::eq(a.as_ptr(), b.as_ptr()));
    }
}
