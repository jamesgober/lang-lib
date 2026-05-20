# Changelog

All notable changes to lang-lib will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

No unreleased changes yet.

## [1.3.0] - 2026-05-20

REPS-compliance release: closes the append-only-interner growth issue
introduced when `hot-reload` shipped in `1.2.0`. Adds a comprehensive
documentation pass and a new opt-in `translate_arc` API.

### The memory-safety fix

The string interner that backs the zero-allocation read path in
`1.1.0` and `1.2.0` only allocates safely because the default build
cannot reload locale files at runtime — the interner grows once at
startup and stays bounded by the count of unique strings in your
locale files. Under the `hot-reload` feature added in `1.2.0`, that
assumption no longer held: every reload that introduced fresh strings
would leak permanently.

`1.3.0` resolves this by making the value-storage strategy
*compile-time conditional* on the `hot-reload` feature:

| Build       | Value storage         | Hit-path return                       | Reclaims on reload? |
|-------------|-----------------------|----------------------------------------|---------------------|
| Default     | Interned `&'static str` | `Cow::Borrowed(&'static str)` — zero alloc | N/A (no reloads)    |
| `hot-reload` | `Arc<str>`            | `Cow::Owned(String)` — one alloc       | **Yes.**            |

Default builds keep their zero-allocation reads. `hot-reload` builds
trade one allocation per call for clean memory lifecycle. Both modes
remain lock-free and `Send + Sync`.

### Added

- **`Lang::translate_arc(key, locale, fallback) -> Arc<str>`** —
  available when `hot-reload` is enabled. Zero-allocation opt-in
  alternative to `Lang::translate`; the hit path is a refcount bump
  on the underlying `Arc<str>`. Use this when you've measured the
  per-call `String` allocation as a hot spot and same-key contention
  is not expected to spike. Returns a fresh `Arc<str>` (one alloc) on
  miss paths.
- **`Translator::translate_arc(&self, key)` and `Translator::translate_arc_with_fallback`** —
  mirror the new `Lang::translate_arc` on the request-scoped helper.
- **`tests/watch.rs::hot_reload_storage_drops_arc_str_on_reload`** —
  explicit refcount-probe test that verifies an `Arc<str>` retrieved
  before a reload has only the local handle outstanding after the
  reload, proving the store dropped its reference cleanly.
- **`docs/API.md`** — full API reference, previously empty. Mirrors
  the format of `pipe-io`'s `docs/API.md`: every public item, every
  parameter, every return type, multiple worked examples, a feature
  matrix, a stability statement, and a memory-model section.
- **`docs/PROJECT-GUIDELINES.md`** — production usage patterns,
  contribution flow, and engineering discipline. Previously empty.

### Changed

- **Internal value-storage layer split.** New private type alias
  `StoredValue = &'static str` (default) or `Arc<str>` (`hot-reload`),
  plumbed through `src/loader.rs` and `src/store.rs`. Public API
  shape is unchanged: `Lang::translate` still returns `Cow<'a, str>`.
- **Loader interns values only in default builds.** Under
  `hot-reload`, the parsed TOML `String` is moved into a fresh
  `Arc<str>` instead of leaked. Keys, locale identifiers, the
  configured path, and fallback chain entries continue to be
  interned in both builds (their cardinality is bounded by the
  developer-defined surface, not by reload churn).
- **`BENCHMARKS.md`** documents the new `translate_hit_concurrent`
  bench and the concurrency stress tests added in `1.1.0` /
  `1.2.0`.
- **`README.md`** clarifies the dual-storage trade-off in the
  optional-features install snippet, adds a Documentation section
  pointing at `docs/API.md` and `docs/PROJECT-GUIDELINES.md`, and
  notes the `examples/hot_reload.rs` example explicitly.

### Removed

- Nothing.

### Fixed

- **Append-only interner growth under `hot-reload`.** Reloading
  `<locale>.toml` files no longer leaks the old string values into
  a process-wide pool that never shrinks. This was the headline
  caveat of `1.2.0`; with `1.3.0` enabled `hot-reload` is genuinely
  REPS-compliant on memory.

### Compatibility

No breakage. The public API is source-compatible with `1.2.0`:

- `Lang::translate` still returns `Cow<'a, str>`. The variant that
  comes back differs by build (`Cow::Borrowed` in default,
  `Cow::Owned` under `hot-reload`), but anything that worked on a
  `Cow<'_, str>` in `1.2.0` continues to work in `1.3.0`.
- `Translator` is still `Copy`; the locale field is still a
  `&'static str`.
- All `1.2.0` feature flags retain their `1.2.0` behavior. The new
  `translate_arc` methods are additive and gated on `hot-reload`.

### Migration from 1.2.0

Just bump the version:

```toml
[dependencies]
lang-lib = "1.3.0"
```

If you're using `hot-reload` and want to avoid the new per-call
`String` allocation on the hit path, opt into `translate_arc`:

```rust,ignore
let value: std::sync::Arc<str> = Lang::translate_arc("greeting", Some("en"), None);
println!("{value}");
```

Be aware: `translate_arc`'s zero-alloc hit path comes with refcount
cache-line contention if the same key is read from many threads
simultaneously. The default `translate` returns `Cow::Owned` on
hits, which trades one allocation for contention-free reads.

## [1.2.0] - 2026-05-20

Hot-reload milestone. Two new opt-in features:

- **`registry`** — wires `lang-lib` to `registry-io 1.0`. Install handlers
  via `Lang::on_change` that fire whenever a locale is loaded, reloaded,
  or unloaded. Sub-microsecond dispatch per handler; panics in one
  handler do not affect siblings.
- **`hot-reload`** — implies `registry`. Adds `Lang::watch` /
  `Lang::unwatch`, which subscribe to filesystem events on the locales
  directory and atomically reload changed `<locale>.toml` files.
  Cross-platform (inotify / FSEvents / ReadDirectoryChangesW via the
  `notify` crate). Per-file events are debounced (~150 ms) so
  atomic-rename writes coalesce into a single reload.

### Added
- `registry` feature flag — pulls in `registry-io = "1"`.
- `hot-reload` feature flag — pulls in `notify = "6"` (implies `registry`).
- `lang_lib::LangChangeEvent` and `lang_lib::ChangeKind` (`registry`
  feature). Re-exported `registry_io::HandlerId` for ergonomic typing.
- `lang_lib::WatchError` (`hot-reload` feature).
- `Lang::on_change(handler) -> HandlerId` and
  `Lang::off_change(id) -> bool` (`registry` feature).
- `Lang::watch(dir)` and `Lang::unwatch()` (`hot-reload` feature).
- `tests/registry.rs` — five tests covering Loaded / Reloaded / Unloaded
  emission, no-op unload, and `off_change` stop semantics.
- `tests/watch.rs` — two smoke tests covering the round-trip
  modify→event→reload cycle and `unwatch` clean-up.
- `examples/hot_reload.rs` — a runnable demo of the watcher feeding live
  changes into `t!`.
- README features bullet for hot-reload / change notifications and an
  optional-features install snippet.

### Changed
- CI `actions/upload-artifact@v4` → `@v7` to follow the Node 20 → Node 24
  GitHub Actions deprecation.
- CI quality matrix now exercises `--no-default-features`,
  `--features registry`, and `cargo check --example hot_reload --features
  hot-reload` to keep feature combinations honest.

### Internal
- `src/change.rs` — `LangChangeEvent` + `ChangeKind` types.
- `src/registry.rs` — singleton `OnceLock<Arc<SyncRegistry<LangChangeEvent>>>`
  with private `emit()` helper.
- `src/watch.rs` — `notify::RecommendedWatcher` + dedicated worker thread
  with `HashMap<PathBuf, Instant>` debounce table and `Mutex<Option<…>>`
  singleton state.

### Migration

No code changes required for callers of `1.1.0` who do not opt in to
either feature. The default-feature surface is unchanged.

To adopt the new features:

```toml
[dependencies]
lang-lib = { version = "1.2.0", features = ["hot-reload"] }
```

```rust
let handler_id = Lang::on_change(|event| {
    println!("{:?} on {}", event.kind, event.locale);
});
Lang::watch("locales")?;
// ... application runs ...
Lang::unwatch();
let _ = Lang::off_change(handler_id);
```

## [1.1.0] - 2026-05-20

The performance release. Both `1.0.x` bottlenecks are eliminated and the
hot read path is now lock-free and zero-allocation. Behavior is preserved;
the only API change is the return type of `Lang::translate` and the two
`Translator::translate*` methods (see Migration below).

### Added
- `arc-swap = "1.7"` dependency for lock-free state snapshots.
- `rustc-hash = "2"` dependency for `FxHashMap` (faster short-key hashing).
- Internal append-only string interner (`src/intern.rs`) backing the
  zero-allocation hit path. Keys, values, locale identifiers, and the base
  path are all interned into a process-wide pool that hands out
  `&'static str` references.
- New concurrency stress test suite (`tests/concurrency.rs`):
  - 64-thread translate storm
  - concurrent reload during reader churn
  - unload-during-reads churn
- New concurrent benchmark `translate_hit_concurrent` sweeping 1, 4, 16,
  and 64 threads with per-iteration latency scaled to a single-thread
  equivalent.

### Changed
- **Read path is lock-free.** `RwLock<LangState>` replaced with
  `ArcSwap<LangState>`. Translate calls never acquire a lock; concurrent
  readers no longer contend. Writes (`load`, `unload`, `set_*`) serialize
  briefly against each other via a small private mutex but never block
  readers.
- **Hot path is zero-allocation.** `Lang::translate`,
  `Translator::translate`, and `Translator::translate_with_fallback` now
  return `Cow<'a, str>` instead of `String`:
  - Hit: `Cow::Borrowed` into the interned translation store — zero alloc
  - Fallback-chain hit: `Cow::Borrowed` into the interned store — zero alloc
  - Inline-fallback hit: `Cow::Borrowed` of the user-supplied fallback —
    zero alloc
  - Complete miss: `Cow::Borrowed` of the requested key — zero alloc
- **Translator is `Copy`.** Internal locale storage moved from `String` to
  `&'static str`, so `Translator` derives `Copy`. Cloning is now a pointer
  copy instead of a `String` clone.
- **Translation lookup uses `FxHashMap`** for both the locale registry and
  per-locale key tables. Empirically ~25–35 % faster than `std::HashMap`
  for short translation keys.
- `Lang::set_path` and `Lang::set_locale` now take `impl AsRef<str>`
  (previously `impl Into<String>`). Calls passing `&str`, `String`,
  `Cow<str>`, or any other `AsRef<str>` impl continue to compile;
  `Cow<str>` callers no longer need `.into_owned()`.
- `Lang::path` and `Lang::locale` now return `&'static str` (previously
  `String`). The string is the interned reference and lives for the
  program's lifetime, so callers no longer pay for a clone on every
  query. Existing `assert_eq!(Lang::path(), "literal")` and
  `format!("{}", Lang::locale())` patterns continue to compile.
- `Lang::loaded` now returns `Vec<&'static str>` (previously
  `Vec<String>`). Same `&str`-deref behavior; one fewer `String` clone
  per element.

### Migration from 1.0.1

The only common patterns that break are:

1. Explicit `let s: String = Lang::translate(...)` or `let s: String = t!(...)`.
   - Fix: append `.into_owned()` (which produces a `String`).
2. Code that stores translation output as a `String` field.
   - Fix: store the field as `Cow<'static, str>` or call `.into_owned()` at the boundary.
3. `Lang::loaded()` consumed as `Vec<String>` explicitly.
   - Fix: most uses index into the result as `&str`, which still works. For
     storage, `.iter().map(ToString::to_string).collect::<Vec<String>>()`.

`format!`, `println!`, `assert_eq!(_, "literal")`, equality against `&str`,
`Display`, and any code that uses the return as `&str` continues to work
without modification — the new return types deref to `&str`.

### Performance Notes

- **Bottleneck #1 (RwLock contention)** — eliminated. Verified by the
  64-thread storm test and the new concurrent benchmark.
- **Bottleneck #2 (per-call `String` allocation)** — eliminated on every
  path. Hits borrow into the interned store; misses borrow into the
  caller's inputs.
- Translation values are interned at load time and **not reclaimed by
  `Lang::unload`** in this release. The interner grows monotonically as
  unique strings are added; memory cost is bounded by the count of
  distinct strings ever loaded (typically a few hundred KB in real apps).
  The `1.2.0` hot-reload milestone will revisit this so long-running
  reloaders do not grow the interner without bound.

### Fixed
- Concurrent translate calls no longer serialize against each other.

## [1.0.1] - 2026-05-20

Portfolio standard compliance and REPS lint discipline. No behavior changes;
existing call sites and the public API are identical to `1.0.0`.

### Added
- Dual licensing under `Apache-2.0 OR MIT`. `LICENSE` renamed to
  `LICENSE-APACHE`; `LICENSE-MIT` added.
- Canonical `REPS.md` at the repo root (Rust Efficiency & Performance Standards).
- `.dev/PROMPT.md`, `.dev/DIRECTIVES.md`, `.dev/ROADMAP.md` — project context,
  engineering directives, and the production roadmap to `1.2.0`.
- `.dev/release/v1.0.1.md` — internal release notes for this patch.
- `docs/release-notes/v1.0.1.md` — public release note.
- `rustfmt.toml`, `clippy.toml` — portfolio-standard tooling configuration.

### Changed
- `Cargo.toml`: edition bumped from `2021` to `2024`.
- `Cargo.toml`: MSRV declared as `rust-version = "1.85"` (required by edition
  2024). Previously undeclared.
- `Cargo.toml`: license changed from `Apache-2.0` to `Apache-2.0 OR MIT`.
- `src/lib.rs`: lint configuration upgraded from `#![deny(warnings)]
  #![deny(clippy::all)]` to the full REPS discipline (deny `unwrap_used`,
  `expect_used`, `todo`, `unimplemented`, `print_stdout`, `print_stderr`,
  `dbg_macro`, `undocumented_unsafe_blocks`, `missing_safety_doc`; warn
  `pedantic`).
- `README.md`: MSRV badge updated to `1.85+`; dual-license footer; install
  snippet bumped to `1.0.1`.

### Fixed
- CI manifest-parse failure: `rust-version = "1.75"` was incompatible with
  `edition = "2024"` (which requires Rust ≥ 1.85). MSRV bumped to `1.85` to
  match edition 2024 requirements.

## [1.0.0] - 2026-03-25

### Added
- Initial implementation: `Lang` struct with `set_path`, `set_locale`, `set_fallbacks`, `load`, `load_from`, `unload`, `is_loaded`, `loaded`, and `translate`
- `t!` macro with four forms: key only, key + locale, key + fallback, key + locale + fallback
- TOML file loading via `loader::load_file` and `loader::parse_toml`
- `LangError` with `Io`, `Parse`, and `NotLoaded` variants
- Full integration test suite covering loading, translation, fallback chain, macro forms, and edge cases
- Expanded README tutorial with startup flow, language file layout, fallback behavior, and error handling guidance
- Runnable example program and sample locale files under `examples/`
- Added a server-side example that demonstrates request-scoped locale resolution without per-request global locale mutation
- Added a lightweight `Translator` helper for request-scoped translation ergonomics
- Added a real `axum` example that resolves locale from HTTP headers inside a handler
- Added a matching `actix-web` example for the same request-scoped translation pattern
- Added a public `resolve_accept_language` helper for mapping request headers to supported locales
- Added `resolve_accept_language_owned` for runtime locale lists such as `Vec<String>`
- Added a Criterion benchmark for `resolve_accept_language` and in-memory translation lookup
- Added a fallback-chain benchmark case for translation misses that resolve through configured fallbacks
- Added GitHub Actions workflows for cross-platform CI and benchmark execution
- Added complete-miss benchmark cases for inline fallback and key-return lookup paths

### Changed
- Hardened locale loading against path traversal and invalid locale identifiers
- Switched file resolution to platform-native path joining for better cross-platform behavior
- Recovered from poisoned state locks instead of panicking on subsequent access
- Made `Lang::loaded()` deterministic by sorting locale identifiers
- Isolated integration tests from shared global state and added coverage for hardened behavior
- Enriched rustdoc on the public API with more examples and production-oriented behavior notes
- Documented the recommended server-side locale policy in the README and API docs
- Documented request-scoped helper usage and included web-server integration guidance
- Feature-gated web example dependencies so `axum` and `actix-web` are only pulled in when those examples are built
- Centralized shared example locale bootstrapping and request locale parsing so server examples stay in sync
- Switched the shared server examples to use the public request-locale helper
- Documented the borrowed and owned request-locale helpers for both static and runtime locale lists
- Documented how to run the new performance benchmark
- Added benchmark guidance and CI notes to make performance regressions easier to spot
- Added workflow badges and a health-signals note in the README for quick status visibility

[Unreleased]: https://github.com/jamesgober/lang-lib/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.3.0
[1.2.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.2.0
[1.1.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.1.0
[1.0.1]: https://github.com/jamesgober/lang-lib/releases/tag/v1.0.1
[1.0.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.0.0
