//! The output directory of a run: accepted proposals as `<slug>.json`, the others under `rejected/` with the checker's report.

use std::fs;
use std::path::{Path, PathBuf};

use mtg_cards::card::Card;
use mtg_cards::load::{canonical, slug};

use super::forge_hint::HintRecord;
use crate::error::CliError;

/// The folder of the proposals of a run.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// A store in `dir`; the folder is made when the first file is written.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the accepted proposal of a card is.
    pub fn accepted_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.json", slug(name)))
    }

    fn rejected_path(&self, name: &str, extension: &str) -> PathBuf {
        self.dir
            .join("rejected")
            .join(format!("{}.{extension}", slug(name)))
    }

    /// Whether a proposal of the card is saved, accepted or not.
    pub fn has(&self, name: &str) -> bool {
        self.accepted_path(name).is_file() || self.rejected_path(name, "json").is_file()
    }

    /// Saves an accepted proposal as the completed rules in canonical form: the file `check-rules --write` would write.
    pub fn save_accepted(&self, name: &str, rules: &Card) -> Result<PathBuf, CliError> {
        let path = self.accepted_path(name);
        write(&path, &canonical(rules))?;
        Ok(path)
    }

    /// Saves the hint modes the card was asked with as `meta/<slug>.json`, so that runs of an A/B can be told apart.
    pub fn save_hints(&self, name: &str, record: &HintRecord) -> Result<(), CliError> {
        let path = self.dir.join("meta").join(format!("{}.json", slug(name)));
        let json = serde_json::to_string_pretty(record).unwrap_or_default();
        write(&path, &format!("{json}\n"))
    }

    /// Saves what the model wrote last and the checker's report, both under `rejected/`.
    pub fn save_rejected(&self, name: &str, json: &str, report: &str) -> Result<(), CliError> {
        write(&self.rejected_path(name, "json"), &format!("{json}\n"))?;
        write(&self.rejected_path(name, "txt"), &format!("{report}\n"))
    }
}

fn write(path: &Path, text: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    fs::write(path, text).map_err(|source| CliError::io(path, source))
}

#[cfg(test)]
mod tests {
    //! Goal: proposals are saved under the slug of the card's name, a saved proposal is found whether it was accepted or
    //! not, and rejected ones keep the model's JSON beside the report.
    use mtg_cards::card::ImplStatus;

    use super::*;

    fn rules() -> Card {
        Card {
            oracle_id: "id".into(),
            name: "Wind Drake".into(),
            impl_status: ImplStatus::Full,
            faces: Vec::new(),
            notes: None,
        }
    }

    #[test]
    fn the_hint_modes_of_a_card_are_saved_in_a_folder_of_their_own() {
        use crate::propose::forge_hint::HintState;
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let record = HintRecord {
            solved_examples: 5,
            forge_hint: HintState::Unhinted,
        };
        store.save_hints("Wind Drake", &record).unwrap();
        let saved = fs::read_to_string(dir.path().join("meta/wind_drake.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"solved_examples": 5, "forge_hint": "unhinted"})
        );
        // A file in that folder is no proposal.
        assert!(!store.has("Wind Drake"));
    }

    #[test]
    fn saved_proposals_are_found_by_the_name_of_the_card() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("out"));
        assert!(!store.has("Wind Drake"));
        let path = store.save_accepted("Wind Drake", &rules()).unwrap();
        assert_eq!(path, dir.path().join("out/wind_drake.json"));
        assert!(store.has("Wind Drake"));
        assert!(!store.has("Serra Angel"));
        store
            .save_rejected("Serra Angel", "{\"faces\":[]}", "Serra Angel: INVALID")
            .unwrap();
        assert!(store.has("Serra Angel"));
        let report = fs::read_to_string(dir.path().join("out/rejected/serra_angel.txt")).unwrap();
        assert_eq!(report, "Serra Angel: INVALID\n");
    }
}
