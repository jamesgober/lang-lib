# lang-lib

[![Crates.io](https://img.shields.io/crates/v/lang-lib.svg)](https://crates.io/crates/lang-lib)
[![Crates.io Downloads](https://img.shields.io/crates/d/lang-lib.svg)](https://crates.io/crates/lang-lib)
[![Docs.rs](https://docs.rs/lang-lib/badge.svg)](https://docs.rs/lang-lib)
[![CI](https://github.com/jamesgober/lang-lib/actions/workflows/ci.yml/badge.svg)](https://github.com/jamesgober/lang-lib/actions/workflows/ci.yml)
[![Benchmarks](https://github.com/jamesgober/lang-lib/actions/workflows/benchmarks.yml/badge.svg)](https://github.com/jamesgober/lang-lib/actions/workflows/benchmarks.yml)

A lightweight, high-performance localization library for Rust.

Loads TOML language files, supports runtime locale switching, configurable
paths per project, and automatic fallback chains. No proc macros, no codegen,
no CLI tooling — just a macro and a map.

The crate is deliberately small. The goal is not to invent a translation
platform. The goal is to make it easy to ship a Rust application with readable
locale files, predictable fallback behavior, and almost no integration cost.

## Installation

```toml
[dependencies]
lang-lib = "1.0.0"
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

## Tutorial

If you are wiring this into a real application, the usual setup looks like
this.

### 1. Create a locale directory

```text
your-app/
|- Cargo.toml
|- src/
|  \- main.rs
\- locales/
   |- en.toml
   \- es.toml
```

### 2. Create language files

Keep each file flat. One translation key maps to one string value.

```toml
# locales/en.toml
app_title = "Acme Control Panel"
login_title = "Sign in"
login_button = "Continue"
bad_password = "Your password is incorrect."
network_error = "We could not reach the server."
```

```toml
# locales/es.toml
app_title = "Panel de Control Acme"
login_title = "Iniciar sesion"
login_button = "Continuar"
bad_password = "La contrasena es incorrecta."
network_error = "No pudimos conectarnos al servidor."
```

Rules that matter:

- File names become locale identifiers, so `en.toml` loads as `en`.
- Locale identifiers must be simple file stems like `en`, `en-US`, or `pt_BR`.
- Nested TOML tables and non-string values are ignored.
- Keep keys stable and descriptive. Treat them like public API for your UI.

### 3. Load locales during startup

```rust
use lang_lib::Lang;

fn configure_i18n() -> Result<(), lang_lib::LangError> {
	Lang::set_path("locales");
	Lang::load("en")?;
	Lang::load("es")?;
	Lang::set_fallbacks(vec!["en".to_string()]);
	Lang::set_locale("en");
	Ok(())
}
```

### 4. Translate where the text is rendered

```rust
use lang_lib::{t, Lang};

fn render_login() {
	println!("{}", t!("login_title"));
	println!("{}", t!("login_button"));

	Lang::set_locale("es");
	println!("{}", t!("login_title"));
	println!("{}", t!("missing_key", fallback: "Default copy"));
}
```

### 5. Run the included example

The repository includes a runnable example wired to sample locale files:

```powershell
cargo run --example basic
```

## Server-Side Locale Policy

For request-driven services, the safest pattern is simple: load locales once
at startup, resolve the locale for each request, and pass that locale
explicitly when translating.

That means you should usually avoid calling `Lang::set_locale` inside request
handlers. `Lang` stores its active locale as process-global state, so changing
it per request creates unnecessary cross-request coupling.

Preferred pattern:

```rust
use lang_lib::{resolve_accept_language, Lang};

fn render_for_request(header: &str) -> String {
	let locale = resolve_accept_language(header, &["en", "es"], "en");
	Lang::translate("login_title", Some(locale), Some("Sign in"))
}
```

Runnable server-oriented example:

```powershell
cargo run --example server
```

If you want less repetition inside handlers, create a request-scoped helper:

```rust
use lang_lib::{resolve_accept_language, Lang};


fn render_for_request(header: &str) -> String {
	let locale = resolve_accept_language(header, &["en", "es"], "en");
	let translator = Lang::translator(locale);
	translator.translate_with_fallback("login_title", "Sign in")
}
```

## Accept-Language Helper

If your application already receives an `Accept-Language` header, the crate now
includes a small helper for turning that header into one of your supported
locale identifiers.

```rust
use lang_lib::resolve_accept_language;

let locale = resolve_accept_language(
	"es-ES,es;q=0.9,en;q=0.8",
	&["en", "es"],
	"en",
);

assert_eq!(locale, "es");
```

The helper prefers higher `q` values, then exact locale matches, then
primary-language matches like `es-ES` -> `es`.

If your supported locales are built at runtime, use the owned variant instead:

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

In plain terms: this version is for cases where your locale list is not a
hard-coded `&["en", "es"]`, but comes from config or some other runtime data.

## Translator Helper

`Translator` is a tiny convenience wrapper around a locale string. It keeps
request-local code readable while still using the safe server-side policy.

```rust
use lang_lib::{Lang, Translator};

fn render_page(locale: &str) -> String {
	let translator = Translator::new(locale);
	translator.translate_with_fallback("dashboard_title", "Dashboard")
}

fn render_page_via_lang(locale: &str) -> String {
	let translator = Lang::translator(locale);
	translator.translate("dashboard_title")
}
```

This helper does not change `Lang::locale()`. It only bundles a locale with
repeated translation calls.

## Axum Example

The repository also includes a real `axum` example that plugs locale
resolution into an HTTP handler.

Run it with:

```powershell
cargo run --example axum_server --features web-example-axum
```

Then request it with different `Accept-Language` headers:

```powershell
curl http://127.0.0.1:3000/
curl -H "Accept-Language: es-ES,es;q=0.9" http://127.0.0.1:3000/
```

If you prefer `actix-web`, the repository includes a matching example:

```powershell
cargo run --example actix_server --features web-example-actix
```

The three server-oriented examples share the same locale bootstrap and
`Accept-Language` parsing helper, so their behavior stays aligned as the
examples evolve.

## File Format

Plain TOML, one key per string:

```toml
# locales/en.toml
bad_password = "Your password is incorrect."
not_found    = "The page you requested does not exist."
```

Files are resolved as `{path}/{locale}.toml`.
Locale identifiers must be simple file stems like `en`, `en-US`, or `pt_BR`.
Path separators and relative path components are rejected before file access.

## API Notes

- `Lang::set_path` changes the base directory used by `Lang::load`.
- `Lang::load_from` lets you load a locale from a one-off directory.
- `Lang::set_locale` changes the process-wide active locale.
- `Lang::translator` creates a request-scoped helper for repeated lookups.
- `resolve_accept_language` maps an `Accept-Language` header to one of your supported locales.
- `resolve_accept_language_owned` does the same job when your supported locales live in `Vec<String>` or similar runtime data.
- `Lang::set_fallbacks` controls the order used when a key is missing.
- `Lang::loaded` returns a sorted list, which is useful for diagnostics.

## Fallback Behavior

When a key is not found, lookup proceeds as follows:

1. Requested locale (or active locale)
2. Each locale in the fallback chain, in order
3. Inline `fallback:` value if provided in `t!`
4. The key string itself — never returns empty

## Error Handling

`lang-lib` keeps failure modes narrow and explicit.

```rust
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

## Tips For Production Use

- Load all required locales during startup instead of lazily during request handling.
- Keep one fallback locale with complete coverage, usually `en`.
- In servers, resolve a locale per request and pass it explicitly instead of mutating the global active locale.
- Treat translation keys as stable identifiers and review changes to them carefully.

## Production Notes

- File lookup is cross-platform and uses the platform's native path handling.
- Locale loading rejects path traversal inputs such as `../en` or nested paths.
- Internal state recovers from poisoned locks instead of panicking on future reads.
- `Lang::loaded()` returns a sorted list for deterministic diagnostics and tests.

## Benchmarks

The repository includes a small Criterion benchmark that measures two hot paths:

- `resolve_accept_language`
- translation lookup through the loaded in-memory store
- fallback-chain lookup when a key is missing in the requested locale
- complete miss with inline fallback string
- complete miss that returns the key itself

Run it with:

```powershell
cargo bench --bench performance
```

This is not a full benchmarking suite, but it gives you repeatable numbers for
the operations most likely to matter in a request-driven application.

For interpretation guidance and CI benchmark policy, see [BENCHMARKS.md](BENCHMARKS.md).

## Health Signals

The badges at the top of this README point to the CI and benchmark workflows.
If they are blank right after adding workflows, trigger the workflows once and
GitHub will start showing status immediately.

## Repository Examples

- `examples/basic.rs`: end-to-end startup and translation flow.
- `examples/server.rs`: request-scoped locale resolution for server-side code.
- `examples/axum_server.rs`: real `axum` handler using request-scoped translation.
- `examples/actix_server.rs`: real `actix-web` handler using the same request-scoped policy.
- `examples/common/mod.rs`: shared example helper for locale loading and request locale resolution.
- `examples/locales/en.toml`: sample English locale file.
- `examples/locales/es.toml`: sample Spanish locale file.
- `BENCHMARKS.md`: benchmark usage notes, regression guidance, and CI benchmark policy.
- `benches/performance.rs`: Criterion benchmark for request locale resolution and translation lookup.

## License

Apache-2.0 — Copyright © 2026 James Gober
