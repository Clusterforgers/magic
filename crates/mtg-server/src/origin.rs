//! The `Origin` check for game sockets: browsers always send it, so it keeps other sites from opening a player's socket.

use axum::http::HeaderMap;
use axum::http::header::{HOST, ORIGIN};

/// Whether a socket request may proceed.
///
/// No `Origin` header means a non-browser client (bots, tests), which a hijacking page cannot imitate.
/// Otherwise the origin must be on the allowed list or name the same authority as the `Host` header.
pub(crate) fn origin_allowed(headers: &HeaderMap, allowed: &[String]) -> bool {
    let mut origins = headers.get_all(ORIGIN).iter();
    let Some(origin) = origins.next() else {
        return true;
    };
    if origins.next().is_some() {
        return false;
    }
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let origin = origin.trim().to_ascii_lowercase();
    if allowed.contains(&origin) {
        return true;
    }
    let host = headers
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .map(|host| host.trim().to_ascii_lowercase());
    match (origin.split_once("://"), host) {
        (Some((_, authority)), Some(host)) => !host.is_empty() && authority == host,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: only a missing origin, a listed origin or the server's own host may open a socket; every other shape is refused.
    use super::*;
    use axum::http::HeaderValue;
    use rstest::rstest;

    fn headers(origins: &[&str], host: Option<&str>) -> HeaderMap {
        let mut map = HeaderMap::new();
        for origin in origins {
            map.append(ORIGIN, HeaderValue::from_str(origin).unwrap());
        }
        if let Some(host) = host {
            map.insert(HOST, HeaderValue::from_str(host).unwrap());
        }
        map
    }

    fn list() -> Vec<String> {
        vec![
            "https://magic.example".into(),
            "http://localhost:5173".into(),
        ]
    }

    #[rstest]
    #[case::no_origin(&[], Some("magic.example"), true)]
    #[case::no_origin_no_host(&[], None, true)]
    #[case::listed(&["https://magic.example"], Some("other.example"), true)]
    #[case::listed_dev_server(&["http://localhost:5173"], Some("127.0.0.1:8080"), true)]
    #[case::listed_other_case(&["HTTPS://Magic.Example"], None, true)]
    #[case::same_host(&["http://127.0.0.1:8080"], Some("127.0.0.1:8080"), true)]
    #[case::same_host_other_case(&["http://Play.Example"], Some("play.EXAMPLE"), true)]
    #[case::same_host_default_port(&["https://play.example"], Some("play.example"), true)]
    #[case::stranger(&["https://evil.example"], Some("magic.example"), false)]
    #[case::same_name_other_port(&["http://127.0.0.1:9999"], Some("127.0.0.1:8080"), false)]
    #[case::suffix_trick(&["https://magic.example.evil.example"], Some("magic.example"), false)]
    #[case::prefix_trick(&["https://evil.example/https://magic.example"], None, false)]
    #[case::null_origin(&["null"], Some("null"), false)]
    #[case::no_scheme(&["magic.example"], Some("magic.example"), false)]
    #[case::empty(&[""], Some("magic.example"), false)]
    #[case::origin_without_host(&["http://evil.example"], None, false)]
    #[case::two_origins(&["https://magic.example", "https://magic.example"], None, false)]
    fn socket_origins(
        #[case] origins: &[&str],
        #[case] host: Option<&str>,
        #[case] expected: bool,
    ) {
        assert_eq!(origin_allowed(&headers(origins, host), &list()), expected);
    }

    #[test]
    fn an_origin_that_is_not_visible_ascii_is_refused() {
        let mut map = HeaderMap::new();
        map.insert(
            ORIGIN,
            HeaderValue::from_bytes(b"https://m\xe9.example").unwrap(),
        );
        map.insert(HOST, HeaderValue::from_static("magic.example"));
        assert!(!origin_allowed(&map, &list()));
    }
}
