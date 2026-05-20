use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, http::header, web};
use lang_lib::Translator;

#[path = "common/mod.rs"]
mod shared;

#[derive(Clone)]
struct AppState {
    default_locale: &'static str,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    shared::configure_i18n().map_err(to_io_error)?;

    let state = web::Data::new(AppState {
        default_locale: shared::DEFAULT_LOCALE,
    });

    println!("listening on http://127.0.0.1:3001");
    println!("try: curl -H \"Accept-Language: es-ES,es;q=0.9\" http://127.0.0.1:3001/");

    HttpServer::new(move || App::new().app_data(state.clone()).service(home))
        .bind(("127.0.0.1", 3001))?
        .run()
        .await
}

#[get("/")]
async fn home(state: web::Data<AppState>, request: HttpRequest) -> impl Responder {
    let locale = request
        .headers()
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .map(|header| shared::resolve_request_locale(header, state.default_locale))
        .unwrap_or(state.default_locale);

    let translator = Translator::new(locale);

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(format!(
            "locale: {}\ntitle: {}\naction: {}\nerror: {}\n",
            translator.locale(),
            translator.translate_with_fallback("app_title", "Acme Dashboard"),
            translator.translate_with_fallback("save_button", "Save changes"),
            translator.translate_with_fallback("network_error", "We could not reach the server."),
        ))
}

fn to_io_error(error: lang_lib::LangError) -> std::io::Error {
    std::io::Error::other(error.to_string())
}
