//! `propose-order`: the order of a paid run. The cards of the saved decks come first, the most used first, then the other
//! cards by EDHREC popularity; only Commander-legal paper cards (no Alchemy, perpetual or other digital-only card) without
//! rules in the repository (no hand-written file, no draft, not made `Verified` by the compiler).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use mtg_cards::builtin;
use mtg_cards::load::{Library, slug};
use mtg_cards::{Caps, PrintedCard};
use mtg_db::{CardSearch, Database};

use super::lines::pool::Translation;
use super::select::in_sweep_scope;
use crate::args::ProposeOrderArgs;
use crate::audit::{stored_cards, stored_cards_matching};
use crate::cache::Cache;
use crate::error::CliError;
use crate::popularity::Popularity;

/// How many decks of the server use a card.
pub type Demand = BTreeMap<String, u32>;

/// Whether a card exists only in digital play: an Alchemy rebalance (`A-` name) or a card whose text uses a digital-only
/// mechanic (perpetual changes, seek, conjure, spellbooks). Scryfall's Commander legality leaves these out already; this
/// is the guard for a stale database.
pub fn digital_only(card: &PrintedCard) -> bool {
    const WORDS: &[&str] = &["perpetual", "seek ", "seeks ", "conjure", "spellbook"];
    card.name.starts_with("A-")
        || card.faces.iter().any(|face| {
            let text = face.oracle_text.to_lowercase();
            WORDS.iter().any(|word| text.contains(word))
        })
}

/// The cards that are Commander-legal (their oracle id in `legal`) and not digital-only.
pub fn commander_cards(cards: Vec<PrintedCard>, legal: &BTreeSet<String>) -> Vec<PrintedCard> {
    cards
        .into_iter()
        .filter(|card| legal.contains(&card.oracle_id) && !digital_only(card))
        .collect()
}

/// The oracle ids of the stored cards Scryfall lists as legal in Commander.
pub async fn commander_legal_ids(db: &Database) -> Result<BTreeSet<String>, CliError> {
    let search = CardSearch {
        legal_in: Some("commander".to_owned()),
        ..CardSearch::default()
    };
    let cards = stored_cards_matching(db, &search).await?;
    Ok(cards.into_iter().map(|card| card.oracle_id).collect())
}

/// Whether line mode can work on a card: its translation in `dir` has one or two gapped lines and nothing else wrong.
pub fn line_ready(dir: &Path, name: &str) -> bool {
    Translation::load(dir, name).is_ok_and(|translation| translation.gaps().is_ok())
}

/// The names of the cards in the order of a run: first those of `demand` (a card's oracle id and the number of saved decks
/// that use it), the most used first, then the rest by EDHREC rank (a card without a rank last), ties by name. Only cards
/// a sweep would take and that have no draft (`drafted` holds the slugs of the drafts).
pub fn ordered_names(
    cards: &[PrintedCard],
    library: &Library,
    caps: &Caps,
    demand: &Demand,
    popularity: Option<&Popularity>,
    drafted: &BTreeSet<String>,
) -> Vec<String> {
    let mut pool: Vec<&PrintedCard> = cards
        .iter()
        .filter(|card| !drafted.contains(&slug(&card.name)))
        .filter(|card| in_sweep_scope(card, library, caps))
        .collect();
    let rank = |card: &PrintedCard| {
        popularity
            .and_then(|popularity| popularity.rank(&card.oracle_id))
            .unwrap_or(u32::MAX)
    };
    pool.sort_by(|a, b| {
        let key = |card: &PrintedCard| {
            let decks = demand.get(&card.oracle_id).copied().unwrap_or(0);
            // More decks first; among cards in no deck, the lower rank first.
            (std::cmp::Reverse(decks), rank(card), card.name.clone())
        };
        key(a).cmp(&key(b))
    });
    pool.into_iter().map(|card| card.name.clone()).collect()
}

/// `propose-order`: the names, one a line, written to `--out` or returned.
pub async fn command(
    db: &Database,
    cache: &Cache,
    args: &ProposeOrderArgs,
) -> Result<String, CliError> {
    let library = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let legal = commander_legal_ids(db).await?;
    let cards = commander_cards(stored_cards(db).await?, &legal);
    let demand: Demand = db
        .decks()
        .card_demand()
        .await?
        .into_iter()
        .map(|row| (row.oracle_id.0, row.decks))
        .collect();
    let popularity = Popularity::from_cache(cache).ok();
    let drafted: BTreeSet<String> = crate::drafts::embedded()
        .iter()
        .map(|(slug, _)| (*slug).to_owned())
        .collect();
    let mut names = ordered_names(
        &cards,
        library,
        &mtg_engine::caps(),
        &demand,
        popularity.as_ref(),
        &drafted,
    );
    if let Some(dir) = &args.lines {
        names.retain(|name| line_ready(dir, name));
    }
    if let Some(limit) = args.limit {
        names.truncate(limit);
    }
    let in_decks = names
        .iter()
        .filter(|name| {
            cards
                .iter()
                .find(|card| &card.name == *name)
                .is_some_and(|card| demand.contains_key(&card.oracle_id))
        })
        .count();
    let mut text = String::new();
    for name in &names {
        let _ = writeln!(text, "{name}");
    }
    let Some(path) = &args.out else {
        return Ok(text.trim_end().to_owned());
    };
    tokio::fs::write(path, &text)
        .await
        .map_err(|source| CliError::io(path, source))?;
    let asks = if args.lines.is_some() {
        "propose --lines --names-file"
    } else {
        "propose --cards"
    };
    Ok(format!(
        "propose-order: {} written to {} ({in_decks} of the saved decks first, then by popularity{}); `{asks} {}` asks them in that order",
        if names.len() == 1 {
            "1 card".to_owned()
        } else {
            format!("{} cards", names.len())
        },
        path.display(),
        if popularity.is_some() {
            ""
        } else {
            "; no EDHREC ranks in the cache, so by name"
        },
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: the order puts the cards of saved decks first by how many decks use them, then the rest by popularity with
    //! unranked cards last, leaves out cards that have rules, a draft or a compiled `Verified` reading and cards that are
    //! digital-only or not Commander-legal, and the line list keeps the cards with one or two gapped lines.
    use mtg_cards::PrintedFace;

    use super::*;

    fn card(name: &str, text: &str) -> PrintedCard {
        PrintedCard {
            oracle_id: format!("id-{name}"),
            name: name.to_owned(),
            keywords: Vec::new(),
            layout: "normal".into(),
            faces: vec![PrintedFace {
                name: name.to_owned(),
                type_line: "Sorcery".into(),
                oracle_text: text.to_owned(),
                ..PrintedFace::default()
            }],
        }
    }

    fn ranks(pairs: &[(&str, u32)]) -> Popularity {
        Popularity::new(
            pairs
                .iter()
                .map(|(name, rank)| (format!("id-{name}"), *rank))
                .collect(),
        )
        .unwrap()
    }

    fn order(
        cards: &[PrintedCard],
        demand: &[(&str, u32)],
        popularity: Option<&Popularity>,
        drafted: &[&str],
    ) -> Vec<String> {
        let library = builtin::library().unwrap();
        let demand: Demand = demand
            .iter()
            .map(|(name, decks)| (format!("id-{name}"), *decks))
            .collect();
        let drafted = drafted.iter().map(|slug| (*slug).to_owned()).collect();
        ordered_names(
            cards,
            library,
            &mtg_engine::caps(),
            &demand,
            popularity,
            &drafted,
        )
    }

    // Cards whose text nothing reads, so that every one is in the sweep's scope.
    fn oddities() -> Vec<PrintedCard> {
        ["Zeta", "Alpha", "Mu", "Omega", "Beta"]
            .iter()
            .map(|name| card(name, "Frobnicate the zorblax twice."))
            .collect()
    }

    #[test]
    fn saved_decks_come_first_most_used_first_then_by_popularity() {
        let cards = oddities();
        let popularity = ranks(&[("Zeta", 5), ("Alpha", 900), ("Mu", 50)]);
        let names = order(&cards, &[("Beta", 2), ("Omega", 7)], Some(&popularity), &[]);
        // Omega (7 decks), Beta (2 decks), then Zeta 5, Mu 50, Alpha 900, last the card without a rank.
        assert_eq!(names, ["Omega", "Beta", "Zeta", "Mu", "Alpha"]);
    }

    #[test]
    fn without_ranks_the_rest_is_by_name() {
        let names = order(&oddities(), &[("Mu", 1)], None, &[]);
        assert_eq!(names, ["Mu", "Alpha", "Beta", "Omega", "Zeta"]);
    }

    #[test]
    fn cards_with_a_draft_or_a_compiled_reading_are_left_out() {
        let mut cards = oddities();
        let mut bear = card("Bear", "Flying");
        bear.keywords = vec!["Flying".into()];
        bear.faces[0].type_line = "Creature — Bear".into();
        bear.faces[0].mana_cost = "{2}{G}".into();
        bear.faces[0].power = Some("2".into());
        bear.faces[0].toughness = Some("2".into());
        cards.push(bear);
        let names = order(&cards, &[], None, &["mu"]);
        assert!(!names.contains(&"Mu".to_owned()), "{names:?}");
        assert!(!names.contains(&"Bear".to_owned()), "{names:?}");
        assert_eq!(names.len(), 4);
    }

    #[rstest::rstest]
    #[case::paper("Opt", "Scry 1. Draw a card.", false)]
    #[case::alchemy_rebalance("A-Opt", "Scry 1. Draw a card.", true)]
    #[case::perpetual("Rope", "Target creature perpetually gets +1/+1.", true)]
    #[case::seek("Finder", "Seek a land card.", true)]
    #[case::conjure("Maker", "Conjure a card named Opt into your hand.", true)]
    #[case::spellbook("Book", "Draft a card from this card's spellbook.", true)]
    fn digital_only_cards_are_left_out_of_the_commander_pool(
        #[case] name: &str,
        #[case] text: &str,
        #[case] digital: bool,
    ) {
        let printed = card(name, text);
        assert_eq!(digital_only(&printed), digital);
        let legal: BTreeSet<String> = [printed.oracle_id.clone()].into();
        assert_eq!(commander_cards(vec![printed], &legal).is_empty(), digital);
    }

    #[test]
    fn a_card_without_commander_legality_is_left_out() {
        let cards = oddities();
        let legal: BTreeSet<String> = ["id-Mu".to_owned(), "id-Beta".to_owned()].into();
        let kept: Vec<String> = commander_cards(cards, &legal)
            .into_iter()
            .map(|card| card.name)
            .collect();
        assert_eq!(kept, ["Mu", "Beta"]);
    }

    #[test]
    fn the_line_list_keeps_cards_with_one_or_two_gapped_lines() {
        let dir = tempfile::tempdir().unwrap();
        let line = |gap: Option<&str>, abilities: Vec<usize>| serde_json::json!({"face": 0, "tag": "T", "text": gap.unwrap_or("x"), "abilities": abilities, "gap": gap});
        let put = |name: &str, lines: Vec<serde_json::Value>| {
            let stem = slug(name);
            std::fs::create_dir_all(dir.path().join("meta")).unwrap();
            std::fs::write(
                dir.path().join(format!("{stem}.json")),
                r#"{"faces":[{"abilities":[]}]}"#,
            )
            .unwrap();
            std::fs::write(
                dir.path().join("meta").join(format!("{stem}.json")),
                serde_json::json!({"lines": lines}).to_string(),
            )
            .unwrap();
        };
        put("One", vec![line(Some("a"), vec![])]);
        put(
            "Two",
            vec![line(Some("a"), vec![]), line(Some("b"), vec![])],
        );
        put(
            "Three",
            vec![
                line(Some("a"), vec![]),
                line(Some("b"), vec![]),
                line(Some("c"), vec![]),
            ],
        );
        put("Whole", vec![line(None, vec![0])]);
        let ready: Vec<&str> = ["One", "Two", "Three", "Whole", "Absent"]
            .into_iter()
            .filter(|name| line_ready(dir.path(), name))
            .collect();
        assert_eq!(ready, ["One", "Two"]);
    }
}
