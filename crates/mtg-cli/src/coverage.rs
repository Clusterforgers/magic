//! `coverage`: how well the compiler and the engine cover the stored precons, from the stored cards.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mtg_cards::coverage::{PreconCard, analyze};
use mtg_cards::{PrintedCard, PrintedFace};
use mtg_db::{Card, Database, DeckBoard, PreconDeckSummary, StoredFace};

use crate::args::CoverageArgs;
use crate::error::CliError;
use crate::status::missing_table;

/// The report on the precons the arguments select (all by default), as the text to print.
pub async fn coverage(db: &Database, args: &CoverageArgs) -> Result<String, CliError> {
    let (cards, unmatched) = precon_cards(db, args.deck.as_deref()).await?;
    let report = analyze(&cards, &mtg_engine::caps());
    if std::env::var("W14C_DUMP").is_ok() {
        for card in &report.cards {
            let labels: Vec<String> = card.blockers.iter().map(|b| b.label()).collect();
            eprintln!("W14C|{}|{}", card.name, labels.join(" ;; "));
        }
    }
    let mut text = report.render(args.top);
    if unmatched > 0 {
        let _ = write!(
            text,
            "\n{unmatched} card lines match no stored card and are left out\n"
        );
    }
    Ok(text.trim_end().to_owned())
}

/// The distinct cards of the main boards of the stored precons that `deck` selects (all when `None`), with their copies
/// per deck, and the count of card lines that match no stored card.
pub(crate) async fn precon_cards(
    db: &Database,
    deck: Option<&str>,
) -> Result<(Vec<PreconCard>, u64), CliError> {
    let repo = db.precons();
    let listed = repo.list().await.map_err(|error| {
        if missing_table(&error) {
            CliError::Config("the schema is not applied yet; `mtg-cli ingest` applies it".into())
        } else {
            CliError::Db(error)
        }
    })?;
    let decks: Vec<PreconDeckSummary> = listed
        .into_iter()
        .filter(|candidate| deck.is_none_or(|wanted| selects(candidate, wanted)))
        .collect();
    if decks.is_empty() {
        return Err(CliError::Config(match deck {
            Some(wanted) => format!("no stored precon has the code or a name like `{wanted}`"),
            None => {
                "no precons are stored; `mtg-cli fetch decks` and `mtg-cli ingest` load them".into()
            }
        }));
    }
    let labels = labels(&decks);
    let mut cards: BTreeMap<String, PreconCard> = BTreeMap::new();
    let mut unmatched = 0u64;
    for (deck, label) in decks.iter().zip(&labels) {
        let Some(detail) = repo.get(deck.id).await? else {
            continue;
        };
        for entry in detail.entries {
            // The sideboard is not part of the 100 cards a game plays.
            if entry.board == DeckBoard::Sideboard {
                continue;
            }
            let Some(card) = entry.card else {
                unmatched += u64::from(entry.count);
                continue;
            };
            let slot = cards
                .entry(card.oracle_id.as_str().to_owned())
                .or_insert_with(|| PreconCard {
                    printed: printed_card(&card),
                    copies: BTreeMap::new(),
                });
            *slot.copies.entry(label.clone()).or_default() += entry.count;
        }
    }
    Ok((cards.into_values().collect(), unmatched))
}

/// Whether `wanted` is the deck's code (any case) or part of its name (any case).
fn selects(deck: &PreconDeckSummary, wanted: &str) -> bool {
    deck.code.eq_ignore_ascii_case(wanted)
        || deck.name.to_lowercase().contains(&wanted.to_lowercase())
}

/// One label per deck, `code name`; a label two decks share ends in the deck's id.
fn labels(decks: &[PreconDeckSummary]) -> Vec<String> {
    let label = |deck: &PreconDeckSummary| format!("{} {}", deck.code, deck.name);
    let mut seen = BTreeSet::new();
    let mut twice = BTreeSet::new();
    for deck in decks {
        let label = label(deck);
        if !seen.insert(label.clone()) {
            twice.insert(label);
        }
    }
    decks
        .iter()
        .map(|deck| {
            let label = label(deck);
            if twice.contains(&label) {
                format!("{label} (#{})", deck.id.0)
            } else {
                label
            }
        })
        .collect()
}

/// A stored face as the strings the compiler reads.
fn printed_face(face: &StoredFace) -> PrintedFace {
    PrintedFace {
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
            .filter_map(|color| "WUBRG".chars().nth(color.index()))
            .map(String::from)
            .collect(),
    }
}

/// A stored card as the strings the compiler reads (the same as `mtg-server`'s `printed_card`).
pub(crate) fn printed_card(card: &Card) -> PrintedCard {
    PrintedCard {
        layout: card.layout.keyword().to_owned(),
        oracle_id: card.oracle_id.as_str().to_owned(),
        name: card.name.clone(),
        keywords: card.keywords.clone(),
        faces: card.faces.iter().map(printed_face).collect(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a stored face becomes the strings the compiler reads with its colors as WUBRG letters, a code or a name part selects a precon in any case, and decks that share a label stay apart.
    use mtg_db::PreconDeckId;
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    fn deck(id: i64, code: &str, name: &str) -> PreconDeckSummary {
        PreconDeckSummary {
            id: PreconDeckId(id),
            code: code.into(),
            name: name.into(),
            deck_type: None,
            release_date: None,
            card_count: 100,
            unresolved_count: 0,
        }
    }

    #[rstest]
    #[case::the_code("C21", true)]
    #[case::the_code_in_lower_case("c21", true)]
    #[case::a_prefix_of_the_code_is_not_the_code("C2", false)]
    #[case::a_name_part("Quandrix", true)]
    #[case::a_name_part_in_any_case("quantum QUAN", true)]
    #[case::the_whole_name("Quantum Quandrix", true)]
    #[case::words_that_are_not_adjacent("Quantum Quadrix", false)]
    #[case::another_deck("Lethality", false)]
    fn a_code_or_a_name_part_selects_a_deck(#[case] wanted: &str, #[case] selected: bool) {
        assert_eq!(
            selects(&deck(1, "C21", "Quantum Quandrix"), wanted),
            selected
        );
    }

    #[test]
    fn decks_that_share_a_code_and_a_name_are_told_apart_by_their_id() {
        let decks = [
            deck(1, "C21", "Quantum Quandrix"),
            deck(2, "C16", "Breed Lethality"),
            deck(3, "C16", "Breed Lethality"),
        ];
        assert_eq!(
            labels(&decks),
            [
                "C21 Quantum Quandrix",
                "C16 Breed Lethality (#2)",
                "C16 Breed Lethality (#3)"
            ]
        );
    }

    #[test]
    fn a_stored_face_keeps_its_strings_and_its_colors_as_letters_in_wubrg_order() {
        let face: StoredFace = serde_json::from_value(json!({
            "name": "Test Face",
            "mana_cost": "{G}{W}{U}{B}{R}",
            "type_line": "Creature \u{2014} Beast",
            "oracle_text": "Flying",
            "power": "*",
            "toughness": "2",
            "colors": ["G", "R", "W", "U", "B"],
        }))
        .unwrap();
        let printed = printed_face(&face);
        assert_eq!(printed.colors, ["W", "U", "B", "R", "G"]);
        assert_eq!(
            (
                printed.name.as_str(),
                printed.mana_cost.as_str(),
                printed.type_line.as_str(),
                printed.oracle_text.as_str()
            ),
            (
                "Test Face",
                "{G}{W}{U}{B}{R}",
                "Creature \u{2014} Beast",
                "Flying"
            )
        );
        assert_eq!(
            (printed.power.as_deref(), printed.toughness.as_deref()),
            (Some("*"), Some("2"))
        );
        assert_eq!((printed.loyalty, printed.defense), (None, None));
    }

    #[test]
    fn a_colorless_face_has_no_colors() {
        let face: StoredFace = serde_json::from_value(json!({
            "name": "Sol Ring", "mana_cost": "{1}", "type_line": "Artifact", "oracle_text": "", "colors": [],
        }))
        .unwrap();
        assert_eq!(printed_face(&face).colors, Vec::<String>::new());
    }
}
