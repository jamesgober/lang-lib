# Changelog

All notable changes to lang-lib will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

No unreleased changes yet.

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

[Unreleased]: https://github.com/jamesgober/lang-lib/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/jamesgober/lang-lib/releases/tag/v1.0.1
[1.0.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.0.0
