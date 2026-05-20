//! Hot-reload example.
//!
//! Run with:
//!
//! ```text
//! cargo run --example hot_reload --features hot-reload
//! ```
//!
//! The example writes a temporary `en.toml`, loads it, starts the file
//! watcher, then mutates the file every second. Each change should trigger
//! a Reloaded event and the next `t!("greeting")` returns the new value.
//! Press Ctrl+C to stop.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::sleep;
use std::time::Duration;

use lang_lib::{Lang, t};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let locale_file = dir.path().join("en.toml");

    write_locale(&locale_file, "Welcome (v1)")?;

    Lang::set_path(dir.path().to_string_lossy());
    Lang::load("en")?;
    Lang::set_locale("en");

    let counter = Arc::new(AtomicUsize::new(0));

    let event_counter = Arc::clone(&counter);
    let _handler = Lang::on_change(move |event| {
        let n = event_counter.fetch_add(1, Ordering::Relaxed) + 1;
        println!("[event {n:>2}] {:?} -> {}", event.kind, event.locale);
    });

    Lang::watch(dir.path())?;
    println!("watcher started; press Ctrl+C to stop");
    println!("initial value: {}", t!("greeting"));

    for i in 2..=10 {
        sleep(Duration::from_secs(1));
        let next = format!("Welcome (v{i})");
        write_locale(&locale_file, &next)?;
        // Allow OS event delivery + 150 ms debounce.
        sleep(Duration::from_millis(300));
        println!("after rewrite #{i}: {}", t!("greeting"));
    }

    Lang::unwatch();
    Ok(())
}

fn write_locale(path: &PathBuf, greeting: &str) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "greeting = \"{greeting}\"")?;
    f.sync_all()?;
    Ok(())
}
