//! The rules files of this repository (`cards/`), embedded in the crate at build time, so a server
//! needs no directory at run time and a deploy carries exactly the rules it was tested with.

use std::sync::OnceLock;

use crate::load::{Library, LoadErrors, Source};

/// Every file under `cards/`, with the name its errors use (`cards/lightning_bolt.json`), by path.
const FILES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/builtin_files.rs"));

static LIBRARY: OnceLock<Result<Library, LoadErrors>> = OnceLock::new();

/// The embedded files as sources.
pub fn sources() -> Vec<Source> {
    FILES
        .iter()
        .map(|&(path, text)| Source {
            path: path.to_owned(),
            text: text.to_owned(),
        })
        .collect()
}

/// The description of a predefined token, read from its embedded file without loading the library: the walker that
/// validates the library asks for it, so it cannot ask the library.
pub(crate) fn token_spec(id: &str) -> Option<crate::card::TokenSpec> {
    let path = format!("cards/tokens/{id}.json");
    let (_, text) = FILES.iter().find(|(file, _)| *file == path)?;
    serde_json::from_str::<crate::card::TokenFile>(text)
        .ok()
        .map(|file| file.token)
}

/// The library of the embedded files, loaded and validated on first use. The tests of this crate
/// load it, so an error here means the crate was built from files they would have refused.
pub fn library() -> Result<&'static Library, &'static LoadErrors> {
    LIBRARY
        .get_or_init(|| Library::from_sources(&sources()))
        .as_ref()
}

#[cfg(test)]
mod tests {
    //! Goal: the embedded library is the directory it was built from: every card and token file is in it, it validates, and asking twice gives the same library.
    use super::*;

    #[test]
    fn the_embedded_files_are_the_corpus() {
        let library = library().unwrap_or_else(|errors| panic!("{errors}"));
        assert_eq!(library.cards().count(), 162);
        assert_eq!(library.tokens().count(), 3);
        assert!(library.card_by_name("Lightning Bolt").is_some());
        assert!(std::ptr::eq(library, self::library().unwrap()));
    }

    #[test]
    fn the_sources_are_named_like_the_directory_and_equal_its_files() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let sources = sources();
        assert_eq!(sources.len(), 165);
        for source in &sources {
            assert!(source.path.starts_with("cards/"), "{}", source.path);
            let text = std::fs::read_to_string(root.join(&source.path)).unwrap();
            assert_eq!(text, source.text, "{}", source.path);
        }
        let mut paths: Vec<&str> = sources.iter().map(|s| s.path.as_str()).collect();
        let sorted = paths.clone();
        paths.sort_unstable();
        assert_eq!(paths, sorted, "listed in path order");
        assert!(paths.contains(&"cards/tokens/treasure.json"));
    }
}
