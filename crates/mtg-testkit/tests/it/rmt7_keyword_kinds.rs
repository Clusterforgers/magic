//! Goal: a change that takes away a kind of keyword (`RemoveKeywordKind`: all landwalk, protection, ward) removes every keyword of that kind whatever its parameter and nothing else, at two to four seats, on a target ("Target creature loses all landwalk abilities until end of turn") and on a group with plain keywords beside it ("Permanents your opponents control lose hexproof, indestructible, protection, shroud, and ward until end of turn"); the keywords come back as the turn ends and the log replays to the live state. A planeswalker given a loyalty ability by a static ability (Kiora of Salt and Sand) activates it for its loyalty.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::keywords::keywords_of;
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

const KEYWORDS: [&str; 6] = [
    r#"{"Keyword":{"Landwalk":[{"Subtype":"Island"}]}}"#,
    r#"{"Keyword":{"Landwalk":[{"Subtype":"Swamp"}]}}"#,
    r#"{"Keyword":{"Protection":[{"Color":"Red"}]}}"#,
    r#"{"Keyword":{"Ward":"{2}"}}"#,
    r#"{"Keyword":"Flying"}"#,
    r#"{"Keyword":"Indestructible"}"#,
];

/// A 2/2 with two landwalks, protection from red, ward {2}, flying and indestructible.
fn walker(name: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for json in KEYWORDS {
        let ability: Ability = serde_json::from_str(json).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

/// A sorcery for {0} with the given spell ability.
fn sorcery(rule: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(rule).unwrap();
    Arc::new(
        CardDefinition::new("Change")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap())
            .with_rules(rule),
    )
}

/// The names of the keywords the permanent has, in the order of `KEYWORDS`.
fn held(driver: &Driver, name: &str) -> Vec<&'static str> {
    let id = driver.id_of(name).unwrap();
    let mut names: Vec<&'static str> = keywords_of(driver.state(), id)
        .into_iter()
        .map(Keyword::name)
        .collect();
    names.sort_unstable();
    names
}

const ALL: [&str; 6] = [
    "Flying",
    "Indestructible",
    "Landwalk",
    "Landwalk",
    "Protection",
    "Ward",
];

const ON_TARGET: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"RemoveKeywordKind":"KIND"}],"duration":"UntilEndOfTurn"}}]}}"#;

#[rstest]
#[case::landwalk("Landwalk", &["Flying", "Indestructible", "Protection", "Ward"])]
#[case::protection("Protection", &["Flying", "Indestructible", "Landwalk", "Landwalk", "Ward"])]
#[case::ward("Ward", &["Flying", "Indestructible", "Landwalk", "Landwalk", "Protection"])]
fn rmt7_a_target_loses_every_keyword_of_the_kind(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: &str,
    #[case] left: &[&str],
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [PresetPermanent::new(walker("Mine"))])
        .battlefield(1, [PresetPermanent::new(walker("Theirs"))])
        .hand(0, [sorcery(&ON_TARGET.replace("KIND", kind))]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    assert_eq!(held(&driver, "Mine"), ALL);
    let mine = driver.target_named("Mine").unwrap();
    driver.cast_at("Change", &[mine]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(held(&driver, "Mine"), left);
    assert_eq!(held(&driver, "Theirs"), ALL, "only the target changes");
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    driver.idle_until(4, Step::Upkeep).unwrap();
    assert_eq!(held(&driver, "Mine"), ALL, "the change ends with the turn");
}

const OPPONENTS: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"ControlledBy":"Opponents"}]},"mods":[{"RemoveKeyword":"Hexproof"},{"RemoveKeyword":"Indestructible"},{"RemoveKeywordKind":"Protection"},{"RemoveKeyword":"Shroud"},{"RemoveKeywordKind":"Ward"}],"duration":"UntilEndOfTurn"}}]}}"#;

#[rstest]
fn rmt7_the_permanents_of_every_opponent_lose_the_kinds_and_the_keywords(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [PresetPermanent::new(walker("Mine"))])
        .hand(0, [sorcery(OPPONENTS)]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [PresetPermanent::new(walker(&format!("Theirs {seat}")))],
        );
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Change").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(
        held(&driver, "Mine"),
        ALL,
        "your own permanents keep theirs"
    );
    for seat in 1..seats {
        assert_eq!(
            held(&driver, &format!("Theirs {seat}")),
            ["Flying", "Landwalk", "Landwalk"]
        );
    }
}

const KIORA: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Planeswalker"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Loyalty":{"cost":{"Minus":8},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Leviathan","colors":["Blue"],"types":["Creature"],"subtypes":["Leviathan"],"power":8,"toughness":8,"abilities":[{"Keyword":"Hexproof"}]}}}}]}}}]}}"#;

/// A planeswalker given a loyalty ability by a static ability ("Planeswalkers you control have \"[−8]: ...\"") activates it
/// like its own, paying the loyalty, at two to four seats; one with too little loyalty is not offered it, and the
/// log replays to the live state.
#[rstest]
fn rmt7_a_given_loyalty_ability_is_activated_for_its_loyalty(
    #[values(2, 3, 4)] seats: usize,
    #[values(7, 9)] loyalty: u32,
) {
    use mtg_cards::value::CounterKind;
    let giver: Ability = serde_json::from_str(KIORA).unwrap();
    let giver = Arc::new(
        CardDefinition::new("Giver")
            .with_type(CardType::Enchantment)
            .with_rules(giver),
    );
    let walker = Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(loyalty),
    );
    let mut preset = PresetPermanent::new(walker);
    preset.counters = vec![(CounterKind::Loyalty, loyalty)];
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [PresetPermanent::new(giver), preset]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let offered = driver.offered_abilities("Walker").unwrap();
    if loyalty < 8 {
        assert!(offered.is_empty(), "too little loyalty to pay [−8]");
        return;
    }
    assert_eq!(offered.len(), 1, "the given ability");
    driver.begin_ability("Walker", offered[0]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert!(driver.id_of("Leviathan").is_ok(), "the token is made");
    let walker = driver.id_of("Walker").unwrap();
    let left = driver
        .state()
        .object(walker)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::Loyalty));
    assert_eq!(left, Some(loyalty - 8));
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
