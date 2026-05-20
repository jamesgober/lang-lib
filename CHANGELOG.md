# Changelog

All notable changes to lang-lib will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

No unreleased changes yet.

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

[Unreleased]: https://github.com/jamesgober/lang-lib/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.1.0
[1.0.1]: https://github.com/jamesgober/lang-lib/releases/tag/v1.0.1
[1.0.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.0.0
