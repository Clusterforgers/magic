//! Goal: deck lists from every format resolve against an index built from recorded Scryfall data to the same cards, while typos and unknown names are reported with suggestions and never guessed.

use std::collections::BTreeMap;

use mtg_import::decklist::{Section, parse};
use mtg_import::ids::OracleId;
use mtg_import::mtgjson;
use mtg_import::resolve::{
    CardIndex, Lookup, Resolution, ResolveError, UnresolvedEntry, resolve_deck,
};
use mtg_import::scryfall::{CardRecord, cards};
use rstest::rstest;

const CARDS_JSON: &str = include_str!("../fixtures/scryfall_oracle_cards.json");
const PLAIN: &str = include_str!("../fixtures/deck_plain.txt");
const PLAIN_BLANK: &str = include_str!("../fixtures/deck_plain_blank_sideboard.txt");
const ARENA: &str = include_str!("../fixtures/deck_arena.txt");
const MOXFIELD: &str = include_str!("../fixtures/deck_moxfield.txt");
const ARCHIDEKT: &str = include_str!("../fixtures/deck_archidekt.txt");
const MTGO: &str = include_str!("../fixtures/deck_mtgo.dek");
const ARENA_COMMANDER: &str = include_str!("../fixtures/deck_arena_commander.txt");
const ARCHIDEKT_COMMANDER: &str = include_str!("../fixtures/deck_archidekt_commander.txt");
const UNICODE_AND_TYPOS: &str = include_str!("../fixtures/deck_unicode_and_typos.txt");
const MTGJSON_DECK: &str = include_str!("../fixtures/mtgjson_deck_commander.json");

fn records() -> Vec<CardRecord> {
    cards(CARDS_JSON.as_bytes())
        .filter_map(Result::ok)
        .collect()
}

fn index() -> CardIndex {
    CardIndex::from_records(&records())
}

fn oracle_of(name: &str) -> OracleId {
    records()
        .into_iter()
        .find(|c| c.name == name)
        .map_or_else(|| panic!("fixture has no card {name:?}"), |c| c.oracle_id)
}

fn by_oracle(resolution: &Resolution) -> BTreeMap<(OracleId, Section), u32> {
    let mut totals = BTreeMap::new();
    for entry in &resolution.resolved {
        *totals
            .entry((entry.oracle_id.clone(), entry.section))
            .or_default() += entry.count;
    }
    totals
}

fn unresolved<'a>(resolution: &'a Resolution, name: &str) -> &'a UnresolvedEntry {
    resolution
        .unresolved
        .iter()
        .find(|u| u.entry.name == name)
        .unwrap_or_else(|| panic!("{name:?} should be unresolved"))
}

#[test]
fn index_contains_deck_cards_only() {
    let index = index();
    assert_eq!(index.len(), 20);
    assert_eq!(index.find("Goblin"), Lookup::Missing);
    assert_eq!(
        index.find("Elspeth, Sun's Champion Emblem"),
        Lookup::Missing
    );
    assert!(
        matches!(index.find("Lightning Bolt"), Lookup::Found(c) if c.oracle_id == oracle_of("Lightning Bolt"))
    );
}

#[rstest]
#[case::plain(PLAIN)]
#[case::plain_blank(PLAIN_BLANK)]
#[case::arena(ARENA)]
#[case::moxfield(MOXFIELD)]
#[case::archidekt(ARCHIDEKT)]
#[case::mtgo(MTGO)]
fn every_format_resolves_to_the_same_cards(#[case] input: &str) {
    let deck = parse(input);
    let resolution = resolve_deck(&index(), &deck);
    assert!(
        resolution.unresolved.is_empty(),
        "{:?}",
        resolution.unresolved
    );
    assert_eq!(resolution.resolved.len(), deck.entries.len());

    let expected: BTreeMap<_, _> = [
        (("Lightning Bolt", Section::Main), 4),
        (("Shock", Section::Main), 4),
        (("Fire // Ice", Section::Main), 2),
        (("Sol Ring", Section::Main), 1),
        (("Mountain", Section::Main), 20),
        (("Pyroblast", Section::Sideboard), 2),
        (("Æther Vial", Section::Sideboard), 1),
    ]
    .into_iter()
    .map(|((name, section), count)| ((oracle_of(name), section), count))
    .collect();
    assert_eq!(by_oracle(&resolution), expected);
}

#[test]
fn canonical_names_replace_the_spelling_used_in_the_list() {
    let resolution = resolve_deck(&index(), &parse(MTGO));
    let names: Vec<_> = resolution
        .resolved
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert!(names.contains(&"Fire // Ice"));
    assert!(names.contains(&"Æther Vial"));
    assert!(!names.contains(&"Fire/Ice"));
}

#[test]
fn printing_hints_pass_through_unchecked() {
    let resolution = resolve_deck(&index(), &parse(ARENA));
    let bolt = &resolution.resolved[0];
    assert_eq!(bolt.set.as_deref(), Some("M10"));
    assert_eq!(bolt.collector_number.as_deref(), Some("146"));
}

#[rstest]
#[case::arena_commander(ARENA_COMMANDER, 4, Section::Commander)]
#[case::archidekt_commander(ARCHIDEKT_COMMANDER, 5, Section::Commander)]
fn commander_lists_resolve(#[case] input: &str, #[case] entries: usize, #[case] first: Section) {
    let resolution = resolve_deck(&index(), &parse(input));
    assert!(
        resolution.unresolved.is_empty(),
        "{:?}",
        resolution.unresolved
    );
    assert_eq!(resolution.resolved.len(), entries);
    assert_eq!(resolution.resolved[0].section, first);
    assert_eq!(
        resolution.resolved[0].oracle_id,
        oracle_of("Atraxa, Praetors' Voice")
    );
}

#[test]
fn spelling_variants_resolve_and_typos_do_not() {
    let deck = parse(UNICODE_AND_TYPOS);
    assert!(deck.errors.is_empty(), "{:?}", deck.errors);
    let resolution = resolve_deck(&index(), &deck);

    let resolved: Vec<_> = resolution
        .resolved
        .iter()
        .map(|r| (r.name.as_str(), r.oracle_id.clone()))
        .collect();
    let expected = [
        "Æther Vial",
        "Lim-Dûl's Vault",
        "Atraxa, Praetors' Voice",
        "Fire // Ice",
        "Bonecrusher Giant // Stomp",
        "Delver of Secrets // Insectile Aberration",
        "Hazmat Suit (Used)",
    ];
    let expected: Vec<_> = expected.iter().map(|n| (*n, oracle_of(n))).collect();
    assert_eq!(resolved, expected);

    let unresolved_names: Vec<_> = resolution
        .unresolved
        .iter()
        .map(|u| u.entry.name.as_str())
        .collect();
    assert_eq!(
        unresolved_names,
        [
            "Lightnin Bolt",
            "Sol Rign",
            "Jace the Mind Sculptor",
            "Counterspell",
            "Stomp"
        ]
    );
}

#[rstest]
#[case("Lightnin Bolt", Some("Lightning Bolt"))]
#[case("Sol Rign", Some("Sol Ring"))]
#[case("Jace the Mind Sculptor", Some("Jace, the Mind Sculptor"))]
#[case("Stomp", Some("Bonecrusher Giant // Stomp"))]
#[case("Counterspell", None)]
fn unknown_names_come_with_suggestions_not_guesses(
    #[case] name: &str,
    #[case] first_suggestion: Option<&str>,
) {
    let resolution = resolve_deck(&index(), &parse(UNICODE_AND_TYPOS));
    let entry = unresolved(&resolution, name);
    match &entry.error {
        ResolveError::UnknownCard {
            name: reported,
            suggestions,
        } => {
            assert_eq!(reported, name);
            assert_eq!(
                suggestions.first().map(|s| s.name.as_str()),
                first_suggestion
            );
        }
        other @ ResolveError::AmbiguousName { .. } => panic!("unexpected {other:?}"),
    }
    assert!(!resolution.resolved.iter().any(|r| r.name == name));
    assert_eq!(entry.entry.count, 1);
    assert!(entry.index < 12);
}

#[test]
fn precon_cards_resolve_and_agree_with_their_scryfall_ids() {
    let precon = mtgjson::parse_deck_str(MTGJSON_DECK).unwrap();
    let resolution = resolve_deck(&index(), &precon.to_parsed_deck());
    assert!(
        resolution.unresolved.is_empty(),
        "{:?}",
        resolution.unresolved
    );
    assert_eq!(resolution.resolved.len(), 5);

    let mtgjson_cards = precon.commander.iter().chain(&precon.main_board);
    for (card, resolved) in mtgjson_cards.zip(&resolution.resolved) {
        if let Some(oracle_id) = &card.oracle_id {
            assert_eq!(&resolved.oracle_id, oracle_id, "{}", card.name);
        }
    }
}

#[test]
fn art_series_cards_do_not_make_front_face_names_ambiguous() {
    let index = index();
    assert!(matches!(index.find("Lightning Bolt"), Lookup::Found(_)));
    let all: Vec<_> = records()
        .into_iter()
        .filter(|c| c.name.starts_with("Lightning Bolt"))
        .collect();
    assert_eq!(all.len(), 2);
    assert_eq!(index.len(), 20);
}
