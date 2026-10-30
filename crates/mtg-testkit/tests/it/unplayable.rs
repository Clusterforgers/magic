//! Goal: the priority offer names, for each card of the holder's hand and command zone it leaves out, the first reason in the order of `Unplayable` (timing, land played, mana after cost changes, targets, other costs, restrictions), honours extra land plays, flash and "as though it had flash", and in whole bot games the reasons and the offered plays are disjoint and together cover the hand and the command zone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{DecisionKind, PriorityOptions, Unplayable};
use mtg_engine::effects::Duration;
use mtg_engine::object::ObjectKind;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, GameState};
use mtg_testkit::bot::apply_answer;
use mtg_testkit::cards::{cost, forest, island, keyworded_creature, mountain, swamp};
use mtg_testkit::cost_cards::helm;
use mtg_testkit::{Driver, RandomBot, Scenario, commander_fuzz_setup, fuzz_setup, ready};
use proptest::prelude::*;
use rstest::rstest;

fn card(name: &str) -> Arc<CardDefinition> {
    mtg_testkit::corpus::card(name).unwrap()
}

fn rule(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A free instant that draws a card, cast only during combat.
fn combat_trick() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Combat Trick")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rule(r#"{"CastOnly":{"condition":"InCombat"}}"#))
            .with_rules(rule(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#)),
    )
}

/// An artifact whose controller may play an additional land each turn.
fn explorer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Explorer")
            .with_type(CardType::Artifact)
            .with_rules(rule(r#"{"Rule":{"rule":{"AdditionalLandPlays":{}}}}"#)),
    )
}

fn flash_bear() -> Arc<CardDefinition> {
    keyworded_creature("Flash Bear", 1, &[], 2, 2, &[Keyword::Flash])
}

/// Seat 0 holds `hand` in `step` of its own turn with `lands` ready; seat 1 controls `theirs`.
fn table(
    step: Step,
    hand: Vec<Arc<CardDefinition>>,
    lands: Vec<Arc<CardDefinition>>,
    theirs: Vec<Arc<CardDefinition>>,
) -> Driver {
    let scenario = Scenario::new(2)
        .at(2, 0, step)
        .hand(0, hand)
        .battlefield(0, lands.into_iter().map(ready))
        .battlefield(1, theirs.into_iter().map(ready))
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    Driver::from_scenario(scenario).unwrap()
}

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("not a priority decision: {other:?}"),
    }
}

/// The reason the offer gives for `name`, or `None` when it offers the card.
fn reason_of(driver: &Driver, name: &str) -> Option<Unplayable> {
    let id = driver.id_of(name).unwrap();
    let options = options(driver);
    let reason = options
        .unplayable
        .iter()
        .find(|entry| entry.card == id)
        .map(|entry| entry.reason);
    assert_eq!(
        reason.is_none(),
        offered(&options).contains(&id),
        "{name} is either offered or has a reason"
    );
    reason
}

fn offered(options: &PriorityOptions) -> BTreeSet<ObjectId> {
    options
        .playable_lands
        .iter()
        .chain(&options.castable)
        .chain(options.faces.iter().map(|face| &face.card))
        .chain(&options.suspendable)
        .chain(&options.foretellable)
        .copied()
        .collect()
}

type Cards = fn() -> Vec<Arc<CardDefinition>>;

#[rstest]
#[case::a_sorcery_speed_creature_in_upkeep(Step::Upkeep, || vec![card("Grizzly Bears")], || vec![forest(), forest()], Vec::new, "Grizzly Bears", Some(Unplayable::Timing))]
#[case::a_land_in_upkeep(Step::Upkeep, || vec![forest()], Vec::new, Vec::new, "Forest", Some(Unplayable::Timing))]
#[case::a_land_in_the_main_phase(Step::Main1, || vec![forest()], Vec::new, Vec::new, "Forest", None)]
#[case::an_instant_in_upkeep(Step::Upkeep, || vec![card("Opt")], || vec![island()], Vec::new, "Opt", None)]
#[case::flash_in_upkeep(Step::Upkeep, || vec![flash_bear()], || vec![forest()], Vec::new, "Flash Bear", None)]
#[case::flash_in_upkeep_without_mana(Step::Upkeep, || vec![flash_bear()], Vec::new, Vec::new, "Flash Bear", Some(Unplayable::Mana))]
#[case::as_though_it_had_flash(Step::Upkeep, || vec![card("Grizzly Bears")], || vec![forest(), forest(), card("Vedalken Orrery")], Vec::new, "Grizzly Bears", None)]
#[case::too_little_mana(Step::Main1, || vec![card("Grizzly Bears")], || vec![forest()], Vec::new, "Grizzly Bears", Some(Unplayable::Mana))]
#[case::a_cost_increase(Step::Main1, || vec![card("Opt")], || vec![island()], || vec![card("Thalia, Guardian of Thraben")], "Opt", Some(Unplayable::Mana))]
#[case::a_cost_reduction(Step::Main1, || vec![card("Grizzly Bears")], || vec![forest(), helm().unwrap()], Vec::new, "Grizzly Bears", None)]
#[case::no_legal_target(Step::Main1, || vec![card("Doom Blade")], || vec![swamp(), swamp()], Vec::new, "Doom Blade", Some(Unplayable::Targets))]
#[case::a_legal_target(Step::Main1, || vec![card("Doom Blade")], || vec![swamp(), swamp()], || vec![card("Grizzly Bears")], "Doom Blade", None)]
#[case::mana_before_targets(Step::Main1, || vec![card("Doom Blade")], Vec::new, Vec::new, "Doom Blade", Some(Unplayable::Mana))]
#[case::nothing_to_sacrifice(Step::Main1, || vec![card("Altar's Reap")], || vec![swamp(), swamp()], Vec::new, "Altar's Reap", Some(Unplayable::Cost))]
#[case::nothing_to_discard(Step::Main1, || vec![card("Thrill of Possibility")], || vec![mountain(), mountain()], Vec::new, "Thrill of Possibility", Some(Unplayable::Cost))]
#[case::something_to_discard(Step::Main1, || vec![card("Thrill of Possibility"), forest()], || vec![mountain(), mountain()], Vec::new, "Thrill of Possibility", None)]
#[case::cast_only_in_combat(Step::Main1, || vec![combat_trick()], Vec::new, Vec::new, "Combat Trick", Some(Unplayable::Restricted))]
fn the_offer_says_why_a_card_cannot_be_played(
    #[case] step: Step,
    #[case] hand: Cards,
    #[case] lands: Cards,
    #[case] theirs: Cards,
    #[case] name: &str,
    #[case] expected: Option<Unplayable>,
) {
    let driver = table(step, hand(), lands(), theirs());
    assert_eq!(reason_of(&driver, name), expected);
}

#[rstest]
#[case::one_land_a_turn(false, Some(Unplayable::LandPlayed))]
#[case::an_extra_land_play(true, None)]
fn a_second_land_waits_for_a_land_play(#[case] extra: bool, #[case] expected: Option<Unplayable>) {
    let permanents = if extra { vec![explorer()] } else { Vec::new() };
    let mut driver = table(Step::Main1, vec![forest(), island()], permanents, Vec::new());
    driver.play_land("Forest").unwrap();
    assert_eq!(reason_of(&driver, "Island"), expected);
}

#[test]
fn a_rule_that_forbids_casting_comes_last() {
    let mut driver = table(
        Step::Main1,
        vec![card("Grizzly Bears"), card("Opt")],
        vec![forest(), forest()],
        Vec::new(),
    );
    assert_eq!(reason_of(&driver, "Grizzly Bears"), None);
    let forbid = Rule::CantCastSpells {
        who: vec![PlayerId(0)],
        turn: None,
    };
    driver.add_rule(forbid, Duration::UntilEndOfTurn).unwrap();
    assert_eq!(reason_of(&driver, "Grizzly Bears"), Some(Unplayable::Restricted));
    assert_eq!(reason_of(&driver, "Opt"), Some(Unplayable::Mana));
}

/// The cards of `player`'s hand and command zone, in zone order.
fn own_cards(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    [Zone::Hand(player), Zone::Command(player)]
        .into_iter()
        .flat_map(|zone| state.zone(zone).iter().copied())
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.kind() == ObjectKind::Card)
        })
        .collect()
}

/// Plays a bot game of `setup` and checks every priority offer on the way: what it offers and what it explains share no
/// card, and the explained cards are the rest of the holder's hand and command zone, in zone order.
fn check_game(setup: GameSetup, seed: u64, players: usize) -> Result<(), String> {
    let mut engine = Engine::new(setup).map_err(|error| error.to_string())?;
    let mut bots: Vec<RandomBot> = (0..players as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    for step in 0..600 {
        let Some(decision) = engine.decision().cloned() else {
            return Ok(());
        };
        if let DecisionKind::Priority(options) = &decision.kind {
            let explained: Vec<ObjectId> =
                options.unplayable.iter().map(|entry| entry.card).collect();
            let offered = offered(options);
            let rest: Vec<ObjectId> = own_cards(engine.state(), decision.player)
                .into_iter()
                .filter(|id| !offered.contains(id))
                .collect();
            if explained.iter().any(|id| offered.contains(id)) {
                return Err(format!("seed {seed} step {step}: offered and explained"));
            }
            if explained != rest {
                return Err(format!(
                    "seed {seed} step {step}: explained {explained:?}, left out {rest:?}"
                ));
            }
        }
        let answer = bots[decision.player.index()].choose_in(&decision, engine.state());
        apply_answer(&mut engine, &decision, answer).map_err(|error| error.to_string())?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    #[test]
    fn the_reasons_and_the_offer_cover_the_hand(players in 2usize..=4, seed in any::<u64>()) {
        if let Err(problem) = check_game(fuzz_setup(players, seed), seed, players) {
            prop_assert!(false, "{}", problem);
        }
    }

    #[test]
    fn the_reasons_and_the_offer_cover_the_command_zone(players in 2usize..=4, seed in any::<u64>()) {
        let setup = commander_fuzz_setup(players, seed)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        if let Err(problem) = check_game(setup, seed, players) {
            prop_assert!(false, "{}", problem);
        }
    }
}
