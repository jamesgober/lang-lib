//! Filesystem watcher that reloads locale files on change.
//!
//! Available when the `hot-reload` feature is enabled. Calling
//! [`crate::Lang::watch`] starts a single background thread that subscribes
//! to filesystem events for the configured locale directory, debounces
//! rapid bursts (e.g. atomic-rename writes), and calls
//! [`crate::Lang::load_from`] for each affected `<locale>.toml` file.
//!
//! Successful reloads fire a [`crate::ChangeKind::Reloaded`] event through
//! the shared `registry-io` channel; missing files or parse failures fire
//! [`crate::ChangeKind::FileMissing`] or [`crate::ChangeKind::ParseFailed`].
//! Wire your application up via [`crate::Lang::on_change`].
//!
//! The watcher is **not** intended for build-time hot-swap UX; it is a
//! production primitive for long-running services that need to pick up
//! translation edits without a restart.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::change::{ChangeKind, LangChangeEvent};
use crate::error::LangError;
use crate::intern::intern;
use crate::registry::emit;
use crate::store::Lang;

/// Errors emitted by [`crate::Lang::watch`] when starting the watcher.
#[derive(Debug)]
pub enum WatchError {
    /// The configured locale directory could not be observed.
    Io(notify::Error),
    /// A watcher is already running. Call [`crate::Lang::unwatch`] first.
    AlreadyRunning,
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::Io(e) => write!(f, "failed to start watcher: {e}"),
            WatchError::AlreadyRunning => {
                f.write_str("a watcher is already running; call Lang::unwatch first")
            }
        }
    }
}

impl std::error::Error for WatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WatchError::Io(e) => Some(e),
            WatchError::AlreadyRunning => None,
        }
    }
}

impl From<notify::Error> for WatchError {
    fn from(e: notify::Error) -> Self {
        WatchError::Io(e)
    }
}

const DEBOUNCE: Duration = Duration::from_millis(150);

struct WatchState {
    _watcher: RecommendedWatcher,
    handle: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

static WATCH: Mutex<Option<WatchState>> = Mutex::new(None);

pub(crate) fn start(dir: PathBuf) -> Result<(), WatchError> {
    let mut slot = WATCH.lock().unwrap_or_else(PoisonError::into_inner);
    if slot.is_some() {
        return Err(WatchError::AlreadyRunning);
    }

    let (tx, rx) = mpsc::channel::<Result<notify::Event, notify::Error>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(&dir, RecursiveMode::NonRecursive)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    let handle = thread::Builder::new()
        .name("lang-lib-watcher".into())
        .spawn(move || worker(rx, stop_thread, dir))
        .map_err(|e| WatchError::Io(notify::Error::generic(&e.to_string())))?;

    *slot = Some(WatchState {
        _watcher: watcher,
        handle: Some(handle),
        stop,
    });
    Ok(())
}

pub(crate) fn stop() {
    let mut slot = WATCH.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(mut state) = slot.take() {
        state.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = state.handle.take() {
            let _ = handle.join();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "worker is spawned in a thread and must own its inputs"
)]
fn worker(
    rx: mpsc::Receiver<Result<notify::Event, notify::Error>>,
    stop: Arc<AtomicBool>,
    dir: PathBuf,
) {
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }

        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if !is_toml_file(&path) {
                        continue;
                    }
                    let _ = pending.insert(path, Instant::now());
                }
            }
            Ok(Err(_)) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        let now = Instant::now();
        let mut ready: Vec<PathBuf> = Vec::new();
        pending.retain(|p, ts| {
            if now.duration_since(*ts) >= DEBOUNCE {
                ready.push(p.clone());
                false
            } else {
                true
            }
        });

        for path in ready {
            reload(&path, &dir);
        }
    }
}

fn is_toml_file(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("toml")
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| !s.is_empty())
}

fn reload(path: &std::path::Path, fallback_dir: &std::path::Path) {
    let Some(locale) = path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let dir = path.parent().unwrap_or(fallback_dir);
    let dir_str = dir.to_string_lossy();
    match Lang::load_from(locale, &dir_str) {
        // Ok: Lang::load_from already emits Loaded/Reloaded via the
        // registry feature (implied by hot-reload).
        // NotLoaded/InvalidLocale: locale rejected before file access —
        // a developer-side bug, not a runtime change. No event.
        Ok(()) | Err(LangError::NotLoaded { .. } | LangError::InvalidLocale { .. }) => {}
        Err(LangError::Io { .. }) => {
            emit(LangChangeEvent {
                locale: intern(locale),
                kind: ChangeKind::FileMissing,
            });
        }
        Err(LangError::Parse { .. }) => {
            emit(LangChangeEvent {
                locale: intern(locale),
                kind: ChangeKind::ParseFailed,
            });
        }
    }
}
