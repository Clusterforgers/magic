//! Downloads into the cache: Scryfall bulk files and MTGJSON decks.

pub mod decks;
pub mod scryfall;

use reqwest::Url;

use crate::error::CliError;

/// Where the data comes from; tests point it at a local server.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// Scryfall's `bulk-data` listing.
    pub scryfall_listing: Url,
    /// MTGJSON's deck index.
    pub mtgjson_index: Url,
    /// The directory of MTGJSON deck files, with a trailing slash.
    pub mtgjson_decks: Url,
}

impl Endpoints {
    /// The real Scryfall and MTGJSON.
    pub fn real() -> Result<Self, CliError> {
        let parse = |text: &str| {
            Url::parse(text).map_err(|e| CliError::Config(format!("built-in address {text}: {e}")))
        };
        Ok(Self {
            scryfall_listing: parse("https://api.scryfall.com/bulk-data")?,
            mtgjson_index: parse("https://mtgjson.com/api/v5/DeckList.json")?,
            mtgjson_decks: parse("https://mtgjson.com/api/v5/decks/")?,
        })
    }
}

/// Whether a name from a server can be a file in the cache: one plain path component.
pub fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}

#[cfg(test)]
mod tests {
    //! Goal: only plain single-component names from a server can become cache files; traversal, separators and hidden names cannot.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("oracle-cards-20261018210200.jsonl.gz", true)]
    #[case("ForcesOfTheImperium_40K", true)]
    #[case("MahōGakuinSeishunHakusho魔法学院青春白書_SLD", true)]
    #[case("Atraxa's Trapper (Precon)", true)]
    #[case("", false)]
    #[case(".", false)]
    #[case("..", false)]
    #[case(".hidden", false)]
    #[case("../up", false)]
    #[case("dir/file", false)]
    #[case("dir\\file", false)]
    #[case("C:evil", false)]
    #[case("nul\0byte", false)]
    #[case("new\nline", false)]
    fn names_from_a_server_are_checked(#[case] name: &str, #[case] plain: bool) {
        assert_eq!(is_plain_file_name(name), plain);
    }

    #[test]
    fn the_real_endpoints_parse() {
        let endpoints = Endpoints::real().unwrap();
        assert_eq!(
            endpoints.scryfall_listing.host_str(),
            Some("api.scryfall.com")
        );
        assert!(endpoints.mtgjson_decks.path().ends_with('/'));
    }
}
