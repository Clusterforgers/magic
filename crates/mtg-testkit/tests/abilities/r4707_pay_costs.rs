//! Goal: "Players can't pay life or sacrifice creatures to cast spells or activate abilities." and its kin play at two
//! to four seats, wherever the ban sits: Angel of Jubilation, Karn's Sylex and Yasharn bar the activated abilities and
//! mana abilities whose costs pay life or sacrifice what they name, leave the others open (a noncreature artifact under
//! Angel, mana abilities under Karn's Sylex), and keep a Phyrexian symbol from being paid with life.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A permanent of one type named `name` with one ability.
fn r4707_permanent(name: &str, kind: CardType, ability: &str) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new(name)
        .with_type(kind)
        .with_rules(serde_json::from_str::<Ability>(ability).unwrap());
    if kind == CardType::Creature {
        definition = definition.with_power_toughness(1, 1);
    }
    Arc::new(definition)
}

/// The permanents of seat 0: the holder of each ability and what its costs may take.
fn r4707_board() -> Vec<PresetPermanent> {
    [
        r4707_permanent(
            "Life Drawer",
            CardType::Artifact,
            r#"{"Activated":{"cost":[{"PayLife":2}],"effects":[{"Draw":{}}]}}"#,
        ),
        r4707_permanent(
            "Creature Eater",
            CardType::Enchantment,
            r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"GainLife":{"amount":1}}]}}"#,
        ),
        r4707_permanent(
            "Artifact Eater",
            CardType::Enchantment,
            r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Artifact"}]}}],"effects":[{"GainLife":{"amount":1}}]}}"#,
        ),
        r4707_permanent(
            "Martyr",
            CardType::Creature,
            r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#,
        ),
        r4707_permanent(
            "Life Land",
            CardType::Land,
            r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        ),
        r4707_permanent(
            "Trinket",
            CardType::Artifact,
            r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#,
        ),
    ]
    .into_iter()
    .map(ready)
    .collect()
}

/// The game with the board on seat 0 and the ban on the seat named, or none.
fn r4707_game(seats: usize, ban: Option<(&str, usize)>) -> Result<Driver, ScriptError> {
    let mut board = r4707_board();
    let mut theirs = Vec::new();
    if let Some((name, seat)) = ban {
        let held = if seat == 0 { &mut board } else { &mut theirs };
        held.push(ready(card(name).unwrap()));
    }
    let mut scenario = Scenario::new(seats).battlefield(0, board);
    if !theirs.is_empty() {
        scenario = scenario.battlefield(1, theirs);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::nothing_barred(None, &["Life Drawer", "Creature Eater", "Artifact Eater", "Martyr"], &[])]
#[case::angel(Some("Angel of Jubilation"), &["Artifact Eater"], &["Life Drawer", "Creature Eater", "Martyr"])]
#[case::karns_sylex(Some("Karn's Sylex"), &["Creature Eater", "Artifact Eater", "Martyr"], &["Life Drawer"])]
#[case::yasharn(Some("Yasharn, Implacable Earth"), &[], &["Life Drawer", "Creature Eater", "Artifact Eater", "Martyr"])]
fn r4707_a_ban_bars_the_activations_whose_costs_it_names(
    #[case] ban: Option<&str>,
    #[case] open: &[&str],
    #[case] barred: &[&str],
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] seat: usize,
) -> Result<(), ScriptError> {
    let driver = r4707_game(seats, ban.map(|name| (name, seat)))?;
    for name in open {
        assert!(
            !driver.offered_abilities(name)?.is_empty(),
            "{name} is open under {ban:?}"
        );
    }
    for name in barred {
        assert!(
            driver.offered_abilities(name)?.is_empty(),
            "{name} is barred under {ban:?}"
        );
    }
    Ok(())
}

#[rstest]
#[case::nothing_barred(None, true, true)]
#[case::angel(Some("Angel of Jubilation"), false, true)]
#[case::karns_sylex_spares_mana_abilities(Some("Karn's Sylex"), true, true)]
#[case::yasharn(Some("Yasharn, Implacable Earth"), false, false)]
fn r4707_a_ban_reads_the_costs_of_mana_abilities_unless_it_spares_them(
    #[case] ban: Option<&str>,
    #[case] life_land: bool,
    #[case] trinket: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r4707_game(seats, ban.map(|name| (name, 1)))?;
    assert_eq!(
        driver.tap_for_mana("Life Land").is_ok(),
        life_land,
        "{ban:?}"
    );
    assert_eq!(driver.tap_for_mana("Trinket").is_ok(), trinket, "{ban:?}");
    Ok(())
}

/// A sorcery that costs {U/P}: without blue mana, only 2 life pays it.
fn r4707_phyrexian_spell() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Phyrexian Thought")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{U/P}").unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap(),
            ),
    )
}

#[rstest]
#[case::nothing_barred(None, true)]
#[case::karns_sylex(Some("Karn's Sylex"), false)]
#[case::yasharn(Some("Yasharn, Implacable Earth"), false)]
fn r4707_a_phyrexian_symbol_is_not_paid_with_life_under_a_ban(
    #[case] ban: Option<&str>,
    #[case] castable: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).hand(0, [r4707_phyrexian_spell()]);
    if let Some(name) = ban {
        scenario = scenario.battlefield(1, [ready(card(name).unwrap())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        driver.begin_cast("Phyrexian Thought").is_ok(),
        castable,
        "{ban:?}"
    );
    Ok(())
}
