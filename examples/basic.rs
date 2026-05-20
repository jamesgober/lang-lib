use std::path::PathBuf;

use lang_lib::{Lang, t};

fn main() -> Result<(), lang_lib::LangError> {
    let locale_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/locales");

    Lang::set_path(locale_dir.to_string_lossy());
    Lang::load("en")?;
    Lang::load("es")?;
    Lang::set_fallbacks(vec!["en".to_string()]);

    Lang::set_locale("en");
    println!("English:");
    println!("  {}", t!("app_title"));
    println!("  {}", t!("welcome"));

    Lang::set_locale("es");
    println!("Spanish:");
    println!("  {}", t!("app_title"));
    println!("  {}", t!("welcome"));
    println!(
        "  {}",
        t!("missing_copy", fallback: "Default text from code")
    );

    Ok(())
}
