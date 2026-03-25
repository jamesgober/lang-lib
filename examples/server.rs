use lang_lib::Lang;

#[path = "common/mod.rs"]
mod shared;

struct Request {
    id: u64,
    accept_language: &'static str,
}

struct Response {
    locale: &'static str,
    title: String,
    action: String,
    error: String,
}

fn main() -> Result<(), lang_lib::LangError> {
    shared::configure_i18n()?;

    let requests = [
        Request {
            id: 101,
            accept_language: "en-US,en;q=0.9",
        },
        Request {
            id: 102,
            accept_language: "es-ES,es;q=0.9,en;q=0.5",
        },
        Request {
            id: 103,
            accept_language: "fr-FR,fr;q=0.9",
        },
    ];

    for request in requests {
        let response = handle_request(&request);

        println!("request {} -> locale {}", request.id, response.locale);
        println!("  title: {}", response.title);
        println!("  action: {}", response.action);
        println!("  error: {}", response.error);
    }

    Ok(())
}

fn handle_request(request: &Request) -> Response {
    let locale = shared::resolve_request_locale(request.accept_language, shared::DEFAULT_LOCALE);

    Response {
        locale,
        title: Lang::translate("app_title", Some(locale), Some("Acme Dashboard")),
        action: Lang::translate("save_button", Some(locale), Some("Save changes")),
        error: Lang::translate(
            "network_error",
            Some(locale),
            Some("We could not reach the server."),
        ),
    }
}
