//! Goal: cost changes that count for the spell being cast play their Oracle text at tables of two to four seats: Hum of
//! the Radix adds {1} for each artifact the caster controls (another player's artifacts and nonartifact spells aside),
//! Locket of Yesterdays takes {1} for each card of the spell's name in its controller's graveyard (another name, another
//! graveyard and an opponent's spell aside), and Battlefield Thaumaturge takes {1} for each creature an instant or
//! sorcery of its controller targets (a player target, a creature spell and an opponent's spell aside).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_testkit::cards::{bear, cost, damage, forest, giant, mountain, one_target};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;

/// The cost as the engine writes it.
fn r402_cost(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

/// A colorless artifact for `{3}` with no ability.
fn r402_trinket(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(3, &[])),
    )
}

/// A sorcery for `{4}{R}` with a target creature and a second target of any kind, 1 damage to each.
fn r402_volley() -> Arc<CardDefinition> {
    let creature = Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)]));
    Arc::new(
        CardDefinition::new("Twin Volley")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(4, &[mtg_core::color::Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(creature), one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 1), damage(Ref::Target(1), 1)],
            }),
    )
}

/// The total cost of the spell called `name` once seat `caster` has begun casting it (and chosen `targets`, by name
/// or `seat:N` for a player), with eight Mountains and eight Forests beside the permanents `board` gives each seat.
fn r402_cost_of(
    mut scenario: Scenario,
    mut board: Vec<Vec<PresetPermanent>>,
    caster: usize,
    name: &str,
    targets: &[&str],
) -> Result<String, ScriptError> {
    for (seat, permanents) in board.iter_mut().enumerate() {
        permanents.extend(lands(8, mountain));
        permanents.extend(lands(8, forest));
        scenario = scenario.battlefield(seat, std::mem::take(permanents));
    }
    let scenario = scenario.at(1, caster, mtg_engine::turn::Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(name)?;
    // One target for each specification, asked in turn.
    for target in targets {
        let chosen = match target.strip_prefix("seat:") {
            Some(seat) => driver.target_player(seat.parse().unwrap()),
            None => driver.target_named(target)?,
        };
        driver.choose_targets(&[chosen])?;
    }
    Ok(driver.pay_options()?.cost.to_string())
}

#[rstest]
#[case::two_seats_none(2, 0, 0, "Sol Ring", "{1}")]
#[case::two_seats_two(2, 2, 0, "Sol Ring", "{3}")]
#[case::three_seats_others_artifacts(3, 0, 3, "Sol Ring", "{1}")]
#[case::four_seats_one(4, 1, 2, "Sol Ring", "{2}")]
#[case::a_creature_spell(3, 2, 0, "Grizzly Bears", "{1}{G}")]
fn r402_hum_counts_the_casters_artifacts(
    #[case] seats: usize,
    #[case] own: usize,
    #[case] others: usize,
    #[case] name: &str,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    // The Hum is the last seat's, so it taxes the caster as it taxes everyone.
    let hum = card("Hum of the Radix").unwrap();
    let mut board: Vec<Vec<PresetPermanent>> = vec![Vec::new(); seats];
    board[seats - 1].push(ready(hum));
    board[0].extend((0..own).map(|i| ready(r402_trinket(&format!("Own {i}")))));
    board[1].extend((0..others).map(|i| ready(r402_trinket(&format!("Other {i}")))));
    let scenario = Scenario::new(seats).hand(0, vec![bear(), card("Sol Ring").unwrap()]);
    assert_eq!(
        r402_cost_of(scenario, board, 0, name, &[])?,
        r402_cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::none(2, 0, 0, 0, "Craw Wurm", "{3}{G}{G}")]
#[case::two_of_its_name(2, 2, 0, 0, "Craw Wurm", "{1}{G}{G}")]
#[case::four_of_its_name(3, 4, 0, 0, "Craw Wurm", "{G}{G}")]
#[case::other_names_only(4, 0, 3, 0, "Craw Wurm", "{3}{G}{G}")]
#[case::an_opponents_graveyard(3, 0, 0, 3, "Craw Wurm", "{3}{G}{G}")]
fn r402_locket_counts_cards_of_the_spells_name(
    #[case] seats: usize,
    #[case] same: usize,
    #[case] other: usize,
    #[case] opponents: usize,
    #[case] name: &str,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let locket = card("Locket of Yesterdays").unwrap();
    let mut graveyard: Vec<Arc<CardDefinition>> = (0..same).map(|_| giant()).collect();
    graveyard.extend((0..other).map(|_| bear()));
    let mut board: Vec<Vec<PresetPermanent>> = vec![Vec::new(); seats];
    board[0].push(ready(locket));
    let scenario = Scenario::new(seats)
        .graveyard(0, graveyard)
        .graveyard(1, (0..opponents).map(|_| giant()))
        .hand(0, vec![giant()]);
    assert_eq!(
        r402_cost_of(scenario, board, 0, name, &[])?,
        r402_cost(expected)
    );
    Ok(())
}

#[test]
fn r402_locket_spares_an_opponents_spell() -> Result<(), ScriptError> {
    let locket = card("Locket of Yesterdays").unwrap();
    let board = vec![vec![ready(locket)], Vec::new()];
    let scenario = Scenario::new(2)
        .graveyard(1, vec![giant(), giant()])
        .hand(1, vec![giant()]);
    assert_eq!(
        r402_cost_of(scenario, board, 1, "Craw Wurm", &[])?,
        r402_cost("{3}{G}{G}")
    );
    Ok(())
}

#[rstest]
#[case::two_creatures(2, 0, &["Grizzly Bears", "Craw Wurm"], "{2}{R}")]
#[case::a_creature_and_a_player(3, 0, &["Grizzly Bears", "seat:1"], "{3}{R}")]
#[case::an_opponents_spell(4, 1, &["Grizzly Bears", "Craw Wurm"], "{4}{R}")]
fn r402_thaumaturge_counts_creature_targets(
    #[case] seats: usize,
    #[case] caster: usize,
    #[case] targets: &[&str],
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let thaumaturge = card("Battlefield Thaumaturge").unwrap();
    let mut board: Vec<Vec<PresetPermanent>> = vec![Vec::new(); seats];
    board[0].push(ready(thaumaturge));
    board[1].extend([ready(bear()), ready(giant())]);
    let scenario = Scenario::new(seats).hand(caster, vec![r402_volley()]);
    assert_eq!(
        r402_cost_of(scenario, board, caster, "Twin Volley", targets)?,
        r402_cost(expected)
    );
    Ok(())
}

#[test]
fn r402_thaumaturge_spares_a_creature_spell() -> Result<(), ScriptError> {
    let thaumaturge = card("Battlefield Thaumaturge").unwrap();
    let board = vec![vec![ready(thaumaturge)], Vec::new()];
    let scenario = Scenario::new(2).hand(0, vec![giant()]);
    assert_eq!(
        r402_cost_of(scenario, board, 0, "Craw Wurm", &[])?,
        r402_cost("{3}{G}{G}")
    );
    Ok(())
}
