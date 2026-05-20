//! Smoke test for the `hot-reload` feature.
//!
//! Exercises the full path: start watcher → modify a `.toml` file → wait for
//! debounce → assert the file's new contents are visible via `Lang::translate`
//! and that a Reloaded change event fires.
//!
//! These tests touch real filesystem watchers (inotify / FSEvents /
//! ReadDirectoryChangesW) and are inherently timing-sensitive. They are
//! gated behind the `hot-reload` feature so the standard test run is not
//! affected.

#![cfg(feature = "hot-reload")]

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread::sleep;
use std::time::Duration;

use lang_lib::{ChangeKind, Lang, LangChangeEvent};
use tempfile::TempDir;

fn test_guard() -> MutexGuard<'static, ()> {
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn reset_lang() {
    Lang::unwatch();
    for locale in Lang::loaded() {
        Lang::unload(locale);
    }
    Lang::set_path("locales");
    Lang::set_locale("en");
    Lang::set_fallbacks(vec!["en".to_string()]);
}

fn write_locale(dir: &TempDir, locale: &str, content: &str) {
    let mut f = std::fs::File::create(dir.path().join(format!("{locale}.toml")))
        .expect("create locale file");
    write!(f, "{content}").expect("write locale content");
    f.sync_all().expect("sync locale file");
}

#[test]
fn watcher_picks_up_file_changes_and_fires_reloaded() {
    let _guard = test_guard();
    reset_lang();

    let dir = tempfile::tempdir().expect("tempdir");
    write_locale(&dir, "watch_a", "greeting = \"v1\"");
    let path_str = dir.path().to_str().expect("utf8 path").to_owned();
    Lang::set_path(path_str.clone());
    Lang::load("watch_a").expect("initial load");
    assert_eq!(Lang::translate("greeting", Some("watch_a"), None), "v1");

    let events: Arc<Mutex<Vec<LangChangeEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let id = Lang::on_change(move |event| {
        sink.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(*event);
    });

    Lang::watch(&path_str).expect("start watcher");

    // Give the watcher a moment to subscribe before the first write.
    sleep(Duration::from_millis(200));
    write_locale(&dir, "watch_a", "greeting = \"v2\"");

    // Wait long enough for the OS to deliver the event + our 150 ms debounce
    // + reload work. 2 seconds is a generous upper bound on slow CI runners.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if Lang::translate("greeting", Some("watch_a"), None) == "v2" {
            break;
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "watcher did not pick up change within deadline; current value is {}",
                Lang::translate("greeting", Some("watch_a"), None)
            );
        }
        sleep(Duration::from_millis(50));
    }

    let captured = events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(
        captured
            .iter()
            .any(|e| e.locale == "watch_a" && e.kind == ChangeKind::Reloaded),
        "expected a Reloaded event for watch_a, got {captured:?}"
    );

    Lang::unwatch();
    assert!(Lang::off_change(id));
}

#[test]
fn unwatch_stops_event_delivery() {
    let _guard = test_guard();
    reset_lang();

    let dir = tempfile::tempdir().expect("tempdir");
    write_locale(&dir, "watch_b", "k = \"a\"");
    let path_str = dir.path().to_str().expect("utf8 path").to_owned();
    Lang::set_path(path_str.clone());
    Lang::load("watch_b").expect("load");

    let count = Arc::new(AtomicUsize::new(0));
    let sink = Arc::clone(&count);
    let id = Lang::on_change(move |_| {
        let _ = sink.fetch_add(1, Ordering::Relaxed);
    });

    Lang::watch(&path_str).expect("start watcher");
    sleep(Duration::from_millis(200));
    Lang::unwatch();

    let before = count.load(Ordering::Relaxed);
    write_locale(&dir, "watch_b", "k = \"b\"");
    sleep(Duration::from_millis(500));
    let after = count.load(Ordering::Relaxed);

    assert_eq!(
        before, after,
        "no events should fire after Lang::unwatch (before={before}, after={after})"
    );

    assert!(Lang::off_change(id));
}
