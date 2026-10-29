//! A picture link from the database that is safe to fetch, and the names derived from it.

use std::fmt::Write;

use reqwest::Url;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Bytes of the digest an `ETag` keeps.
const TAG_BYTES: usize = 16;
/// Longest query a link may carry: Scryfall's is the time the picture changed, in digits.
const MAX_QUERY_CHARS: usize = 32;
/// The domain every picture comes from.
const CDN_DOMAIN: &str = "scryfall.io";

/// Why a stored link is not fetched.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LinkError {
    /// The text is not an absolute URL.
    #[error("the link is not a URL")]
    Malformed,
    /// Pictures come over `https` only.
    #[error("the link is not https")]
    Scheme,
    /// Pictures come from `scryfall.io` and its subdomains only.
    #[error("the link's host is not under scryfall.io")]
    Host,
    /// A link with a port or credentials names more than a CDN path.
    #[error("the link has a port or credentials")]
    Authority,
    /// The path or query is not one of a Scryfall picture.
    #[error("the link's path is not a picture path")]
    Path,
}

/// The picture formats Scryfall serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Jpeg,
    Png,
}

/// A picture link that passed the checks: `https`, a host under `scryfall.io`, no port or
/// credentials, and a path of plain segments ending in `.jpg` or `.png`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageLink {
    url: Url,
    key: String,
    format: Format,
}

impl ImageLink {
    /// Checks `text`, a link as the database stores it.
    pub fn parse(text: &str) -> Result<ImageLink, LinkError> {
        let url = Url::parse(text).map_err(|_| LinkError::Malformed)?;
        if url.scheme() != "https" {
            return Err(LinkError::Scheme);
        }
        let host = url.domain().ok_or(LinkError::Host)?;
        let under_cdn = host == CDN_DOMAIN
            || host
                .strip_suffix(CDN_DOMAIN)
                .is_some_and(|rest| rest.ends_with('.'));
        if !under_cdn {
            return Err(LinkError::Host);
        }
        if url.port().is_some() || !url.username().is_empty() || url.password().is_some() {
            return Err(LinkError::Authority);
        }
        let segments: Vec<&str> = url.path_segments().ok_or(LinkError::Path)?.collect();
        let plain = |segment: &&str| {
            !matches!(*segment, "" | "." | "..")
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        };
        let last = segments.last().ok_or(LinkError::Path)?;
        let format = match last.rsplit_once('.') {
            Some((_, "jpg")) => Format::Jpeg,
            Some((_, "png")) => Format::Png,
            _ => return Err(LinkError::Path),
        };
        if !segments.iter().all(plain) {
            return Err(LinkError::Path);
        }
        let mut key = segments.join("_");
        if let Some(query) = url.query() {
            let fine =
                query.len() <= MAX_QUERY_CHARS && query.bytes().all(|b| b.is_ascii_alphanumeric());
            if !fine {
                return Err(LinkError::Path);
            }
            key.push('@');
            key.push_str(query);
        }
        Ok(ImageLink { url, key, format })
    }

    /// The stored link.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// The name of the picture's file in the cache: the link's path with `_` between its
    /// segments, and `@` and the link's query when it has one.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The `Content-Type` of the picture.
    pub fn content_type(&self) -> &'static str {
        match self.format {
            Format::Jpeg => "image/jpeg",
            Format::Png => "image/png",
        }
    }

    /// Whether `bytes` start like a picture of this link's format.
    pub fn looks_like_picture(&self, bytes: &[u8]) -> bool {
        match self.format {
            Format::Jpeg => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
            Format::Png => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        }
    }

    /// The strong `ETag` of the picture: a digest of its key, so it changes when Scryfall
    /// changes the picture (the link's query) and needs no read of the file.
    pub fn etag(&self) -> String {
        let digest = Sha256::digest(self.key.as_bytes());
        let mut tag = String::with_capacity(2 + 2 * TAG_BYTES);
        tag.push('"');
        for byte in &digest[..TAG_BYTES] {
            // Writing to a `String` cannot fail.
            let _ = write!(tag, "{byte:02x}");
        }
        tag.push('"');
        tag
    }
}

#[cfg(test)]
mod tests {
    //! Goal: only https links to scryfall.io and its subdomains with plain picture paths are accepted, and the cache key, content type and tag follow the link.
    use rstest::rstest;

    use super::*;

    const GOOD: &str = "https://cards.scryfall.io/normal/front/0/0/0072bbbf-a695.jpg?1783911937";

    #[rstest]
    #[case::subdomain(GOOD)]
    #[case::apex("https://scryfall.io/a/b.png")]
    #[case::deeper("https://x.y.cards.scryfall.io/a/b.jpg")]
    #[case::upper_case_host("https://CARDS.SCRYFALL.IO/a/b.jpg")]
    #[case::no_query("https://cards.scryfall.io/normal/front/a/b.jpg")]
    fn accepted(#[case] link: &str) {
        assert!(ImageLink::parse(link).is_ok(), "{link}");
    }

    #[rstest]
    #[case::plain_http("http://cards.scryfall.io/a/b.jpg", LinkError::Scheme)]
    #[case::ftp("ftp://cards.scryfall.io/a/b.jpg", LinkError::Scheme)]
    #[case::other_host("https://evil.example/a/b.jpg", LinkError::Host)]
    #[case::lookalike_suffix("https://evilscryfall.io/a/b.jpg", LinkError::Host)]
    #[case::cdn_as_a_prefix("https://scryfall.io.evil.example/a/b.jpg", LinkError::Host)]
    #[case::cdn_in_the_path("https://evil.example/scryfall.io/b.jpg", LinkError::Host)]
    #[case::ip_address("https://127.0.0.1/a/b.jpg", LinkError::Host)]
    #[case::port("https://cards.scryfall.io:8443/a/b.jpg", LinkError::Authority)]
    #[case::credentials("https://u:p@cards.scryfall.io/a/b.jpg", LinkError::Authority)]
    #[case::empty_segment("https://cards.scryfall.io/a//b.jpg", LinkError::Path)]
    #[case::encoded_slash("https://cards.scryfall.io/a%2Fb.jpg", LinkError::Path)]
    #[case::not_a_picture("https://cards.scryfall.io/a/b.html", LinkError::Path)]
    #[case::no_extension("https://cards.scryfall.io/a/b", LinkError::Path)]
    #[case::bare_host("https://cards.scryfall.io/", LinkError::Path)]
    #[case::odd_query("https://cards.scryfall.io/a/b.jpg?x=../..", LinkError::Path)]
    #[case::not_a_url("cards.scryfall.io/a/b.jpg", LinkError::Malformed)]
    #[case::empty("", LinkError::Malformed)]
    fn refused(#[case] link: &str, #[case] error: LinkError) {
        assert_eq!(ImageLink::parse(link), Err(error), "{link}");
    }

    #[rstest]
    #[case("https://cards.scryfall.io/a/../b.jpg", "b.jpg")]
    #[case("https://cards.scryfall.io/a/%2e%2e/b.jpg", "b.jpg")]
    #[case("https://cards.scryfall.io/../../b.jpg", "b.jpg")]
    fn dots_are_resolved_before_the_key_is_made(#[case] link: &str, #[case] key: &str) {
        assert_eq!(ImageLink::parse(link).unwrap().key(), key);
    }

    #[test]
    fn the_key_is_the_path_and_the_query_in_one_safe_name() {
        let link = ImageLink::parse(GOOD).unwrap();
        assert_eq!(link.key(), "normal_front_0_0_0072bbbf-a695.jpg@1783911937");
        assert_eq!(link.content_type(), "image/jpeg");
        let png = ImageLink::parse("https://cards.scryfall.io/png/front/a/b.png").unwrap();
        assert_eq!(
            (png.key(), png.content_type()),
            ("png_front_a_b.png", "image/png")
        );
    }

    #[test]
    fn the_tag_follows_the_key_alone() {
        let one = ImageLink::parse(GOOD).unwrap().etag();
        let again = ImageLink::parse(
            "https://cards.scryfall.io/normal/front/0/0/0072bbbf-a695.jpg?1783911937",
        )
        .unwrap()
        .etag();
        let changed = ImageLink::parse(
            "https://cards.scryfall.io/normal/front/0/0/0072bbbf-a695.jpg?1783911938",
        )
        .unwrap()
        .etag();
        assert_eq!(one, again);
        assert_ne!(one, changed);
        assert!(
            one.starts_with('"') && one.ends_with('"') && one.len() == 34,
            "{one}"
        );
    }

    #[rstest]
    #[case::jpeg("a.jpg", &[0xFF, 0xD8, 0xFF, 0xE0], true)]
    #[case::jpeg_with_html("a.jpg", b"<html>", false)]
    #[case::jpeg_with_png("a.jpg", &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], false)]
    #[case::png("a.png", &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0], true)]
    #[case::png_with_jpeg("a.png", &[0xFF, 0xD8, 0xFF], false)]
    #[case::empty("a.png", &[], false)]
    fn the_first_bytes_tell_the_format(
        #[case] file: &str,
        #[case] bytes: &[u8],
        #[case] fine: bool,
    ) {
        let link = ImageLink::parse(&format!("https://cards.scryfall.io/{file}")).unwrap();
        assert_eq!(link.looks_like_picture(bytes), fine);
    }
}
