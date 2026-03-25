# Changelog

All notable changes to lang-lib will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

No unreleased changes yet.

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

[Unreleased]: https://github.com/jamesgober/lang-lib/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jamesgober/lang-lib/releases/tag/v1.0.0
