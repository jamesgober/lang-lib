# lang-lib - Project Guidelines

Operational guidelines for using and contributing to `lang-lib`. The
authoritative reference for the public API is [`API.md`](API.md); this
document covers usage patterns, contribution flow, and the engineering
discipline the project commits to.

## Using lang-lib in production

### Loading model

Load every locale your application supports during startup. Do not
load lazily from inside a request handler — locale files are read
from disk, which is too slow for the request hot path.

```rust,no_run
use lang_lib::Lang;

Lang::set_path("locales");
for locale in ["en", "es", "fr", "de", "pt-BR"] {
    Lang::load(locale)?;
}
Lang::set_fallbacks(vec!["en".to_string()]);
# Ok::<(), lang_lib::LangError>(())
```

### Per-request locale handling

In a server, resolve the locale per request and pass it explicitly
to translation calls. Do **not** mutate `Lang::set_locale` from a
request handler — `Lang` stores its active locale as process-global
state, and writing to it from request handlers creates cross-request
coupling.

The preferred patterns are either explicit-locale calls into `Lang`,
or per-request `Translator` instances:

```rust,no_run
use lang_lib::{resolve_accept_language, Lang, Translator};

fn render_with_lang(header: &str) -> String {
    let locale = resolve_accept_language(header, &["en", "es"], "en");
    Lang::translate("login_title", Some(locale), Some("Sign in")).into_owned()
}

fn render_with_translator(header: &str) -> String {
    let locale = resolve_accept_language(header, &["en", "es"], "en");
    let t = Translator::new(locale);
    t.translate_with_fallback("login_title", "Sign in").into_owned()
}
```

`Translator` is `Copy`, so passing it through handler layers costs
nothing — it is a pointer copy.

### Choosing a feature set

| Use case                                                                  | Recommended features                |
|---------------------------------------------------------------------------|-------------------------------------|
| Web service, CLI tool, desktop app — load once, never touch files again   | *(default)*                         |
| You want to be notified when locales are loaded / unloaded programmatically | `registry`                          |
| Long-running service that reloads `<locale>.toml` files at runtime        | `hot-reload` (implies `registry`)   |

Enabling `hot-reload` changes the internal value-storage strategy
from interned `&'static str` (zero-alloc reads, never reclaims) to
`Arc<str>` (one allocation per read, reclaims on reload). See
[`API.md` § Memory model](API.md#memory-model) for the contract.

### Fallback chain policy

The fallback chain runs whenever a key is missing in the requested
locale. The recommended setup is:

- The active locale is whatever the user requested
- The fallback chain has one entry: your most complete locale, usually `en`

```rust,no_run
use lang_lib::Lang;

Lang::set_locale("ja");
Lang::set_fallbacks(vec!["en".to_string()]);
```

Keep fallback chains short. Each entry is checked on every miss; a
five-entry chain costs five hash lookups when a key is genuinely
absent everywhere. The `t!` macro's `fallback:` argument is the
last resort and is virtually free.

## Contributing

### Setup

1. Install Rust 1.85 or newer (`rustup install 1.85.0`).
2. Clone the repository.
3. `cargo test --all-features` should produce 60+ passing tests on a
   clean tree. If it doesn't, file an issue before opening a PR.

### Branches & commits

- Work from `main`. Open feature branches as `feat/<short-name>`,
  fixes as `fix/<short-name>`.
- Commit messages follow the format `<verb>: <summary>`. Use the
  imperative mood (`add`, `fix`, `refactor`).
- Milestone releases use `Milestone Update v<X.Y.Z>` as the commit
  title.

### CI gates

Every pull request must pass:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features` (60+ tests)
- `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS="-D warnings"`
- `cargo bench --bench performance --no-run`
- All four example checks (`server`, `axum_server`, `actix_server`, `hot_reload`)
- `cargo test --no-default-features`
- `cargo test --no-default-features --features registry`

### Adding a new public item

1. Add the item to `src/`.
2. Add rustdoc with at least one tested example.
3. Add an entry to [`API.md`](API.md) under the appropriate section.
4. Add a CHANGELOG entry under `[Unreleased]`.
5. Add a test under `tests/` if the item has behavior beyond a getter.

### Performance changes

If you touch any hot-path code (`src/store.rs::translate`,
`src/loader.rs`, the interner), capture before/after numbers from
`cargo bench --bench performance` in the pull request description.
Regressions over ~10% require justification.

## Engineering discipline

`lang-lib` is part of the broader portfolio standard documented in
`REPS.md` at the repository root. Items worth flagging here:

- **No `unwrap` / `expect` / `todo!` / `unimplemented!`** in
  non-test, non-example code. The `src/lib.rs` lint configuration
  enforces this.
- **No `print_stdout` / `print_stderr` / `dbg!`** in library code.
  Examples may use them.
- **Lock-free reads.** The translate hot path uses `ArcSwap`
  snapshots — no `Mutex`, no `RwLock` on the read path.
- **Documented unsafe.** The project currently has **zero** `unsafe`
  blocks. Keep it that way unless a lock-free primitive genuinely
  requires it.
- **Backward compatibility.** From `1.0.0` forward, removing or
  renaming a public item requires a `2.0` release. New items in
  minor releases are fine; feature-gating is fine.

## Reporting issues

- **Bugs:** include a minimal reproducer, your Rust version,
  platform, and the feature set you have enabled.
- **Performance regressions:** include criterion numbers from your
  hardware and the commit range you suspect.
- **Documentation gaps:** PRs are usually the fastest path to a fix.

---

<sub>lang-lib project guidelines - Copyright (c) 2026 James Gober. Apache-2.0 OR MIT.</sub>
