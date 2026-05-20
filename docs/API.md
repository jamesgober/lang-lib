# lang-lib - API Reference

> Authoritative reference for the public API of `lang-lib 1.3.0`.
> Mirrors the rustdoc on docs.rs. The surface listed here is **stable**;
> see [Stability](#stability) below for the guarantee.

## Contents

- [Crate root](#crate-root)
- [`lang_lib::Lang`](#lang_liblang)
  - [Path & locale configuration](#path--locale-configuration)
  - [Loading & unloading](#loading--unloading)
  - [Querying loaded state](#querying-loaded-state)
  - [Translation](#translation)
  - [Change notifications (`registry`)](#change-notifications-registry)
  - [Filesystem watcher (`hot-reload`)](#filesystem-watcher-hot-reload)
- [`lang_lib::Translator`](#lang_libtranslator)
- [`lang_lib::LangError`](#lang_liblangerror)
- [Macros](#macros)
- [Request helpers](#request-helpers)
- [Change events (`registry`)](#change-events-registry)
- [`lang_lib::WatchError` (`hot-reload`)](#lang_libwatcherror-hot-reload)
- [Feature flags](#feature-flags)
- [MSRV](#msrv)
- [Runtime dependencies](#runtime-dependencies)
- [Stability](#stability)
- [Memory model](#memory-model)
- [Performance](#performance)
- [Quick example](#quick-example)
- [Hot-reload example](#hot-reload-example)

## Crate root

| Item                                       | Kind   | Feature       | Notes                                              |
|--------------------------------------------|--------|---------------|----------------------------------------------------|
| `lang_lib::Lang`                           | struct | always        | Process-global translation store (zero-sized facade). |
| `lang_lib::Translator`                     | struct | always        | Request-scoped translation helper. `Copy`.         |
| `lang_lib::LangError`                      | enum   | always        | All errors produced by the crate.                  |
| `lang_lib::t!`                             | macro  | always        | The standard translation call.                     |
| `lang_lib::resolve_accept_language`        | fn     | always        | Parse `Accept-Language` header into one of your supported locales. |
| `lang_lib::resolve_accept_language_owned`  | fn     | always        | Same, but supported list is `&[impl AsRef<str>]`.  |
| `lang_lib::LangChangeEvent`                | struct | `registry`    | Change event emitted on load / reload / unload.    |
| `lang_lib::ChangeKind`                     | enum   | `registry`    | Kind of change: Loaded / Reloaded / Unloaded / FileMissing / ParseFailed. |
| `lang_lib::HandlerId`                      | alias  | `registry`    | Re-export of `registry_io::HandlerId`.             |
| `lang_lib::WatchError`                     | enum   | `hot-reload`  | Errors returned by `Lang::watch`.                  |

## `lang_lib::Lang`

```rust
pub struct Lang;
```

A zero-sized facade over the process-global translation state. All
methods are associated functions; there is no instance to construct.

Concurrent calls to read-side methods (`Lang::translate`,
`Lang::path`, `Lang::locale`, `Lang::loaded`, `Lang::is_loaded`) never
acquire a lock. They take an `ArcSwap` snapshot of the underlying
state and read from it. Write-side methods (`Lang::set_*`,
`Lang::load`, `Lang::load_from`, `Lang::unload`) briefly serialize
against each other via a private mutex but never block readers.

### Path & locale configuration

```rust
impl Lang {
    pub fn set_path(path: impl AsRef<str>);
    pub fn path() -> &'static str;
    pub fn set_locale(locale: impl AsRef<str>);
    pub fn locale() -> &'static str;
    pub fn set_fallbacks(chain: Vec<String>);
}
```

`set_path` and `set_locale` accept any `AsRef<str>` — `&str`,
`String`, `Cow<str>`, and `Path::display()` output all satisfy the
bound. Each call interns the value into the process-wide pool
exactly once; subsequent calls with the same string return the same
`'static` reference.

`set_fallbacks` accepts `Vec<String>` and interns each element. The
chain is checked in order when a key is missing in the requested
locale; duplicates are deduplicated at lookup time, so a chain of
`["en", "en", "en"]` behaves identically to `["en"]`.

**Example:**

```rust,no_run
use lang_lib::Lang;

Lang::set_path("assets/locales");
Lang::set_locale("en");
Lang::set_fallbacks(vec!["en".to_string()]);

assert_eq!(Lang::path(), "assets/locales");
assert_eq!(Lang::locale(), "en");
```

### Loading & unloading

```rust
impl Lang {
    pub fn load(locale: impl AsRef<str>) -> Result<(), LangError>;
    pub fn load_from(locale: impl AsRef<str>, path: &str) -> Result<(), LangError>;
    pub fn unload(locale: &str);
}
```

`load` reads `{path}/{locale}.toml` (where `{path}` is whatever was
set via `Lang::set_path`). `load_from` ignores the configured path
and reads from the supplied directory instead — useful when one
locale lives in a different tree from the rest. `unload` removes a
locale from the lookup table.

Locale identifiers must be single file stems such as `en`, `en-US`,
or `pt_BR`. Path separators and relative path components are
rejected before any file access.

Loading the same locale a second time replaces its translations
with a fresh load from disk. Under the `registry` feature this fires
`ChangeKind::Reloaded`; the first load fires `ChangeKind::Loaded`.

**Example:**

```rust,no_run
use lang_lib::Lang;

Lang::set_path("locales");
Lang::load("en")?;
Lang::load("es")?;

// One-off path for a single locale.
Lang::load_from("ja", "translations/ja-pack")?;

Lang::unload("ja");
# Ok::<(), lang_lib::LangError>(())
```

### Querying loaded state

```rust
impl Lang {
    pub fn is_loaded(locale: &str) -> bool;
    pub fn loaded() -> Vec<&'static str>;
}
```

`loaded` returns a sorted list of all currently loaded locale
identifiers. Sorting keeps diagnostics and tests deterministic.

**Example:**

```rust,no_run
use lang_lib::Lang;

Lang::load_from("es", "tests/fixtures/locales")?;
Lang::load_from("en", "tests/fixtures/locales")?;

assert!(Lang::is_loaded("en"));
assert_eq!(Lang::loaded(), vec!["en", "es"]);
# Ok::<(), lang_lib::LangError>(())
```

### Translation

```rust
impl Lang {
    pub fn translate<'a>(
        key: &'a str,
        locale: Option<&'a str>,
        fallback: Option<&'a str>,
    ) -> Cow<'a, str>;

    pub fn translator(locale: impl AsRef<str>) -> Translator;

    // hot-reload feature only
    pub fn translate_arc(
        key: &str,
        locale: Option<&str>,
        fallback: Option<&str>,
    ) -> Arc<str>;
}
```

`translate` is the underlying lookup function called by the `t!`
macro.

Lookup order:

1. Requested locale (or active locale when `locale` is `None`)
2. Each locale in the fallback chain, in order
3. The `fallback` argument if `Some`
4. The `key` itself (never returns an empty string)

Return type contract:

| Build         | Hit path                              | Fallback / miss path                    | Allocates per call?           |
|---------------|---------------------------------------|------------------------------------------|-------------------------------|
| Default       | `Cow::Borrowed(&'static str)`         | `Cow::Borrowed(&'a str)` of input        | **No.**                       |
| `hot-reload`  | `Cow::Owned(String)` from `Arc<str>`  | `Cow::Borrowed(&'a str)` of input        | Hit-path yes; miss-path no.   |

`translate_arc` (available only when `hot-reload` is enabled) avoids
the per-call `String` allocation by returning the underlying
`Arc<str>` directly — the hit path is a refcount bump (zero alloc).
Trade-off: many threads cloning the same `Arc<str>` simultaneously
contend on the refcount cache line. Use when you measured the
allocation cost as a hot spot and same-key contention is low.

**Example:**

```rust,no_run
use lang_lib::Lang;

Lang::load_from("en", "tests/fixtures/locales")?;
let text = Lang::translate("welcome", Some("en"), Some("Welcome"));
assert_eq!(text, "Welcome");

// Display / format / equality all work with Cow:
println!("{text}");
assert!(text.starts_with("Wel"));
# Ok::<(), lang_lib::LangError>(())
```

`translator` is a convenience for `Translator::new(locale)`:

```rust,no_run
use lang_lib::Lang;

let translator = Lang::translator("es");
assert_eq!(translator.locale(), "es");
```

### Change notifications (`registry`)

```rust
impl Lang {
    pub fn on_change<F>(handler: F) -> HandlerId
    where
        F: Fn(&LangChangeEvent) + Send + Sync + 'static;

    pub fn off_change(id: HandlerId) -> bool;
}
```

Available when the `registry` feature is enabled. Installs a handler
that fires whenever the translation store changes (load, reload,
unload, or — under `hot-reload` — a file-watch failure).

Handlers fire inline on the thread that produced the change. The
dispatch is lock-free and panic-isolating: a panic in one handler
does not stop sibling handlers from running and does not propagate
to the caller.

`off_change` returns `true` if a handler with the given id was
present and removed, `false` if it was already gone.

**Example:**

```rust,ignore
use lang_lib::{ChangeKind, Lang};

let id = Lang::on_change(|event| {
    match event.kind {
        ChangeKind::Loaded   => println!("loaded {}", event.locale),
        ChangeKind::Reloaded => println!("reloaded {}", event.locale),
        ChangeKind::Unloaded => println!("unloaded {}", event.locale),
        _ => {}
    }
});

Lang::load_from("en", "tests/fixtures/locales").unwrap();
let _ = Lang::off_change(id);
```

### Filesystem watcher (`hot-reload`)

```rust
impl Lang {
    pub fn watch(dir: impl AsRef<Path>) -> Result<(), WatchError>;
    pub fn unwatch();
}
```

Available when the `hot-reload` feature is enabled (which also
enables `registry`). Starts a background thread that subscribes to
filesystem events on `dir`, debounces rapid bursts (~150 ms per
file), and atomically reloads each affected `<locale>.toml`.
Reloads fire `ChangeKind::Reloaded` events through the registry.

Only one watcher may be active per process. Call `unwatch` before
starting a new one. `unwatch` is idempotent.

**Example:**

```rust,ignore
use lang_lib::Lang;

Lang::set_path("locales");
Lang::load("en").unwrap();

Lang::watch("locales").unwrap();
// application runs; edits to locales/*.toml are picked up automatically
Lang::unwatch();
```

## `lang_lib::Translator`

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Translator { /* interned locale */ }

impl Translator {
    pub fn new(locale: impl AsRef<str>) -> Self;
    pub fn locale(&self) -> &'static str;

    pub fn translate<'a>(&self, key: &'a str) -> Cow<'a, str>;
    pub fn translate_with_fallback<'a>(&self, key: &'a str, fallback: &'a str) -> Cow<'a, str>;

    // hot-reload feature only
    pub fn translate_arc(&self, key: &str) -> Arc<str>;
    pub fn translate_arc_with_fallback(&self, key: &str, fallback: &str) -> Arc<str>;
}
```

A request-scoped translation helper that binds a locale once and
forwards lookups to the global `Lang` store. The locale is held as a
`&'static str` (interned), so `Translator` is `Copy` — passing it
across function boundaries is a pointer copy.

**When to use:** request-driven services where locale is part of the
input rather than process-global state. Using `Translator` instead
of mutating `Lang::set_locale` per request keeps requests independent
and avoids cross-request coupling.

**Example:**

```rust,no_run
use lang_lib::{Lang, Translator};

Lang::load_from("en", "tests/fixtures/locales").unwrap();
Lang::load_from("es", "tests/fixtures/locales").unwrap();

fn render(locale: &str) -> String {
    let translator = Translator::new(locale);
    format!(
        "{} / {}",
        translator.translate("welcome"),
        translator.translate_with_fallback("missing_key", "Default"),
    )
}

println!("{}", render("en"));
println!("{}", render("es"));
```

## `lang_lib::LangError`

```rust
#[derive(Debug)]
pub enum LangError {
    Io           { locale: String, cause: String },
    Parse        { locale: String, cause: String },
    NotLoaded    { locale: String },
    InvalidLocale { locale: String },
}

impl std::fmt::Display for LangError { /* ... */ }
impl std::error::Error    for LangError { /* ... */ }
```

| Variant         | When                                                                                  |
|-----------------|---------------------------------------------------------------------------------------|
| `Io`            | The locale file could not be read (missing, permission denied, etc.).                 |
| `Parse`         | The file was read but is not valid TOML, or contained no string-typed top-level keys. |
| `NotLoaded`     | A locale was looked up that had never been loaded (reserved; future use).             |
| `InvalidLocale` | The locale identifier was rejected before file access (path separators, traversal).   |

**Example:**

```rust,no_run
use lang_lib::{Lang, LangError};

match Lang::load("en") {
    Ok(()) => {}
    Err(LangError::Io { locale, cause }) => {
        eprintln!("could not read {locale}: {cause}");
    }
    Err(LangError::Parse { locale, cause }) => {
        eprintln!("invalid TOML in {locale}: {cause}");
    }
    Err(LangError::InvalidLocale { locale }) => {
        eprintln!("rejected invalid locale identifier: {locale}");
    }
    Err(LangError::NotLoaded { locale }) => {
        eprintln!("locale was expected but not loaded: {locale}");
    }
}
```

## Macros

### `t!`

```rust
t!("key");                                      // active locale
t!("key", "es");                                // specific locale
t!("key", fallback: "Default");                 // inline fallback
t!("key", "es", fallback: "Default");           // both
```

Expands to a call to `Lang::translate`. Returns whatever `translate`
returns — a `Cow<'_, str>` whose lifetime ties to the input strings.

**Example:**

```rust,no_run
use lang_lib::{Lang, t};

Lang::set_path("locales");
Lang::load("en").unwrap();
Lang::load("es").unwrap();
Lang::set_locale("en");

assert_eq!(t!("greeting"), "Hello");
assert_eq!(t!("greeting", "es"), "Hola");
assert_eq!(t!("missing", fallback: "Default"), "Default");
assert_eq!(t!("missing", "es", fallback: "Hola"), "Hola");
```

## Request helpers

### `resolve_accept_language`

```rust
pub fn resolve_accept_language<'a>(
    header: &str,
    supported_locales: &[&'a str],
    default_locale: &'a str,
) -> &'a str;
```

Parses an HTTP `Accept-Language` header and returns the best match
from `supported_locales`, or `default_locale` if nothing matches.
Quality values (`q=…`) and primary-language matches (`es-ES → es`)
are honored.

**Example:**

```rust
use lang_lib::resolve_accept_language;

let locale = resolve_accept_language(
    "es-ES,es;q=0.9,en;q=0.8",
    &["en", "es"],
    "en",
);
assert_eq!(locale, "es");
```

### `resolve_accept_language_owned`

```rust
pub fn resolve_accept_language_owned<S>(
    header: &str,
    supported_locales: &[S],
    default_locale: &str,
) -> String
where
    S: AsRef<str>;
```

The owned variant for when your supported locale list is built at
runtime (`Vec<String>`, `&[Cow<str>]`, etc.) rather than a static
array.

**Example:**

```rust
use lang_lib::resolve_accept_language_owned;

let supported = vec!["en".to_string(), "es".to_string()];
let locale = resolve_accept_language_owned(
    "es-MX,es;q=0.9,en;q=0.7",
    &supported,
    "en",
);
assert_eq!(locale, "es");
```

## Change events (`registry`)

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LangChangeEvent {
    pub locale: &'static str,
    pub kind: ChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ChangeKind {
    Loaded,
    Reloaded,
    Unloaded,
    FileMissing,
    ParseFailed,
}

pub type HandlerId = registry_io::HandlerId;
```

| Variant       | Emitted when                                                                          |
|---------------|---------------------------------------------------------------------------------------|
| `Loaded`      | `Lang::load` / `Lang::load_from` added a previously unknown locale.                   |
| `Reloaded`    | `Lang::load` / `Lang::load_from` replaced an already-loaded locale's contents.        |
| `Unloaded`    | `Lang::unload` removed a previously loaded locale.                                    |
| `FileMissing` | (`hot-reload`) The watcher saw a change but the file disappeared before reload could read it. |
| `ParseFailed` | (`hot-reload`) The watcher reloaded a file but it no longer parses as valid TOML.     |

`HandlerId` is a re-export of `registry_io::HandlerId` so callers do
not need a direct dependency on `registry-io`.

## `lang_lib::WatchError` (`hot-reload`)

```rust
#[derive(Debug)]
pub enum WatchError {
    Io(notify::Error),
    AlreadyRunning,
}

impl std::fmt::Display for WatchError { /* ... */ }
impl std::error::Error    for WatchError { /* ... */ }
impl From<notify::Error>  for WatchError { /* ... */ }
```

| Variant          | When                                                                                  |
|------------------|---------------------------------------------------------------------------------------|
| `Io`             | The underlying `notify` watcher failed to subscribe to filesystem events.             |
| `AlreadyRunning` | `Lang::watch` was called while another watcher was already active. Call `Lang::unwatch` first. |

## Feature flags

| Flag                  | Default | Effect                                                                                          |
|-----------------------|---------|-------------------------------------------------------------------------------------------------|
| `registry`            | off     | Adds `Lang::on_change` / `Lang::off_change` and the `LangChangeEvent` / `ChangeKind` / `HandlerId` re-exports. Pulls in `registry-io = "1"`. |
| `hot-reload`          | off     | Implies `registry`. Adds `Lang::watch` / `Lang::unwatch` and `WatchError`. **Changes the value-storage strategy from interner (`&'static str`) to `Arc<str>`** so reloaded files do not leak. Pulls in `notify = "6"`. |
| `web-example-axum`    | off     | Compiles the `axum_server` example. Pulls in `axum` and `tokio`.                                |
| `web-example-actix`   | off     | Compiles the `actix_server` example. Pulls in `actix-web`.                                      |

## MSRV

`1.85` (required by edition 2024). Locked from `1.0.1` onward; a
bump requires a minor version increment and a CHANGELOG entry
under `### Changed`.

## Runtime dependencies

| Dependency     | Always pulled?      | Why                                                              |
|----------------|---------------------|------------------------------------------------------------------|
| `arc-swap`     | yes                 | Lock-free `LangState` snapshots on the translate read path.      |
| `rustc-hash`   | yes                 | `FxHashMap` — faster hashing for short translation keys.         |
| `toml`         | yes                 | Locale file parser.                                              |
| `registry-io`  | with `registry`     | Sub-microsecond change-event dispatch.                           |
| `notify`       | with `hot-reload`   | Cross-platform filesystem watcher (inotify / FSEvents / RDCW).   |

## Stability

From `1.0.0` forward, the public surface above is stable.

- Patch releases (`1.x.y`) ship bug fixes, doc improvements, and
  internal optimizations that do not change observable behavior.
- Minor releases (`1.x.0`) add to the surface but never remove or
  rename. New items may be feature-gated.
- A `2.0` would only be cut for a deliberate breaking change to the
  surface; there is no such release planned.

One historical breakage to be aware of when migrating from
pre-`1.1.0`: `Lang::translate` returned `String` in `1.0.x` and
returns `Cow<'a, str>` from `1.1.0` onward. `format!`, `println!`,
`assert_eq!(_, "literal")`, and `&str`-deref use continue to work.
Code that explicitly typed `String` for the return needs
`.into_owned()`.

## Memory model

`lang-lib` interns short strings (locale identifiers, translation
keys, the configured path, fallback locale names) into a global
append-only pool. Once interned, a string lives for the program's
lifetime. The interner is bounded by the number of *unique* strings
the application ever sees — typically a few hundred KB in real
multi-locale apps.

**Translation values** use one of two storage strategies depending
on the active feature set:

- **Default builds** intern values too. The translate hit path
  returns `Cow::Borrowed(&'static str)` — pure pointer copy, zero
  allocation. Values are never reclaimed; this is correct because
  the default build cannot reload locale data after startup, so the
  interner cannot grow over time.

- **`hot-reload` builds** store values as `Arc<str>` instead. The
  translate hit path returns `Cow::Owned(String)` — one allocation
  per call, but reloading a locale drops the old `Arc<str>`
  instances cleanly. No leak under reload churn.

`Lang::translate_arc` (under `hot-reload`) returns the underlying
`Arc<str>` directly — zero allocation per call at the cost of
refcount cache-line contention when many threads hit the same key
concurrently.

## Performance

The Criterion suite under `benches/performance.rs` measures:

- `resolve_accept_language` — `Accept-Language` header parsing
- `translate_lookup` — single-thread hit
- `translate_fallback_chain_miss` — hit via the fallback chain
- `translate_complete_miss_inline_fallback` — miss returning the inline fallback
- `translate_complete_miss_key_return` — miss returning the key itself
- `translate_hit_concurrent` — single-key hit scaled across 1, 4, 16, 64 threads

Run with:

```text
cargo bench --bench performance
```

Numbers vary by hardware; the CI workflow uploads Criterion HTML
reports as artifacts on every `main` push. See [`BENCHMARKS.md`](../BENCHMARKS.md) for methodology.

## Quick example

```rust,no_run
use lang_lib::{t, Lang};

// Configure once at startup.
Lang::set_path("locales");
Lang::load("en")?;
Lang::load("es")?;
Lang::set_fallbacks(vec!["en".to_string()]);
Lang::set_locale("en");

// Translate anywhere.
let msg     = t!("bad_password");
let msg_es  = t!("bad_password", "es");
let msg_fb  = t!("missing_key", fallback: "Default message");
let msg_lfb = t!("missing_key", "es", fallback: "Default es");

println!("{msg} / {msg_es} / {msg_fb} / {msg_lfb}");
# Ok::<(), lang_lib::LangError>(())
```

## Hot-reload example

```rust,ignore
use lang_lib::{ChangeKind, Lang, t};

Lang::set_path("locales");
Lang::load("en").unwrap();
Lang::set_locale("en");

let _handler_id = Lang::on_change(|event| {
    if event.kind == ChangeKind::Reloaded {
        println!("{} reloaded from disk", event.locale);
    }
});

Lang::watch("locales").unwrap();

// application runs; t!("greeting") returns the latest file contents
println!("{}", t!("greeting"));

Lang::unwatch();
```

---

<sub>lang-lib API reference - Copyright (c) 2026 James Gober. Apache-2.0 OR MIT.</sub>
