use axum::{
    extract::State,
    http::{header::ACCEPT_LANGUAGE, HeaderMap},
    routing::get,
    Router,
};
use lang_lib::Translator;

#[path = "common/mod.rs"]
mod shared;

#[derive(Clone)]
struct AppState {
    default_locale: &'static str,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    shared::configure_i18n()?;

    let app = Router::new().route("/", get(home)).with_state(AppState {
        default_locale: shared::DEFAULT_LOCALE,
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    println!("listening on http://127.0.0.1:3000");
    println!("try: curl -H \"Accept-Language: es-ES,es;q=0.9\" http://127.0.0.1:3000/");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn home(State(state): State<AppState>, headers: HeaderMap) -> String {
    let locale = headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .map(|header| shared::resolve_request_locale(header, state.default_locale))
        .unwrap_or(state.default_locale);

    let translator = Translator::new(locale);

    format!(
        "locale: {}\ntitle: {}\naction: {}\nerror: {}\n",
        translator.locale(),
        translator.translate_with_fallback("app_title", "Acme Dashboard"),
        translator.translate_with_fallback("save_button", "Save changes"),
        translator.translate_with_fallback("network_error", "We could not reach the server."),
    )
}
