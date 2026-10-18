//! The session cookie: setting it, clearing it and finding it in a request.
//!
//! `mtg_session` holds the 256-bit session token and nothing else. It is `HttpOnly` (scripts cannot
//! read it), `SameSite=Lax` (other sites cannot send it along with a request of their own),
//! `Path=/` and, when the server sits behind TLS, `Secure`. It has no `Domain`, so it stays
//! with the host that set it.

use std::time::Duration;

use axum::http::header::COOKIE;
use axum::http::{HeaderMap, HeaderValue};

use crate::secret::{SessionToken, looks_like_token};

/// The cookie's name.
pub(crate) const COOKIE_NAME: &str = "mtg_session";

/// The `Set-Cookie` value that starts a session for `lifetime`.
pub(crate) fn start(token: &SessionToken, lifetime: Duration, secure: bool) -> Option<HeaderValue> {
    HeaderValue::from_str(&attributes(token.expose(), lifetime.as_secs(), secure)).ok()
}

/// The `Set-Cookie` value that makes the browser forget the session.
pub(crate) fn end(secure: bool) -> Option<HeaderValue> {
    HeaderValue::from_str(&attributes("", 0, secure)).ok()
}

fn attributes(value: &str, max_age: u64, secure: bool) -> String {
    let mut text =
        format!("{COOKIE_NAME}={value}; Max-Age={max_age}; Path=/; HttpOnly; SameSite=Lax");
    if secure {
        text.push_str("; Secure");
    }
    text
}

/// The session token a request presents: the first `mtg_session` cookie, if it has the shape of
/// a token. Whether the token is any good is for the database to say.
pub(crate) fn presented(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|line| line.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value)
        .filter(|value| looks_like_token(value))
}

#[cfg(test)]
mod tests {
    //! Goal: the cookie carries exactly the documented attributes (Secure only on request), a cleared cookie expires at once, and a request's cookies are read strictly: right name, right shape, first one wins, nothing else is taken for a session.
    use super::*;
    use rstest::rstest;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const MONTH: Duration = Duration::from_hours(24 * 30);

    fn token() -> SessionToken {
        SessionToken::generate().unwrap()
    }

    fn request(lines: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for line in lines {
            headers.append(COOKIE, HeaderValue::from_str(line).unwrap());
        }
        headers
    }

    #[rstest]
    #[case::plain(false, "")]
    #[case::secure(true, "; Secure")]
    fn a_session_cookie_has_the_documented_attributes(#[case] secure: bool, #[case] tail: &str) {
        let token = token();
        let value = start(&token, MONTH, secure).unwrap();
        let text = value.to_str().unwrap();
        assert_eq!(
            text,
            format!(
                "mtg_session={}; Max-Age=2592000; Path=/; HttpOnly; SameSite=Lax{tail}",
                token.expose()
            )
        );
        assert!(!text.contains("Domain"), "the cookie stays with the host");
        assert_eq!(text.contains("Secure"), secure);
    }

    #[rstest]
    #[case::plain(false, "")]
    #[case::secure(true, "; Secure")]
    fn the_cleared_cookie_expires_at_once_with_the_same_scope(
        #[case] secure: bool,
        #[case] tail: &str,
    ) {
        let value = end(secure).unwrap();
        assert_eq!(
            value.to_str().unwrap(),
            format!("mtg_session=; Max-Age=0; Path=/; HttpOnly; SameSite=Lax{tail}")
        );
    }

    #[rstest]
    #[case::one_hour(Duration::from_hours(1), 3_600)]
    #[case::thirty_days(MONTH, 2_592_000)]
    #[case::sub_second(Duration::from_millis(900), 0)]
    fn the_lifetime_is_whole_seconds(#[case] lifetime: Duration, #[case] seconds: u64) {
        let value = start(&token(), lifetime, false).unwrap();
        assert!(
            value
                .to_str()
                .unwrap()
                .contains(&format!("Max-Age={seconds};")),
            "{value:?}"
        );
    }

    /// In the cases `{T}` stands for a well-formed token, `{UPPER}` for the same in capitals,
    /// `{G}` for 64 `g` and `{B}` for 64 `b`.
    #[rstest]
    #[case::alone(&["mtg_session={T}"], Some(TOKEN))]
    #[case::among_others(&["theme=dark; mtg_session={T}; lang=en"], Some(TOKEN))]
    #[case::no_spaces(&["a=1;mtg_session={T};b=2"], Some(TOKEN))]
    #[case::extra_spaces(&["a=1;   mtg_session={T}  ;b=2"], Some(TOKEN))]
    #[case::second_header(&["a=1", "mtg_session={T}"], Some(TOKEN))]
    #[case::first_wins(&["mtg_session={T}; mtg_session={B}"], Some(TOKEN))]
    #[case::junk_first_means_none(&["mtg_session=junk; mtg_session={T}"], None)]
    #[case::no_cookie_header(&[], None)]
    #[case::empty_header(&[""], None)]
    #[case::other_cookies_only(&["a=1; b=2"], None)]
    #[case::no_equals(&["mtg_session"], None)]
    #[case::empty_value(&["mtg_session="], None)]
    #[case::name_prefix(&["xmtg_session={T}"], None)]
    #[case::name_suffix(&["mtg_session2={T}"], None)]
    #[case::name_case(&["MTG_SESSION={T}"], None)]
    #[case::inside_another_value(&["other=mtg_session={T}"], None)]
    #[case::too_short(&["mtg_session=abc123"], None)]
    #[case::too_long(&["mtg_session={T}0"], None)]
    #[case::upper_case_hex(&["mtg_session={UPPER}"], None)]
    #[case::not_hex(&["mtg_session={G}"], None)]
    #[case::quoted(&["mtg_session=\"{T}\""], None)]
    fn requests_present_a_token_only_in_the_right_cookie_and_shape(
        #[case] lines: &[&str],
        #[case] expected: Option<&str>,
    ) {
        let upper = TOKEN.to_uppercase();
        let (junk, other) = ("g".repeat(64), "b".repeat(64));
        let lines: Vec<String> = lines
            .iter()
            .map(|line| {
                line.replace("{T}", TOKEN)
                    .replace("{UPPER}", &upper)
                    .replace("{G}", &junk)
                    .replace("{B}", &other)
            })
            .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(presented(&request(&lines)), expected);
    }

    #[test]
    fn a_header_that_is_not_text_is_skipped() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_bytes(b"mtg_session=\xff\xfe").unwrap(),
        );
        assert_eq!(presented(&headers), None);
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("mtg_session={TOKEN}")).unwrap(),
        );
        assert_eq!(presented(&headers), Some(TOKEN));
    }

    #[test]
    fn what_start_sets_is_what_presented_reads_back() {
        let token = token();
        let set = start(&token, MONTH, true).unwrap();
        let pair = set.to_str().unwrap().split(';').next().unwrap().to_owned();
        assert_eq!(presented(&request(&[&pair])), Some(token.expose()));
    }
}
