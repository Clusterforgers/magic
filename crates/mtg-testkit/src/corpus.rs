//! Real card data for engine tests: the cards of the `mtg-cards` corpus, by name.
//!
//! Each card is the imported Scryfall record of its file in `crates/mtg-cards/tests/fixtures/corpus/`
//! put through the assembly the server uses, with the rules files of this repository and the
//! capabilities of this engine, so a test plays what a game would be given. [`Corpus::new`] takes
//! other capabilities, to see what a card becomes once the engine runs more.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use mtg_cards::assemble::Assembled;
use mtg_cards::support::SupportRecord;
use mtg_cards::{Caps, CardDefinition, PrintedCard, PrintedFace, builtin};
use mtg_core::mana::ManaType;
use mtg_import::scryfall::{CardRecord, cards};
use thiserror::Error;

/// The directory of Scryfall objects (trimmed) of the corpus: one `<slug of the card's name>.json` file per card.
pub fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../mtg-cards/tests/fixtures/corpus")
}

/// The files of [`fixture_dir`], in file-name order, as the one JSON array of Scryfall objects that the importer streams.
pub fn scryfall_array() -> Result<String, CorpusError> {
    let dir = fixture_dir();
    let unreadable =
        |error: std::io::Error| CorpusError::Data(format!("{}: {error}", dir.display()));
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(unreadable)? {
        let path = entry.map_err(unreadable)?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }
    files.sort();
    let mut array = String::from("[");
    for (at, file) in files.iter().enumerate() {
        if at > 0 {
            array.push(',');
        }
        let text = std::fs::read_to_string(file).map_err(unreadable)?;
        array.push_str(text.trim());
    }
    array.push(']');
    Ok(array)
}

/// Why a corpus card could not be had.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CorpusError {
    /// The corpus has no card of that name.
    #[error("the corpus has no card named {0:?}")]
    UnknownCard(String),
    /// The data the corpus is made from does not load.
    #[error("the corpus data does not load: {0}")]
    Data(String),
}

/// The cards of the corpus, assembled for one set of capabilities.
#[derive(Debug)]
pub struct Corpus {
    cards: Vec<Entry>,
    by_name: BTreeMap<String, usize>,
}

#[derive(Debug)]
struct Entry {
    assembled: Assembled,
    definition: Arc<CardDefinition>,
    name: String,
}

/// An imported record as the strings assembly reads: what the server makes of a stored card row, here from the record itself.
pub fn printed_card(record: &CardRecord) -> PrintedCard {
    PrintedCard {
        layout: record.layout.keyword().to_owned(),
        oracle_id: record.oracle_id.to_string(),
        name: record.name.clone(),
        keywords: record.keywords.clone(),
        faces: record
            .faces()
            .map(|face| PrintedFace {
                name: face.name.clone(),
                mana_cost: face.mana_cost.clone(),
                type_line: face.type_line.clone(),
                oracle_text: face.oracle_text.clone(),
                power: face.power.clone(),
                toughness: face.toughness.clone(),
                loyalty: face.loyalty.clone(),
                defense: face.defense.clone(),
                colors: face
                    .colors
                    .iter()
                    .map(|color| ManaType::from(color).symbol().to_string())
                    .collect(),
            })
            .collect(),
    }
}

impl Corpus {
    /// Assembles the corpus for an engine with these capabilities.
    pub fn new(caps: &Caps) -> Result<Corpus, CorpusError> {
        let rules = builtin::library().map_err(|errors| CorpusError::Data(errors.to_string()))?;
        let mut corpus = Corpus {
            cards: Vec::new(),
            by_name: BTreeMap::new(),
        };
        for record in cards(scryfall_array()?.as_bytes()) {
            let record = record.map_err(|error| CorpusError::Data(error.to_string()))?;
            let assembled = rules.assemble(&printed_card(&record), caps);
            let at = corpus.cards.len();
            for name in std::iter::once(&record.name).chain(record.faces().map(|f| &f.name)) {
                corpus.by_name.entry(name.to_lowercase()).or_insert(at);
            }
            corpus.cards.push(Entry {
                definition: Arc::new(assembled.definition.clone()),
                assembled,
                name: record.name,
            });
        }
        Ok(corpus)
    }

    fn entry(&self, name: &str) -> Result<&Entry, CorpusError> {
        self.by_name
            .get(&name.to_lowercase())
            .and_then(|&at| self.cards.get(at))
            .ok_or_else(|| CorpusError::UnknownCard(name.to_owned()))
    }

    /// The card called `name` (its full name or the name of one face, in any case), as the engine plays it.
    pub fn card(&self, name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
        self.entry(name).map(|entry| Arc::clone(&entry.definition))
    }

    /// How well these capabilities play the card.
    pub fn support(&self, name: &str) -> Result<&SupportRecord, CorpusError> {
        self.entry(name).map(|entry| &entry.assembled.support)
    }

    /// The full names of all the cards, in file-name order of the fixture.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.cards.iter().map(|entry| entry.name.as_str())
    }
}

/// The corpus for the capabilities of this engine, assembled once.
pub fn corpus() -> Result<&'static Corpus, CorpusError> {
    static CORPUS: OnceLock<Result<Corpus, CorpusError>> = OnceLock::new();
    CORPUS
        .get_or_init(|| Corpus::new(&mtg_engine::caps()))
        .as_ref()
        .map_err(Clone::clone)
}

/// The corpus card called `name`, as this engine plays it.
pub fn card(name: &str) -> Result<Arc<CardDefinition>, CorpusError> {
    corpus()?.card(name)
}

#[cfg(test)]
mod tests {
    //! Goal: the fixture directory loads as one array in file-name order, the corpus holds every card of the rules directory and of the fixture once, finds them by full name or face name in any case, and refuses a name it does not have.
    use super::*;
    use rstest::rstest;

    #[test]
    fn the_fixture_directory_loads_as_one_array_in_file_name_order() {
        let files = std::fs::read_dir(fixture_dir()).unwrap().count();
        let records: Vec<serde_json::Value> =
            serde_json::from_str(&scryfall_array().unwrap()).unwrap();
        assert_eq!(records.len(), files, "one object in each file");
        let files: Vec<String> = records
            .iter()
            .map(|record| {
                format!(
                    "{}.json",
                    mtg_cards::load::slug(record["name"].as_str().unwrap())
                )
            })
            .collect();
        assert!(
            files.windows(2).all(|pair| pair[0] < pair[1]),
            "the array follows the file names: {files:?}"
        );
    }

    #[test]
    fn every_card_file_and_fixture_record_is_there_once() {
        let corpus = corpus().unwrap();
        let names: Vec<&str> = corpus.names().collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "a name twice");
        let files = builtin::library().unwrap().cards().count();
        assert_eq!(names.len(), files, "one entry for each card file");
        assert_eq!(
            names.len(),
            cards(scryfall_array().unwrap().as_bytes()).count()
        );
        for name in &names {
            assert!(
                builtin::library().unwrap().card_by_name(name).is_some(),
                "{name}"
            );
        }
    }

    #[rstest]
    #[case::full_name("Lightning Bolt", "Lightning Bolt")]
    #[case::any_case("lIgHtNiNg bOlT", "Lightning Bolt")]
    #[case::front_face("Fire", "Fire // Ice")]
    #[case::back_face_of_a_split_card("Ice", "Fire // Ice")]
    #[case::whole_name_of_a_split_card("Fire // Ice", "Fire // Ice")]
    #[case::adventure("Stomp", "Bonecrusher Giant")]
    #[case::punctuation("Gaea's Cradle", "Gaea's Cradle")]
    fn a_card_is_found_by_any_of_its_names(#[case] asked: &str, #[case] front: &str) {
        assert_eq!(card(asked).unwrap().name, front);
    }

    #[test]
    fn a_name_the_corpus_lacks_is_refused() {
        assert_eq!(
            card("Black Lotus").unwrap_err(),
            CorpusError::UnknownCard("Black Lotus".into())
        );
    }

    #[rstest]
    #[case::an_engine_that_runs_nothing(Caps::none(), mtg_cards::support::Support::Manual, 0)]
    #[case::this_engine(mtg_engine::caps(), mtg_cards::support::Support::Verified, 1)]
    fn other_capabilities_change_what_a_card_becomes(
        #[case] caps: Caps,
        #[case] state: mtg_cards::support::Support,
        #[case] abilities: usize,
    ) {
        let corpus = Corpus::new(&caps).unwrap();
        assert_eq!(corpus.support("Llanowar Elves").unwrap().support, state);
        assert_eq!(
            corpus.card("Llanowar Elves").unwrap().abilities.len(),
            abilities
        );
        assert_eq!(
            corpus.card("Forest").unwrap().abilities.len(),
            1,
            "the land type needs no rules"
        );
    }

    #[test]
    fn the_same_card_is_shared_not_copied() {
        let (first, second) = (card("Forest").unwrap(), card("forest").unwrap());
        assert!(Arc::ptr_eq(&first, &second));
    }
}
