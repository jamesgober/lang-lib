//! String interner backing zero-allocation translation reads.
//!
//! Translation keys, values, locale identifiers, and the base path can be
//! interned into a global pool that hands out `&'static str` references.
//! Once interned a string lives for the program's lifetime, which lets the
//! translate hot path return `Cow::Borrowed(&'static str)` with zero
//! allocation.
//!
//! The pool deduplicates: interning the same byte sequence again returns
//! the same `&'static str`, so reloading an unchanged locale file does not
//! produce a second copy of its strings.
//!
//! Growth is bounded:
//!
//! - The first load of each locale interns its keys and values.
//! - Later loads (reloads) intern a new string only while the reload budget
//!   ([`RELOAD_BUDGET_BYTES`]) lasts. Past the budget, new keys and values
//!   are kept in reference-counted storage ([`Text::Shared`]) and freed when
//!   the locale is reloaded again or unloaded.
//! - Locale names that are not loaded are interned for [`crate::Translator`]
//!   and [`crate::Lang::set_locale`] only up to [`UNLOADED_LOCALE_CAP`]
//!   distinct names. Past the cap they map to the empty locale, which is
//!   never loaded.

use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use rustc_hash::FxHashSet;

/// Total bytes that reloads may add to the interner over the life of the
/// process.
pub(crate) const RELOAD_BUDGET_BYTES: usize = 1024 * 1024;

/// Distinct locale names that are not loaded and may still be interned by
/// [`crate::Translator::new`] and [`crate::Lang::set_locale`].
pub(crate) const UNLOADED_LOCALE_CAP: usize = 256;

static INTERNER: OnceLock<Mutex<FxHashSet<&'static str>>> = OnceLock::new();
static RELOAD_BYTES_USED: AtomicUsize = AtomicUsize::new(0);
static UNLOADED_LOCALES_USED: AtomicUsize = AtomicUsize::new(0);

fn pool() -> std::sync::MutexGuard<'static, FxHashSet<&'static str>> {
    INTERNER
        .get_or_init(|| Mutex::new(FxHashSet::default()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn leak_into(set: &mut FxHashSet<&'static str>, s: &str) -> &'static str {
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    let _ = set.insert(leaked);
    leaked
}

/// Returns a `&'static str` for `s`, leaking the bytes on first sight.
///
/// Subsequent calls with the same `s` return the existing reference without
/// allocating a second copy. Only use this for strings from a bounded set
/// (application configuration, first loads of locale files).
pub(crate) fn intern(s: &str) -> &'static str {
    let mut set = pool();
    if let Some(&existing) = set.get(s) {
        return existing;
    }
    leak_into(&mut set, s)
}

/// Interns `s` only if it is already interned or the cap on unloaded
/// locale names still has room.
pub(crate) fn intern_unloaded_locale(s: &str) -> Option<&'static str> {
    let mut set = pool();
    if let Some(&existing) = set.get(s) {
        return Some(existing);
    }
    // The pool lock is held, so this check and the increment cannot race.
    let used = UNLOADED_LOCALES_USED.load(Ordering::Relaxed);
    if used >= UNLOADED_LOCALE_CAP {
        return None;
    }
    UNLOADED_LOCALES_USED.store(used + 1, Ordering::Relaxed);
    Some(leak_into(&mut set, s))
}

/// A string held by a locale map: interned for the life of the process, or
/// reference counted and freed with the map that holds it.
#[derive(Clone, Debug)]
pub(crate) enum Text {
    /// Interned; never freed.
    Static(&'static str),
    /// Reference counted; freed when the last locale map holding it drops.
    Shared(Arc<str>),
}

impl Text {
    /// Returns the string.
    #[inline]
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Text::Static(s) => s,
            Text::Shared(s) => s,
        }
    }

    /// Returns the text for `s`, interning it when allowed.
    ///
    /// `first_load` is `true` while loading a locale that has never been
    /// loaded in this process; its strings are always interned. Otherwise a
    /// string already in the pool is reused, a new one is interned while the
    /// reload budget lasts, and anything past the budget is reference
    /// counted.
    pub(crate) fn new(s: &str, first_load: bool) -> Self {
        let mut set = pool();
        if let Some(&existing) = set.get(s) {
            return Text::Static(existing);
        }
        if first_load || take_reload_budget(s.len()) {
            return Text::Static(leak_into(&mut set, s));
        }
        Text::Shared(Arc::from(s))
    }
}

/// Takes `len` bytes from the reload budget. Callers hold the pool lock, so
/// the load and store below cannot interleave with another caller.
fn take_reload_budget(len: usize) -> bool {
    let used = RELOAD_BYTES_USED.load(Ordering::Relaxed);
    match used.checked_add(len) {
        Some(next) if next <= RELOAD_BUDGET_BYTES => {
            RELOAD_BYTES_USED.store(next, Ordering::Relaxed);
            true
        }
        _ => false,
    }
}

impl Borrow<str> for Text {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Hash for Text {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Must match `str`'s hash so lookups by `&str` work.
        self.as_str().hash(state);
    }
}

impl PartialEq for Text {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Text {}

#[cfg(test)]
mod tests {
    use super::{Text, intern};

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

    #[test]
    fn text_reuses_interned_string_on_reload() {
        let a = intern("text_reuse_target");
        match Text::new("text_reuse_target", false) {
            Text::Static(b) => assert!(std::ptr::eq(a.as_ptr(), b.as_ptr())),
            Text::Shared(_) => panic!("expected the interned copy"),
        }
    }

    #[test]
    fn text_hash_and_eq_match_str() {
        use std::collections::HashSet;
        let mut set: HashSet<Text> = HashSet::new();
        let _ = set.insert(Text::Shared("shared_key".into()));
        let _ = set.insert(Text::Static("static_key"));
        assert!(set.contains("shared_key"));
        assert!(set.contains("static_key"));
        assert!(!set.contains("other"));
    }
}
