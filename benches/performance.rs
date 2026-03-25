use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lang_lib::{resolve_accept_language, Lang, Translator};

const EN_LOCALE: &str = r#"
app_title = "Acme Dashboard"
welcome = "Welcome back"
save_button = "Save changes"
network_error = "We could not reach the server."
fallback_chain_hit = "English fallback value"
login_title = "Sign in"
login_button = "Continue"
search_placeholder = "Search"
profile_title = "Profile"
settings_title = "Settings"
toast_saved = "Changes saved"
"#;

const ES_LOCALE: &str = r#"
app_title = "Panel Acme"
welcome = "Bienvenido de nuevo"
save_button = "Guardar cambios"
network_error = "No pudimos conectarnos al servidor."
login_title = "Iniciar sesion"
login_button = "Continuar"
search_placeholder = "Buscar"
profile_title = "Perfil"
settings_title = "Configuracion"
toast_saved = "Cambios guardados"
"#;

fn benchmark_locale_dir() -> &'static Path {
    static BENCH_DIR: OnceLock<PathBuf> = OnceLock::new();

    BENCH_DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/bench-locales");
        fs::create_dir_all(&dir).expect("create benchmark locale directory");
        fs::write(dir.join("en.toml"), EN_LOCALE).expect("write en benchmark locale");
        fs::write(dir.join("es.toml"), ES_LOCALE).expect("write es benchmark locale");
        dir
    })
}

fn ensure_benchmark_state() {
    static INIT: OnceLock<()> = OnceLock::new();

    INIT.get_or_init(|| {
        let locale_dir = benchmark_locale_dir();
        let locale_dir = locale_dir.to_string_lossy().into_owned();

        Lang::set_path(locale_dir);
        Lang::load("en").expect("load en benchmark locale");
        Lang::load("es").expect("load es benchmark locale");
        Lang::set_fallbacks(vec!["en".to_string()]);
        Lang::set_locale("en");
    });
}

fn bench_resolve_accept_language(c: &mut Criterion) {
    let header = "es-MX,es;q=0.9,en-US;q=0.8,en;q=0.7";
    let supported = ["en", "es"];

    c.bench_function("resolve_accept_language", |b| {
        b.iter(|| {
            let locale =
                resolve_accept_language(black_box(header), black_box(&supported), black_box("en"));
            black_box(locale)
        })
    });
}

fn bench_translate_lookup(c: &mut Criterion) {
    ensure_benchmark_state();
    let translator = Translator::new("es");

    c.bench_function("translate_lookup", |b| {
        b.iter(|| {
            let value = translator
                .translate_with_fallback(black_box("network_error"), black_box("fallback"));
            black_box(value)
        })
    });
}

fn bench_translate_fallback_chain_miss(c: &mut Criterion) {
    ensure_benchmark_state();
    let translator = Translator::new("es");

    c.bench_function("translate_fallback_chain_miss", |b| {
        b.iter(|| {
            let value = translator
                .translate_with_fallback(black_box("fallback_chain_hit"), black_box("fallback"));
            black_box(value)
        })
    });
}

fn bench_translate_complete_miss_inline_fallback(c: &mut Criterion) {
    ensure_benchmark_state();
    let translator = Translator::new("es");

    c.bench_function("translate_complete_miss_inline_fallback", |b| {
        b.iter(|| {
            let value = translator.translate_with_fallback(
                black_box("missing_everywhere"),
                black_box("inline fallback"),
            );
            black_box(value)
        })
    });
}

fn bench_translate_complete_miss_key_return(c: &mut Criterion) {
    ensure_benchmark_state();
    let translator = Translator::new("es");

    c.bench_function("translate_complete_miss_key_return", |b| {
        b.iter(|| {
            let value = translator.translate(black_box("missing_everywhere"));
            black_box(value)
        })
    });
}

criterion_group!(
    performance_benches,
    bench_resolve_accept_language,
    bench_translate_lookup,
    bench_translate_fallback_chain_miss,
    bench_translate_complete_miss_inline_fallback,
    bench_translate_complete_miss_key_return
);
criterion_main!(performance_benches);
