//! Heap-growth checks for paths that take outside input.
//!
//! A counting global allocator tracks live heap bytes. Every test holds the
//! same mutex so readings are not disturbed by other tests in this binary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use lang_lib::{Lang, Translator};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    // Allocation counts only include the thread that set this, so the test
    // harness printing results on another thread does not add noise.
    static COUNT_ALLOCS: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: forwards every call to the system allocator unchanged and only
// updates two counters on the side.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        if COUNT_ALLOCS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn guard() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

#[test]
fn test_translator_new_distinct_locales_heap_stays_bounded() {
    let _g = guard();
    // Warm up any lazily created global state first.
    let _ = Translator::new("en").translate("k");
    let before = live();
    for i in 0..20_000u32 {
        let t = Translator::new(format!("x{i:08}"));
        let _ = t.translate("k");
    }
    let grown = live() - before;
    assert!(
        grown < 256 * 1024,
        "Translator::new with 20000 distinct locales grew the heap by {grown} bytes"
    );
}

#[test]
fn test_set_locale_distinct_locales_heap_stays_bounded() {
    let _g = guard();
    Lang::set_locale("en");
    let before = live();
    for i in 0..20_000u32 {
        Lang::set_locale(format!("y{i:08}"));
    }
    Lang::set_locale("en");
    let grown = live() - before;
    assert!(
        grown < 256 * 1024,
        "set_locale with 20000 distinct locales grew the heap by {grown} bytes"
    );
}

#[test]
fn test_reload_changed_values_heap_stays_bounded() {
    let _g = guard();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("utf8 path").to_owned();
    let file = dir.path().join("mem_values.toml");
    std::fs::write(&file, "greeting = \"v0\"\n").expect("write");
    Lang::load_from("mem_values", &path).expect("first load");
    let filler = "x".repeat(2000);
    let before = live();
    for i in 0..1500u32 {
        std::fs::write(&file, format!("greeting = \"{filler}{i}\"\n")).expect("write");
        Lang::load_from("mem_values", &path).expect("reload");
    }
    assert!(Lang::translate("greeting", Some("mem_values"), None).ends_with("1499"));
    Lang::unload("mem_values");
    let grown = live() - before;
    // 1500 reloads of a 2 KB value would leak about 3 MB without a bound.
    assert!(
        grown < 2 * 1024 * 1024,
        "1500 reloads with a changed 2 KB value grew the heap by {grown} bytes"
    );
}

#[test]
fn test_reload_new_keys_heap_stays_bounded() {
    let _g = guard();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("utf8 path").to_owned();
    let file = dir.path().join("mem_keys.toml");
    std::fs::write(&file, "greeting = \"v0\"\n").expect("write");
    Lang::load_from("mem_keys", &path).expect("first load");
    let filler = "k".repeat(2000);
    let before = live();
    for i in 0..1500u32 {
        std::fs::write(&file, format!("{filler}{i} = \"v\"\n")).expect("write");
        Lang::load_from("mem_keys", &path).expect("reload");
    }
    let last = format!("{filler}1499");
    assert_eq!(Lang::translate(&last, Some("mem_keys"), None), "v");
    Lang::unload("mem_keys");
    let grown = live() - before;
    assert!(
        grown < 2 * 1024 * 1024,
        "1500 reloads with a new 2 KB key grew the heap by {grown} bytes"
    );
}

#[test]
fn test_fallback_miss_path_does_not_allocate() {
    let _g = guard();
    Lang::set_fallbacks(vec!["en".to_string(), "de".to_string()]);
    let _ = Lang::translate("no_such_key", Some("fr"), Some("x"));
    COUNT_ALLOCS.with(|c| c.set(true));
    let start = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..1000 {
        let v = Lang::translate("no_such_key", Some("fr"), Some("x"));
        let _ = std::hint::black_box(&v);
    }
    let n = ALLOCS.load(Ordering::Relaxed) - start;
    COUNT_ALLOCS.with(|c| c.set(false));
    Lang::set_fallbacks(vec!["en".to_string()]);
    assert_eq!(n, 0, "1000 fallback misses made {n} heap allocations");
}
