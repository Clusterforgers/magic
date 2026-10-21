//! Goal: recorded Scryfall bulk fixtures stream into normalized records that capture every layout, tolerate junk fields, and isolate bad entries.

use std::collections::BTreeMap;
use std::fs::File;

use mtg_core::color::{Color, ColorSet};
use mtg_import::ids::OracleId;
use mtg_import::scryfall::{
    CardRecord, InvalidRecord, Layout, Legality, RulingRecord, RulingSource, ScryfallError, cards,
    cards_jsonl, rulings, rulings_jsonl,
};
use rstest::rstest;

const CARDS_JSON: &str = include_str!("../fixtures/scryfall_oracle_cards.json");
const RULINGS_JSON: &str = include_str!("../fixtures/scryfall_rulings.json");

fn all_cards() -> BTreeMap<String, CardRecord> {
    cards(CARDS_JSON.as_bytes())
        .filter_map(Result::ok)
        .map(|card| (card.name.clone(), card))
        .collect()
}

fn card(name: &str) -> CardRecord {
    all_cards()
        .remove(name)
        .unwrap_or_else(|| panic!("fixture has no card named {name:?}"))
}

fn colors(symbols: &str) -> ColorSet {
    symbols.chars().filter_map(Color::from_symbol).collect()
}

#[test]
fn fixture_has_one_unusable_entry_and_the_rest_load() {
    let results: Vec<_> = cards(CARDS_JSON.as_bytes()).collect();
    assert_eq!(results.len(), 24);
    let errors: Vec<_> = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.as_ref().err().map(|e| (i, e)))
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(matches!(
        errors[0],
        (
            16,
            ScryfallError::Invalid {
                index: 16,
                reason: InvalidRecord::MissingName
            }
        )
    ));
    assert!(!errors[0].1.is_fatal());
}

#[rstest]
#[case("Lightning Bolt", Layout::Normal, 1, "R", "R")]
#[case("Shock", Layout::Normal, 1, "R", "R")]
#[case("Mountain", Layout::Normal, 1, "", "R")]
#[case("Sol Ring", Layout::Normal, 1, "", "")]
#[case("Fire // Ice", Layout::Split, 2, "UR", "UR")]
#[case(
    "Delver of Secrets // Insectile Aberration",
    Layout::Transform,
    2,
    "U",
    "U"
)]
#[case("Bonecrusher Giant // Stomp", Layout::Adventure, 2, "R", "R")]
#[case(
    "Valakut Awakening // Valakut Stoneforge",
    Layout::ModalDfc,
    2,
    "R",
    "R"
)]
#[case("Goblin", Layout::Token, 1, "R", "R")]
#[case("Little Girl", Layout::Normal, 1, "W", "W")]
#[case("Jace, the Mind Sculptor", Layout::Normal, 1, "U", "U")]
#[case("Æther Vial", Layout::Normal, 1, "", "")]
#[case("Lim-Dûl's Vault", Layout::Normal, 1, "UB", "UB")]
#[case("Atraxa, Praetors' Voice", Layout::Normal, 1, "WUBG", "WUBG")]
#[case("Hazmat Suit (Used)", Layout::Normal, 1, "", "")]
#[case("Lightning Bolt // Lightning Bolt", Layout::ArtSeries, 2, "", "")]
#[case("Elspeth, Sun's Champion Emblem", Layout::Emblem, 1, "", "W")]
fn layout_faces_and_colors(
    #[case] name: &str,
    #[case] layout: Layout,
    #[case] faces: usize,
    #[case] card_colors: &str,
    #[case] identity: &str,
) {
    let record = card(name);
    assert_eq!(record.layout, layout);
    assert_eq!(record.faces().count(), faces);
    assert_eq!(record.colors, colors(card_colors));
    assert_eq!(record.color_identity, colors(identity));
}

#[rstest]
#[case("Lightning Bolt", "{R}", 1.0, "Instant")]
#[case("Mountain", "", 0.0, "Basic Land — Mountain")]
#[case("Sol Ring", "{1}", 1.0, "Artifact")]
#[case("Fire // Ice", "{1}{R} // {1}{U}", 4.0, "Instant // Instant")]
#[case(
    "Delver of Secrets // Insectile Aberration",
    "{U}",
    1.0,
    "Creature — Human Wizard // Creature — Human Insect"
)]
#[case(
    "Bonecrusher Giant // Stomp",
    "{2}{R} // {1}{R}",
    3.0,
    "Creature — Giant // Instant — Adventure"
)]
#[case(
    "Valakut Awakening // Valakut Stoneforge",
    "{2}{R}",
    2.0,
    "Instant // Land"
)]
#[case("Goblin", "", 0.0, "Token Creature — Goblin")]
#[case("Little Girl", "{HW}", 0.5, "Creature — Human Child")]
#[case(
    "Jace, the Mind Sculptor",
    "{2}{U}{U}",
    4.0,
    "Legendary Planeswalker — Jace"
)]
#[case(
    "Atraxa, Praetors' Voice",
    "{G}{W}{U}{B}",
    4.0,
    "Legendary Creature — Phyrexian Angel Horror"
)]
#[case("Hazmat Suit (Used)", "{2}", 2.0, "Artifact — Equipment")]
#[case("Lightning Bolt // Lightning Bolt", "", 0.0, "Card // Card")]
fn cost_value_and_type_line(
    #[case] name: &str,
    #[case] mana_cost: &str,
    #[case] mana_value: f64,
    #[case] type_line: &str,
) {
    let record = card(name);
    assert_eq!(record.mana_cost, mana_cost);
    assert!((record.mana_value - mana_value).abs() < f64::EPSILON);
    assert_eq!(record.type_line, type_line);
}

#[rstest]
#[case(
    "Delver of Secrets // Insectile Aberration",
    0,
    "Delver of Secrets",
    "{U}",
    Some("1"),
    Some("1")
)]
#[case(
    "Delver of Secrets // Insectile Aberration",
    1,
    "Insectile Aberration",
    "",
    Some("3"),
    Some("2")
)]
#[case("Fire // Ice", 0, "Fire", "{1}{R}", None, None)]
#[case("Fire // Ice", 1, "Ice", "{1}{U}", None, None)]
#[case(
    "Bonecrusher Giant // Stomp",
    0,
    "Bonecrusher Giant",
    "{2}{R}",
    Some("4"),
    Some("3")
)]
#[case("Bonecrusher Giant // Stomp", 1, "Stomp", "{1}{R}", None, None)]
#[case(
    "Valakut Awakening // Valakut Stoneforge",
    1,
    "Valakut Stoneforge",
    "",
    None,
    None
)]
#[case("Little Girl", 0, "Little Girl", "{HW}", Some("½"), Some("½"))]
fn face_level_fields(
    #[case] name: &str,
    #[case] index: usize,
    #[case] face_name: &str,
    #[case] mana_cost: &str,
    #[case] power: Option<&str>,
    #[case] toughness: Option<&str>,
) {
    let record = card(name);
    let face = record.faces().nth(index).unwrap();
    assert_eq!(face.name, face_name);
    assert_eq!(face.mana_cost, mana_cost);
    assert_eq!(face.power.as_deref(), power);
    assert_eq!(face.toughness.as_deref(), toughness);
}

#[test]
fn face_text_and_loyalty_are_preserved() {
    let jace = card("Jace, the Mind Sculptor");
    assert_eq!(jace.front.loyalty.as_deref(), Some("3"));
    assert!(jace.front.oracle_text.contains("\n0: Draw three cards"));
    let ice = card("Fire // Ice");
    assert_eq!(
        ice.back_faces[0].oracle_text,
        "Tap target permanent.\nDraw a card."
    );
    assert_eq!(ice.keywords, ["Fuse"]);
}

#[rstest]
#[case("Sol Ring", "", true)]
#[case("Arcane Signet", "WUBRG", false)]
#[case("Island", "U", false)]
#[case("Valakut Awakening // Valakut Stoneforge", "R", false)]
#[case("Lightning Bolt", "", false)]
fn produced_mana(#[case] name: &str, #[case] expected: &str, #[case] colorless: bool) {
    let mana = card(name).produced_mana;
    assert_eq!(mana.colors, colors(expected));
    assert_eq!(mana.colorless, colorless);
}

#[rstest]
#[case("Lightning Bolt", "modern", Legality::Legal)]
#[case("Lightning Bolt", "standard", Legality::NotLegal)]
#[case("Lightning Bolt", "no-such-format", Legality::Unknown)]
#[case("Sol Ring", "legacy", Legality::Banned)]
#[case("Sol Ring", "vintage", Legality::Restricted)]
#[case("Sol Ring", "commander", Legality::Legal)]
#[case("Mystery Card", "commander", Legality::Unknown)]
fn legalities(#[case] name: &str, #[case] format: &str, #[case] expected: Legality) {
    assert_eq!(card(name).legality(format), expected);
}

#[test]
fn images_come_from_the_card_or_its_faces() {
    let bolt = card("Lightning Bolt");
    let images = bolt.images.as_ref().unwrap();
    assert!(images.normal.as_deref().unwrap().contains("/normal/front/"));
    assert!(images.png.is_some() && images.art_crop.is_some() && images.border_crop.is_some());
    assert!(images.small.is_some() && images.large.is_some());

    let delver = card("Delver of Secrets // Insectile Aberration");
    assert!(delver.images.is_none());
    let front = delver
        .face_images(0)
        .and_then(|u| u.normal.as_deref())
        .unwrap();
    let back = delver
        .face_images(1)
        .and_then(|u| u.normal.as_deref())
        .unwrap();
    assert!(front.contains("/front/") && back.contains("/back/"));

    let fire_ice = card("Fire // Ice");
    assert!(fire_ice.front.images.is_none());
    assert!(fire_ice.face_images(1).is_some());

    assert!(card("Pyroblast").face_images(0).is_none());
}

#[test]
fn sparse_cards_with_unknown_fields_load_with_defaults() {
    let mystery = card("Mystery Card");
    assert_eq!(mystery.layout, Layout::Normal);
    assert_eq!(mystery.type_line, "");
    assert!(mystery.colors.is_empty());
    assert!(mystery.legalities.is_empty());
    assert_eq!(mystery.lang, "en");
    assert_eq!(mystery.front.name, "Mystery Card");
}

#[test]
fn reversible_cards_take_their_oracle_id_from_the_faces() {
    let record = card("Elspeth, Sun's Champion // Elspeth, Sun's Champion");
    assert_eq!(record.layout, Layout::ReversibleCard);
    assert_eq!(
        record.oracle_id,
        OracleId("2f770deb-5e3a-432e-b27f-f2050d494a16".into())
    );
    assert_eq!(record.faces().count(), 2);
    assert_eq!(record.colors, colors("W"));
}

#[test]
fn printing_metadata_is_captured() {
    let bolt = card("Lightning Bolt");
    assert_eq!(
        (bolt.set.as_str(), bolt.collector_number.as_str()),
        ("m10", "146")
    );
    assert_eq!(bolt.released_at.as_deref(), Some("2009-07-17"));
    assert!(!bolt.digital);
    assert_eq!(
        bolt.printing_id.as_str(),
        "e3285e6b-3e79-4d7c-bf96-d920f973b122"
    );
    assert_eq!(
        bolt.oracle_id.as_str(),
        "4457ed35-7c10-48c8-9776-456485fdf070"
    );
}

#[test]
fn reading_from_a_file_matches_reading_from_memory() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scryfall_oracle_cards.json"
    );
    let from_file: Vec<_> = cards(File::open(path).unwrap())
        .filter_map(Result::ok)
        .collect();
    let from_memory: Vec<_> = cards(CARDS_JSON.as_bytes())
        .filter_map(Result::ok)
        .collect();
    assert_eq!(from_file.len(), 23);
    assert_eq!(from_file, from_memory);
}

#[test]
fn truncating_the_file_anywhere_yields_a_prefix_then_one_fatal_error() {
    let full: Vec<String> = cards(CARDS_JSON.as_bytes())
        .filter_map(Result::ok)
        .map(|c| c.name)
        .collect();
    let bytes = CARDS_JSON.as_bytes();
    let last_close = CARDS_JSON.rfind(']').unwrap();
    for cut in (0..bytes.len()).step_by(11) {
        let results: Vec<_> = cards(&bytes[..cut]).collect();
        let names: Vec<String> = results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(names, full[..names.len()], "cut at {cut}");
        let fatal = results
            .iter()
            .filter(|r| r.as_ref().is_err_and(ScryfallError::is_fatal))
            .count();
        assert_eq!(fatal, usize::from(cut <= last_close), "cut at {cut}");
        if fatal == 1 {
            assert!(
                results
                    .last()
                    .unwrap()
                    .as_ref()
                    .is_err_and(ScryfallError::is_fatal)
            );
        }
    }
}

#[test]
fn rulings_fixture_streams_and_groups_by_card() {
    let results: Vec<_> = rulings(RULINGS_JSON.as_bytes()).collect();
    assert_eq!(results.len(), 7);
    let good: Vec<&RulingRecord> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    assert_eq!(good.len(), 5);
    assert!(matches!(
        results[5],
        Err(ScryfallError::Invalid {
            index: 5,
            reason: InvalidRecord::RulingWithoutComment { .. }
        })
    ));
    assert!(matches!(
        results[6],
        Err(ScryfallError::Invalid {
            index: 6,
            reason: InvalidRecord::RulingWithoutOracleId
        })
    ));

    let mut by_card: BTreeMap<&OracleId, Vec<&RulingRecord>> = BTreeMap::new();
    for ruling in &good {
        by_card.entry(&ruling.oracle_id).or_default().push(ruling);
    }
    let fire_ice = &by_card[&OracleId("ab0f93cc-80ef-4f9c-baf9-f77695a8850b".into())];
    assert_eq!(fire_ice.len(), 2);
    assert_eq!(fire_ice[0].source, RulingSource::Wotc);
    assert_eq!(fire_ice[1].source, RulingSource::Scryfall);
    assert!(fire_ice[1].comment.contains('—'));
    assert_eq!(fire_ice[0].published_at.as_deref(), Some("2004-10-04"));

    let unknown_source = good
        .iter()
        .find(|r| r.source == RulingSource::Unknown)
        .unwrap();
    assert!(unknown_source.comment.contains("never heard of"));
}

fn as_jsonl(array_json: &str, terminator: &str) -> String {
    let items = match serde_json::from_str::<Vec<serde_json::Value>>(array_json) {
        Ok(items) => items,
        Err(e) => panic!("fixture is not a JSON array: {e}"),
    };
    items
        .iter()
        .flat_map(|item| [item.to_string(), terminator.to_owned()])
        .collect()
}

#[rstest]
#[case::lf("\n")]
#[case::crlf("\r\n")]
fn jsonl_and_array_bulk_files_yield_identical_records(#[case] terminator: &str) {
    let from_array: Vec<_> = cards(CARDS_JSON.as_bytes()).collect();
    let from_jsonl: Vec<_> = cards_jsonl(as_jsonl(CARDS_JSON, terminator).as_bytes()).collect();
    assert_eq!(from_array.len(), from_jsonl.len());
    for (a, b) in from_array.iter().zip(&from_jsonl) {
        match (a, b) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
            other => panic!("array and jsonl disagree: {other:?}"),
        }
    }
}

#[test]
fn jsonl_rulings_match_array_rulings() {
    let from_array: Vec<_> = rulings(RULINGS_JSON.as_bytes()).collect();
    let from_jsonl: Vec<_> = rulings_jsonl(as_jsonl(RULINGS_JSON, "\n").as_bytes()).collect();
    assert_eq!(from_array.len(), from_jsonl.len());
    assert!(
        from_array
            .iter()
            .zip(&from_jsonl)
            .all(|(a, b)| a.as_ref().ok() == b.as_ref().ok())
    );
}

#[test]
fn a_bad_jsonl_line_is_isolated_like_a_bad_array_element() {
    let input = "{\"id\":\"p1\",\"oracle_id\":\"o1\",\"name\":\"A\",\"type_line\":\"Instant\"}\nnot json\n{\"id\":\"p2\",\"oracle_id\":\"o2\",\"name\":\"B\",\"type_line\":\"Instant\"}\n";
    let results: Vec<_> = cards_jsonl(input.as_bytes()).collect();
    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok() && results[2].is_ok());
    assert!(matches!(&results[1], Err(e) if !e.is_fatal()));
}
