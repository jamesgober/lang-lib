//! Concurrency stress tests for the lock-free read path.
//!
//! These tests run translation lookups across many threads in parallel and
//! mix reader workloads with writer workloads (load/unload/`set_*`). They
//! exist to catch any regression where the `ArcSwap` snapshot semantics or
//! the interner break under contention.

use std::io::Write;
use std::sync::Barrier;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;

use lang_lib::{Lang, Translator};
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

fn setup_locales() -> TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    let en = "\
greeting = \"Hello\"
farewell = \"Goodbye\"
profile = \"Profile\"
settings = \"Settings\"
toast_saved = \"Saved\"
network_error = \"Could not reach the server\"
login_button = \"Continue\"
";
    let es = "\
greeting = \"Hola\"
farewell = \"Adios\"
profile = \"Perfil\"
settings = \"Configuracion\"
toast_saved = \"Guardado\"
network_error = \"No pudimos conectarnos al servidor\"
login_button = \"Continuar\"
";
    let mut f = std::fs::File::create(dir.path().join("en.toml")).expect("write en.toml");
    write!(f, "{en}").expect("write en content");
    let mut f = std::fs::File::create(dir.path().join("es.toml")).expect("write es.toml");
    write!(f, "{es}").expect("write es content");
    dir
}

#[test]
fn translate_holds_under_64_thread_storm() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_locales();
    Lang::set_path(dir.path().to_str().expect("utf8 path"));
    Lang::load("en").expect("load en");
    Lang::load("es").expect("load es");
    Lang::set_fallbacks(vec!["en".to_string()]);

    const THREADS: usize = 64;
    const ITERATIONS: usize = 50_000;

    let barrier = std::sync::Arc::new(Barrier::new(THREADS));
    let mut handles = Vec::with_capacity(THREADS);

    for tid in 0..THREADS {
        let barrier = std::sync::Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let translator = Translator::new(if tid % 2 == 0 { "en" } else { "es" });
            let _ = barrier.wait();
            for i in 0..ITERATIONS {
                let key = match i % 5 {
                    0 => "greeting",
                    1 => "farewell",
                    2 => "profile",
                    3 => "settings",
                    _ => "toast_saved",
                };
                let value = translator.translate_with_fallback(key, "fallback");
                assert!(!value.is_empty(), "got empty translation for {key}");
            }
        }));
    }

    for handle in handles {
        handle.join().expect("translator thread joined");
    }
}

#[test]
fn translate_remains_consistent_during_concurrent_reload() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_locales();
    Lang::set_path(dir.path().to_str().expect("utf8 path"));
    Lang::load("en").expect("load en");
    Lang::load("es").expect("load es");
    Lang::set_fallbacks(vec!["en".to_string()]);

    const READER_THREADS: usize = 16;
    const READER_ITERATIONS: usize = 20_000;
    const WRITER_ITERATIONS: usize = 50;

    let dir_path = dir.path().to_str().expect("utf8 path").to_owned();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let writer_stop = std::sync::Arc::clone(&stop);
    let writer = thread::spawn(move || {
        let path = dir_path.as_str();
        for _ in 0..WRITER_ITERATIONS {
            Lang::load_from("en", path).expect("reload en");
            Lang::load_from("es", path).expect("reload es");
            if writer_stop.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
        }
    });

    let mut readers = Vec::with_capacity(READER_THREADS);
    for tid in 0..READER_THREADS {
        readers.push(thread::spawn(move || {
            let translator = Translator::new(if tid % 2 == 0 { "en" } else { "es" });
            for _ in 0..READER_ITERATIONS {
                let value = translator.translate_with_fallback("greeting", "fb");
                let valid = matches!(value.as_ref(), "Hello" | "Hola" | "fb");
                assert!(valid, "unexpected translation value: {value}");
            }
        }));
    }

    for reader in readers {
        reader.join().expect("reader thread joined");
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    writer.join().expect("writer thread joined");
}

#[test]
fn unload_does_not_break_concurrent_readers() {
    let _guard = test_guard();
    reset_lang();
    let dir = setup_locales();
    Lang::set_path(dir.path().to_str().expect("utf8 path"));
    Lang::load("en").expect("load en");
    Lang::load("es").expect("load es");
    Lang::set_fallbacks(vec!["en".to_string()]);

    let path = dir.path().to_str().expect("utf8 path").to_owned();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let writer_stop = std::sync::Arc::clone(&stop);
    let writer = thread::spawn(move || {
        for _ in 0..30 {
            Lang::unload("es");
            Lang::load_from("es", path.as_str()).expect("reload es");
            if writer_stop.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
        }
    });

    let mut readers = Vec::with_capacity(8);
    for _ in 0..8 {
        readers.push(thread::spawn(|| {
            let translator = Translator::new("es");
            for _ in 0..5_000 {
                let value = translator.translate_with_fallback("greeting", "Hello");
                let valid = matches!(value.as_ref(), "Hola" | "Hello");
                assert!(valid, "unexpected value during unload churn: {value}");
            }
        }));
    }

    for reader in readers {
        reader.join().expect("reader joined");
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    writer.join().expect("writer joined");
}
