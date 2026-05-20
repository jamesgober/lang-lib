//! Change-event registry integration tests.
//!
//! These exercise the `registry` feature: `Lang::on_change`,
//! `Lang::off_change`, and the events emitted by `load`/`load_from`/`unload`.

#![cfg(feature = "registry")]

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

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
    for locale in Lang::loaded() {
        Lang::unload(locale);
    }
    Lang::set_path("locales");
    Lang::set_locale("en");
    Lang::set_fallbacks(vec!["en".to_string()]);
}

fn setup_locales(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for (locale, content) in files {
        let mut f = std::fs::File::create(dir.path().join(format!("{locale}.toml")))
            .expect("create locale file");
        write!(f, "{content}").expect("write locale content");
    }
    dir
}

#[test]
fn first_load_emits_loaded_event() {
    let _guard = test_guard();
    reset_lang();
    let events: Arc<Mutex<Vec<LangChangeEvent>>> = Arc::new(Mutex::new(Vec::new()));

    let sink = Arc::clone(&events);
    let id = Lang::on_change(move |event| {
        sink.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(*event);
    });

    let dir = setup_locales(&[("reg_load_a", "hello = \"Hi\"")]);
    let path = dir.path().to_str().expect("utf8 path").to_owned();
    Lang::set_path(path);
    Lang::load("reg_load_a").expect("load");

    let captured = events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(captured.len(), 1, "expected exactly one event");
    assert_eq!(captured[0].locale, "reg_load_a");
    assert_eq!(captured[0].kind, ChangeKind::Loaded);

    assert!(Lang::off_change(id));
}

#[test]
fn second_load_emits_reloaded_event() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_locales(&[("reg_reload_b", "greeting = \"Hello\"")]);
    let path = dir.path().to_str().expect("utf8 path").to_owned();
    Lang::set_path(path);
    Lang::load("reg_reload_b").expect("load 1");

    let events: Arc<Mutex<Vec<LangChangeEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let id = Lang::on_change(move |event| {
        sink.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(*event);
    });

    Lang::load("reg_reload_b").expect("load 2");

    let captured = events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].locale, "reg_reload_b");
    assert_eq!(captured[0].kind, ChangeKind::Reloaded);

    assert!(Lang::off_change(id));
}

#[test]
fn unload_emits_unloaded_event_when_locale_was_present() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_locales(&[("reg_unload_c", "a = \"b\"")]);
    let path = dir.path().to_str().expect("utf8 path").to_owned();
    Lang::set_path(path);
    Lang::load("reg_unload_c").expect("load");

    let events: Arc<Mutex<Vec<LangChangeEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let id = Lang::on_change(move |event| {
        sink.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(*event);
    });

    Lang::unload("reg_unload_c");

    let captured = events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].locale, "reg_unload_c");
    assert_eq!(captured[0].kind, ChangeKind::Unloaded);

    assert!(Lang::off_change(id));
}

#[test]
fn unload_emits_nothing_when_locale_not_present() {
    let _guard = test_guard();
    reset_lang();

    let events: Arc<Mutex<Vec<LangChangeEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let id = Lang::on_change(move |event| {
        sink.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(*event);
    });

    Lang::unload("reg_never_loaded");

    let captured = events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(
        captured.is_empty(),
        "unload of absent locale should not emit, got {captured:?}"
    );

    assert!(Lang::off_change(id));
}

#[test]
fn off_change_stops_subsequent_dispatch() {
    let _guard = test_guard();
    reset_lang();
    let count = Arc::new(AtomicUsize::new(0));

    let sink = Arc::clone(&count);
    let id = Lang::on_change(move |_| {
        let _ = sink.fetch_add(1, Ordering::Relaxed);
    });

    let dir = setup_locales(&[("reg_off_d", "k = \"v\"")]);
    let path = dir.path().to_str().expect("utf8 path").to_owned();
    Lang::set_path(path);
    Lang::load("reg_off_d").expect("load");
    assert_eq!(count.load(Ordering::Relaxed), 1);

    assert!(Lang::off_change(id));
    Lang::load("reg_off_d").expect("load again after off");
    assert_eq!(
        count.load(Ordering::Relaxed),
        1,
        "no further events should fire after off_change"
    );
}
