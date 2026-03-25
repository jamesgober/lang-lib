# Changelog

All notable changes to lang-lib will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial implementation: `Lang` struct with `set_path`, `set_locale`, `set_fallbacks`, `load`, `load_from`, `unload`, `is_loaded`, `loaded`, and `translate`
- `t!` macro with four forms: key only, key + locale, key + fallback, key + locale + fallback
- TOML file loading via `loader::load_file` and `loader::parse_toml`
- `LangError` with `Io`, `Parse`, and `NotLoaded` variants
- Full integration test suite covering loading, translation, fallback chain, macro forms, and edge cases

[Unreleased]: https://github.com/jamesgober/lang-lib/commits/main
