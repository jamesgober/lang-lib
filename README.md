<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <strong>lang-lib</strong>
    <br>
    <sup><sub>TRANSLATION LIBRARY FOR RUST</sub></sup>
</h1>

<p align="center">
    <a href="https://crates.io/crates/lang-lib"><img alt="crates.io" src="https://img.shields.io/crates/v/lang-lib.svg"></a>
    <a href="https://crates.io/crates/lang-lib"><img alt="downloads" src="https://img.shields.io/crates/d/lang-lib.svg?color=0099ff"></a>
    <a href="https://docs.rs/lang-lib"><img alt="docs.rs" src="https://docs.rs/lang-lib/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md" title="MSRV"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
    <a href="https://github.com/jamesgober/lang-lib/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/jamesgober/lang-lib/actions/workflows/ci.yml/badge.svg"></a>
</p>

<p align="center">
    <strong>A High-Performance Multi-Language Translation Library for Rust</strong>
    <br>
    <sub>FAST + SIMPLE + LIGHTWEIGHT + CONCURRENT + LOCK-FREE + THREAD-SAFE + ROBUST + STABLE</sub>
</p>
<br>

**Language Library**  ( **lang-lib** )  is a file-based **multi-language translation** library for Rust. It loads `TOML` translation files at startup and serves lookups by key, with runtime locale switching and configurable fallback chains. Designed to be simple, fast, lightweight, concurrent, and lock-free, it stays focused on doing one thing well — **translation** — without the weight of a full internationalization framework.

Setup is **deliberately frictionless**. There's no code generation, no build script, no CLI tooling, and no compile-time macros to wrestle with, just map your language files, call the `t!` macro, and you're translating. Drop your `TOML` files in a directory, point `lang-lib` at it, and every key is available across your application. Add a new language by adding a file; no rebuild step, no schema regeneration, nothing to wire up.

Every part of the API is shaped to reduce the amount of code you write. The `t!` macro handles the common case in a single line, accepts an optional locale override, and takes an inline fallback for missing keys, covering three distinct lookup patterns with one consistent call. For web handlers that need a fixed locale per request, a lightweight Translator wraps the active locale so you call `.translate("key")` without repeating it. No setup objects, no lifecycle management, no plumbing, just the call you actually wanted to make.

A simple, **lightweight** library with a **high-performance**, enterprise-ready core; engineered for **maximum stability** and the **resilience** to thrive under load. Don't let the simplicity fool you: underneath is a heavily-tested, performance-tuned, and fully error-hardened engine.
**Simplicity**, **stability**, and **high-performance** — together, with **zero compromise**.

<br>

## FEATURES

- **Multi-Language Support** — Load any number of locales from plain TOML files and switch the active language at runtime. No fixed locale set, no recompile to add one.

- **Zero-Allocation Lookups (default build)** — Translation values are interned at load time into a process-wide pool, and every successful lookup returns a `Cow::Borrowed(&'static str)` that points directly into that pool. The hot path costs zero heap allocations. Fallback and key-return paths also avoid allocation by borrowing from the caller's inputs.

- **Lock-Free Reads** — Translation state is swapped atomically via `ArcSwap`. Concurrent lookups never take a lock and never contend, scaling cleanly across cores.

- **Thread-Safe** — Every public type is `Send + Sync`. Share one translation store across your entire application; call it from any thread without external synchronization.

- **Configurable Fallback Chains** — When a key is missing in the active locale, `lang-lib` walks an ordered fallback list before giving up, so partial translations degrade gracefully instead of breaking.

- **Runtime Locale Switching** — Change the active language on the fly without reloading files or rebuilding state. Ideal for per-request locale selection in web servers.

- **One-Line API** — A single `t!` macro covers the common case, with optional locale and inline-fallback arguments. No translator object to construct, no context to thread through your call stack.

- **No Build Step** — No code generation, no build script, no CLI tooling. Map your language files, call the macro, ship. Adding a language means adding a file.

- **Minimal Dependencies** — A lean dependency graph and a small surface area keep compile times fast and audits simple.

- **Cross-Platform** — Runs identically on Linux, macOS, and Windows.

- **Hot Reload (opt-in)** — Enable the `hot-reload` feature to subscribe to filesystem events on your locales directory; edits to `<locale>.toml` are debounced and atomically reloaded in place, with `registry-io`-powered change-event notifications wired into [`Lang::on_change`].

- **Change Notifications (opt-in)** — Enable the `registry` feature to install handlers via [`Lang::on_change`] that fire whenever a locale is loaded, reloaded, or unloaded. Sub-microsecond dispatch overhead per handler.

<br>
<hr>
<br>

## Installation

```toml
[dependencies]
lang-lib = "1.3.0"
```

Optional features:

```toml
[dependencies]
# Subscribe to translation change events via registry-io.
lang-lib = { version = "1.3.0", features = ["registry"] }

# Watch locale files on disk and reload automatically.
# Implies `registry`. Switches value storage to Arc<str> so reloaded
# files no longer leak — the trade-off is one alloc per translate call.
lang-lib = { version = "1.3.0", features = ["hot-reload"] }
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

<br>

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

<br>

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

<br>

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

<br>

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

<br>

### 5. Run the included example

The repository includes a runnable example wired to sample locale files:

```powershell
cargo run --example basic
```

<br>

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

<br>

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

<br>

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

<br>

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

<br>

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

<br>

## API Notes

- `Lang::set_path` changes the base directory used by `Lang::load`.
- `Lang::load_from` lets you load a locale from a one-off directory.
- `Lang::set_locale` changes the process-wide active locale.
- `Lang::translator` creates a request-scoped helper for repeated lookups.
- `resolve_accept_language` maps an `Accept-Language` header to one of your supported locales.
- `resolve_accept_language_owned` does the same job when your supported locales live in `Vec<String>` or similar runtime data.
- `Lang::set_fallbacks` controls the order used when a key is missing.
- `Lang::loaded` returns a sorted list, which is useful for diagnostics.

<br>

## Fallback Behavior

When a key is not found, lookup proceeds as follows:

1. Requested locale (or active locale)
2. Each locale in the fallback chain, in order
3. Inline `fallback:` value if provided in `t!`
4. The key string itself — never returns empty

<br>

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

<br>

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

<br>

## Documentation

- [`docs/API.md`](docs/API.md): authoritative API reference — every public item, parameters, return types, examples, and the stability contract.
- [`docs/PROJECT-GUIDELINES.md`](docs/PROJECT-GUIDELINES.md): production usage patterns, contribution flow, and engineering discipline.
- [`docs/release-notes/`](docs/release-notes/): per-release notes from `1.0.1` onward.
- [`BENCHMARKS.md`](BENCHMARKS.md): benchmark usage, methodology, and CI policy.

## Repository Examples

- `examples/basic.rs`: end-to-end startup and translation flow.
- `examples/server.rs`: request-scoped locale resolution for server-side code.
- `examples/axum_server.rs`: real `axum` handler using request-scoped translation. (`--features web-example-axum`)
- `examples/actix_server.rs`: real `actix-web` handler using the same request-scoped policy. (`--features web-example-actix`)
- `examples/hot_reload.rs`: live filesystem-watcher demo with change-event handler. (`--features hot-reload`)
- `examples/common/mod.rs`: shared example helper for locale loading and request locale resolution.
- `examples/locales/en.toml`: sample English locale file.
- `examples/locales/es.toml`: sample Spanish locale file.
- `benches/performance.rs`: Criterion benchmark suite (single-thread + concurrent).

<br>

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.


<!-- FOOT COPYRIGHT
################################################# -->
<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>JAMES GOBER.</strong></sup>
</div>