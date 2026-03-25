# lang-lib

A lightweight, high-performance localization library for Rust.

Loads TOML language files, supports runtime locale switching, configurable
paths per project, and automatic fallback chains. No proc macros, no codegen,
no CLI tooling — just a macro and a map.

## Installation

```toml
[dependencies]
lang-lib = "0.1.0"
```

## Quick Start

```rust
use lang_lib::{t, Lang};

// Point to your project's lang folder
Lang::set_path("assets/lang");

// Load the locales you need
Lang::load("en").unwrap();
Lang::load("es").unwrap();

// Set the active locale
Lang::set_locale("en");

// Set a fallback chain (checked when a key is missing)
Lang::set_fallbacks(vec!["en".to_string()]);

// Translate
let msg = t!("bad_password");                          // active locale
let msg = t!("bad_password", "es");                    // specific locale
let msg = t!("unknown_key", fallback: "Oops");         // inline fallback
let msg = t!("unknown_key", "es", fallback: "Oops");   // locale + fallback
```

## File Format

Plain TOML, one key per string:

```toml
# locales/en.toml
bad_password = "Your password is incorrect."
not_found    = "The page you requested does not exist."
```

Files are resolved as `{path}/{locale}.toml`.

## Fallback Behavior

When a key is not found, lookup proceeds as follows:

1. Requested locale (or active locale)
2. Each locale in the fallback chain, in order
3. Inline `fallback:` value if provided in `t!`
4. The key string itself — never returns empty

## License

Apache-2.0 — Copyright © 2026 James Gober
