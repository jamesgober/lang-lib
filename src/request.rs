fn parse_quality(param: &str) -> Option<u16> {
    let (_, value) = param.split_once('=')?;
    let value = value.trim();

    if value == "1" {
        return Some(1000);
    }

    if value == "0" {
        return Some(0);
    }

    if let Some(fraction) = value.strip_prefix("1.") {
        if fraction.chars().all(|ch| ch == '0') {
            return Some(1000);
        }

        return None;
    }

    let fraction = value.strip_prefix("0.")?;
    if fraction.is_empty() || !fraction.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let mut padded = fraction.to_string();
    padded.truncate(3);
    while padded.len() < 3 {
        padded.push('0');
    }

    padded.parse().ok()
}

fn parse_weighted_language(token: &str) -> Option<(String, u16)> {
    let mut parts = token.split(';');
    let language = parts.next()?.trim();
    if language.is_empty() {
        return None;
    }

    let mut quality = 1000;
    for part in parts {
        let part = part.trim();
        if part.starts_with("q=") {
            quality = parse_quality(part)?;
            break;
        }
    }

    Some((language.to_ascii_lowercase(), quality))
}

fn primary_subtag(locale: &str) -> &str {
    locale.split(['-', '_']).next().unwrap_or(locale)
}

fn match_score(requested: &str, supported: &str) -> u8 {
    let supported = supported.to_ascii_lowercase();

    if requested == supported {
        return 2;
    }

    if primary_subtag(requested) == primary_subtag(&supported) {
        return 1;
    }

    0
}

fn resolve_accept_language_impl<'a>(
    header: &str,
    supported_locales: &[&'a str],
    default_locale: &'a str,
) -> &'a str {
    let mut best_match: Option<(&'a str, u16, u8, usize)> = None;

    for (order, token) in header.split(',').enumerate() {
        let Some((language, quality)) = parse_weighted_language(token) else {
            continue;
        };

        for &supported in supported_locales {
            let score = match_score(&language, supported);
            if score == 0 {
                continue;
            }

            let is_better = match best_match {
                None => true,
                Some((_, best_quality, best_score, best_order)) => {
                    quality > best_quality
                        || (quality == best_quality
                            && (score > best_score || (score == best_score && order < best_order)))
                }
            };

            if is_better {
                best_match = Some((supported, quality, score, order));
            }
        }
    }

    best_match
        .map(|(locale, _, _, _)| locale)
        .unwrap_or(default_locale)
}

/// Resolves an `Accept-Language` header against a supported locale list.
///
/// The function prefers higher `q` values, then exact locale matches, then
/// primary-language matches such as `es-ES` -> `es`. If no supported locale
/// matches the header, `default_locale` is returned.
///
/// Matching is ASCII case-insensitive and supports locale identifiers that use
/// either `-` or `_` separators.
///
/// # Examples
///
/// ```rust
/// use lang_lib::resolve_accept_language;
///
/// let locale = resolve_accept_language(
///     "es-ES,es;q=0.9,en;q=0.8",
///     &["en", "es"],
///     "en",
/// );
///
/// assert_eq!(locale, "es");
/// ```
pub fn resolve_accept_language<'a>(
    header: &str,
    supported_locales: &[&'a str],
    default_locale: &'a str,
) -> &'a str {
    resolve_accept_language_impl(header, supported_locales, default_locale)
}

/// Resolves an `Accept-Language` header against a runtime locale list and
/// returns an owned `String`.
///
/// This variant is convenient when supported locales come from configuration,
/// a database, or any other runtime source represented as `Vec<String>`.
///
/// # Examples
///
/// ```rust
/// use lang_lib::resolve_accept_language_owned;
///
/// let supported = vec!["en".to_string(), "es".to_string()];
/// let locale = resolve_accept_language_owned(
///     "es-MX,es;q=0.9,en;q=0.7",
///     &supported,
///     "en",
/// );
///
/// assert_eq!(locale, "es");
/// ```
pub fn resolve_accept_language_owned<S>(
    header: &str,
    supported_locales: &[S],
    default_locale: &str,
) -> String
where
    S: AsRef<str>,
{
    let supported_refs: Vec<&str> = supported_locales.iter().map(AsRef::as_ref).collect();
    resolve_accept_language_impl(header, &supported_refs, default_locale).to_string()
}
