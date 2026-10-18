//! Reading rules files: parsing with paths in errors, the library of cards and tokens, and the canonical text of a file.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::assemble::{Assembled, assemble};
use crate::card::{Card, TokenFile};
use crate::pretty::pretty;
use crate::support::{Caps, PrintedCard, SupportRecord, support};
use crate::validate::{Problem, check_card, check_printed, check_token};

/// A problem in a file: which file, where inside it, and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    /// The file, as the source named it (`cards/serra_angel.json`).
    pub file: String,
    /// The path inside the file; empty for the file as a whole.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}: {}", self.file, self.message)
        } else {
            write!(f, "{}: {}: {}", self.file, self.path, self.message)
        }
    }
}

impl std::error::Error for LoadError {}

/// Every problem found while loading a library, one per line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadErrors(pub Vec<LoadError>);

impl fmt::Display for LoadErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, error) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LoadErrors {}

/// A file's name and text, however they were obtained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The name used in errors; a path with a `tokens` directory marks a token file.
    pub path: String,
    /// The JSON text.
    pub text: String,
}

impl Source {
    fn is_token(&self) -> bool {
        self.path
            .split('/')
            .rev()
            .skip(1)
            .any(|part| part == "tokens")
    }

    fn stem(&self) -> &str {
        let name = self.path.rsplit('/').next().unwrap_or(&self.path);
        name.strip_suffix(".json").unwrap_or(name)
    }
}

/// The lowercase ASCII slug of a card name: letters and digits, words joined by `_`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let mapped = match c {
            'Æ' | 'æ' => "ae".to_owned(),
            'á' | 'à' | 'â' | 'ä' | 'Á' | 'Â' => "a".to_owned(),
            'é' | 'è' | 'ê' | 'ë' | 'É' => "e".to_owned(),
            'í' | 'î' | 'ï' => "i".to_owned(),
            'ó' | 'ô' | 'ö' | 'Ö' => "o".to_owned(),
            'ú' | 'û' | 'ü' | 'Ú' | 'Û' => "u".to_owned(),
            '\'' | '\u{2019}' => String::new(),
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase().to_string(),
            _ => "_".to_owned(),
        };
        out.push_str(&mapped);
    }
    out.split('_')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Parses a rules file; errors carry the path of the value that failed.
pub fn parse_json<T: DeserializeOwned>(file: &str, text: &str) -> Result<T, LoadError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let inner = error.into_inner();
        LoadError {
            file: file.to_owned(),
            path: if path == "." || path == "?" {
                String::new()
            } else {
                path
            },
            message: inner.to_string(),
        }
    })
}

/// The canonical text of a value: JSON in serde's field order, short values on one line, and a trailing newline.
pub fn canonical<T: Serialize>(value: &T) -> String {
    pretty(&serde_json::to_string(value).unwrap_or_default())
}

fn errors_of(file: &str, problems: Vec<Problem>) -> impl Iterator<Item = LoadError> + '_ {
    problems.into_iter().map(move |p| LoadError {
        file: file.to_owned(),
        path: p.path,
        message: p.message,
    })
}

/// All the rules files of a directory tree: cards by oracle id, predefined tokens by id.
#[derive(Debug, Clone, Default)]
pub struct Library {
    cards: BTreeMap<String, Card>,
    tokens: BTreeMap<String, TokenFile>,
    files: BTreeMap<String, String>,
}

impl Library {
    /// Parses and validates the sources; every problem found is reported, not just the first.
    pub fn from_sources(sources: &[Source]) -> Result<Library, LoadErrors> {
        let mut library = Library::default();
        let mut errors = Vec::new();
        let mut cards = Vec::new();
        for source in sources {
            if source.is_token() {
                match parse_json::<TokenFile>(&source.path, &source.text) {
                    Ok(file) => {
                        if file.id != source.stem() {
                            errors.push(LoadError {
                                file: source.path.clone(),
                                path: "id".into(),
                                message: format!(
                                    "the file is named `{}.json`, so the id must be `{}`",
                                    source.stem(),
                                    source.stem()
                                ),
                            });
                        }
                        if library.tokens.insert(file.id.clone(), file).is_some() {
                            errors.push(LoadError {
                                file: source.path.clone(),
                                path: "id".into(),
                                message: "another token file has this id".into(),
                            });
                        }
                    }
                    Err(error) => errors.push(error),
                }
            } else {
                match parse_json::<Card>(&source.path, &source.text) {
                    Ok(card) => cards.push((source, card)),
                    Err(error) => errors.push(error),
                }
            }
        }
        let token_ids: BTreeSet<String> = library.tokens.keys().cloned().collect();
        for file in library.tokens.values() {
            let label = format!("tokens/{}.json", file.id);
            let path = sources
                .iter()
                .find(|s| s.is_token() && s.stem() == file.id)
                .map_or(label, |s| s.path.clone());
            errors.extend(errors_of(
                &path,
                check_token(&file.id, &file.token, &token_ids),
            ));
        }
        let mut names = BTreeSet::new();
        for (source, card) in cards {
            errors.extend(errors_of(&source.path, check_card(&card, &token_ids)));
            if source.stem() != slug(&card.name) {
                errors.push(LoadError {
                    file: source.path.clone(),
                    path: "name".into(),
                    message: format!(
                        "the file for `{}` is named `{}.json`",
                        card.name,
                        slug(&card.name)
                    ),
                });
            }
            if !names.insert(card.name.to_lowercase()) {
                errors.push(LoadError {
                    file: source.path.clone(),
                    path: "name".into(),
                    message: "another file has this name".into(),
                });
            }
            library
                .files
                .insert(card.oracle_id.clone(), source.path.clone());
            if library.cards.insert(card.oracle_id.clone(), card).is_some() {
                errors.push(LoadError {
                    file: source.path.clone(),
                    path: "oracle_id".into(),
                    message: "another file has this oracle id".into(),
                });
            }
        }
        if errors.is_empty() {
            Ok(library)
        } else {
            Err(LoadErrors(errors))
        }
    }

    /// Reads every `.json` file under `root` (any depth, sorted by path) and loads them.
    pub fn load_dir(root: &Path) -> Result<Library, LoadErrors> {
        let label = root
            .file_name()
            .map_or_else(|| "cards".to_owned(), |n| n.to_string_lossy().into_owned());
        let mut paths = Vec::new();
        let mut errors = Vec::new();
        collect(root, &mut paths, &mut errors, &label, root);
        paths.sort();
        let mut sources = Vec::new();
        for path in paths {
            let shown = format!(
                "{label}/{}",
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            match std::fs::read_to_string(&path) {
                Ok(text) => sources.push(Source { path: shown, text }),
                Err(error) => errors.push(LoadError {
                    file: shown,
                    path: String::new(),
                    message: error.to_string(),
                }),
            }
        }
        match Library::from_sources(&sources) {
            Ok(library) if errors.is_empty() => Ok(library),
            Ok(_) => Err(LoadErrors(errors)),
            Err(LoadErrors(mut more)) => {
                errors.append(&mut more);
                Err(LoadErrors(errors))
            }
        }
    }

    /// The rules of a card by oracle id.
    pub fn card(&self, oracle_id: &str) -> Option<&Card> {
        self.cards.get(oracle_id)
    }

    /// The rules of a card by its full name or one face name, ignoring case.
    pub fn card_by_name(&self, name: &str) -> Option<&Card> {
        let wanted = name.to_lowercase();
        self.cards.values().find(|card| {
            card.name.to_lowercase() == wanted
                || card
                    .faces
                    .iter()
                    .any(|face| face.name.to_lowercase() == wanted)
        })
    }

    /// A predefined token by id.
    pub fn token(&self, id: &str) -> Option<&TokenFile> {
        self.tokens.get(id)
    }

    /// Every card, by oracle id.
    pub fn cards(&self) -> impl Iterator<Item = &Card> {
        self.cards.values()
    }

    /// Every predefined token, by id.
    pub fn tokens(&self) -> impl Iterator<Item = &TokenFile> {
        self.tokens.values()
    }

    /// The file a card was read from.
    pub fn file_of(&self, oracle_id: &str) -> Option<&str> {
        self.files.get(oracle_id).map(String::as_str)
    }

    /// Checks every card that has an imported counterpart against it; a card with none is a problem too.
    pub fn check_printed(&self, printed: &[PrintedCard]) -> Vec<LoadError> {
        let names: BTreeSet<String> = printed
            .iter()
            .flat_map(|p| std::iter::once(&p.name).chain(p.faces.iter().map(|f| &f.name)))
            .map(|n| n.to_lowercase())
            .collect();
        let token_ids: BTreeSet<String> = self.tokens.keys().cloned().collect();
        let by_id: BTreeMap<&str, &PrintedCard> =
            printed.iter().map(|p| (p.oracle_id.as_str(), p)).collect();
        let mut errors = Vec::new();
        for card in self.cards.values() {
            let file = self.files.get(&card.oracle_id).cloned().unwrap_or_default();
            match by_id.get(card.oracle_id.as_str()) {
                Some(shown) => errors.extend(errors_of(
                    &file,
                    check_printed(card, shown, &names, &token_ids),
                )),
                None => errors.push(LoadError {
                    file,
                    path: "oracle_id".into(),
                    message: "no imported card has this oracle id".into(),
                }),
            }
        }
        errors
    }

    /// The support of an imported card: its rules file if there is one, nothing otherwise.
    pub fn support(&self, printed: &PrintedCard, caps: &Caps) -> SupportRecord {
        support(self.card(&printed.oracle_id), printed, caps)
    }

    /// The definition of an imported card with the rules this library holds for it, if any.
    pub fn assemble(&self, printed: &PrintedCard, caps: &Caps) -> Assembled {
        assemble(printed, self.card(&printed.oracle_id), caps)
    }
}

fn collect(
    dir: &Path,
    found: &mut Vec<PathBuf>,
    errors: &mut Vec<LoadError>,
    label: &str,
    root: &Path,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(LoadError {
                file: format!(
                    "{label}/{}",
                    dir.strip_prefix(root).unwrap_or(dir).to_string_lossy()
                ),
                path: String::new(),
                message: error.to_string(),
            });
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found, errors, label, root);
        } else if path.extension().is_some_and(|e| e == "json") {
            found.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: loading collects every problem with its file and path, names files by slug, and keeps ids and names unique.
    use super::*;
    use rstest::rstest;

    const BOLT: &str = r#"{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"Lightning Bolt","impl_status":"Manual","faces":[{"name":"Lightning Bolt"}]}"#;

    fn source(path: &str, text: &str) -> Source {
        Source {
            path: path.into(),
            text: text.into(),
        }
    }

    fn errors(sources: &[Source]) -> Vec<String> {
        Library::from_sources(sources)
            .err()
            .map(|e| e.0.iter().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    #[rstest]
    #[case("Lightning Bolt", "lightning_bolt")]
    #[case("Fire // Ice", "fire_ice")]
    #[case("Gaea's Cradle", "gaeas_cradle")]
    #[case("Jace, the Mind Sculptor", "jace_the_mind_sculptor")]
    #[case("Æther Vial", "aether_vial")]
    #[case("Lim-Dûl's Vault", "lim_duls_vault")]
    #[case(
        "Delver of Secrets // Insectile Aberration",
        "delver_of_secrets_insectile_aberration"
    )]
    fn slugs_are_lowercase_ascii(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(slug(name), expected);
    }

    #[test]
    fn a_good_library_loads_and_finds_cards_three_ways() {
        let library = Library::from_sources(&[source("cards/lightning_bolt.json", BOLT)]).unwrap();
        assert_eq!(library.cards().count(), 1);
        assert!(
            library
                .card("4457ed35-7c10-48c8-9776-456485fdf070")
                .is_some()
        );
        assert!(library.card_by_name("LIGHTNING BOLT").is_some());
        assert_eq!(
            library.file_of("4457ed35-7c10-48c8-9776-456485fdf070"),
            Some("cards/lightning_bolt.json")
        );
    }

    #[rstest]
    #[case::syntax("{", "cards/x.json: EOF while parsing")]
    #[case::unknown_field(
        r#"{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"X","impl_status":"Manual","faces":[],"id":1}"#,
        "cards/x.json: id: unknown field `id`"
    )]
    #[case::unknown_variant_with_path(
        r#"{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"X","impl_status":"Full","faces":[{"name":"X","abilities":[{"Keyword":"Flyign"}]}]}"#,
        "cards/x.json: faces[0].abilities[0].Keyword: unknown variant `Flyign`"
    )]
    #[case::wrong_type(
        r#"{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"X","impl_status":"Manual","faces":[{"name":"X","abilities":[{"Spell":{"effects":[{"Draw":{"count":"two"}}]}}]}]}"#,
        "faces[0].abilities[0].Spell.effects[0].Draw.count"
    )]
    fn parse_errors_name_the_file_and_the_path(#[case] text: &str, #[case] expected: &str) {
        let found = errors(&[source("cards/x.json", text)]);
        assert!(
            found.iter().any(|e| e.contains(expected)),
            "wanted {expected:?} in {found:#?}"
        );
    }

    #[test]
    fn every_file_is_reported_not_only_the_first() {
        let found = errors(&[source("cards/a.json", "{"), source("cards/b.json", "[")]);
        assert_eq!(found.len(), 2, "{found:#?}");
    }

    #[test]
    fn the_file_name_must_be_the_slug() {
        let found = errors(&[source("cards/bolt.json", BOLT)]);
        assert!(
            found
                .iter()
                .any(|e| e.contains("is named `lightning_bolt.json`")),
            "{found:#?}"
        );
    }

    #[test]
    fn duplicates_are_refused() {
        let other = BOLT.replace("Lightning Bolt", "Other Card");
        let found = errors(&[
            source("cards/lightning_bolt.json", BOLT),
            source("cards/lightning_bolt_again.json", BOLT),
            source("cards/other_card.json", &other),
        ]);
        assert!(
            found
                .iter()
                .any(|e| e.contains("another file has this oracle id")),
            "{found:#?}"
        );
        assert!(
            found
                .iter()
                .any(|e| e.contains("another file has this name")),
            "{found:#?}"
        );
    }

    #[test]
    fn tokens_are_found_by_directory_and_checked_by_name() {
        let treasure = r#"{"id":"treasure","token":{"name":"Treasure","types":["Artifact"],"subtypes":["Treasure"]}}"#;
        let library =
            Library::from_sources(&[source("cards/tokens/treasure.json", treasure)]).unwrap();
        assert!(library.token("treasure").is_some());
        let found = errors(&[source("cards/tokens/gold.json", treasure)]);
        assert!(
            found.iter().any(|e| e.contains("so the id must be `gold`")),
            "{found:#?}"
        );
    }

    #[test]
    fn a_named_token_must_exist() {
        let card = r#"{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"Maker","impl_status":"Full","faces":[{"name":"Maker","oracle_fingerprint":"e3b0c44298fc1c149afbf4c8996fb924","abilities":[{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}]}]}"#;
        let found = errors(&[source("cards/maker.json", card)]);
        assert!(
            found.iter().any(|e| e.contains(
                "faces[0].abilities[0].Spell.effects[0].CreateToken.token: unknown token `treasure`"
            )),
            "{found:#?}"
        );
    }

    #[test]
    fn canonical_text_ends_with_a_newline_and_reads_back() {
        let card: Card = parse_json("x", BOLT).unwrap();
        let text = canonical(&card);
        assert!(text.ends_with("}\n"));
        assert_eq!(parse_json::<Card>("x", &text).unwrap(), card);
    }

    #[test]
    fn a_directory_loads_recursively_and_names_files_with_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cards");
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(root.join("b").join("lightning_bolt.json"), BOLT).unwrap();
        std::fs::write(root.join("notes.txt"), "ignored").unwrap();
        let library = Library::load_dir(&root).unwrap();
        assert_eq!(library.cards().count(), 1);
        assert_eq!(
            library.file_of("4457ed35-7c10-48c8-9776-456485fdf070"),
            Some("cards/b/lightning_bolt.json")
        );
        std::fs::write(root.join("broken.json"), "{").unwrap();
        let error = Library::load_dir(&root).unwrap_err().to_string();
        assert!(error.contains("cards/broken.json: "), "{error}");
        assert!(Library::load_dir(&root.join("missing")).is_err());
    }
}
