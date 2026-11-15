//! Goal: every replacement of counters put on a permanent sees the loyalty counters of a planeswalker, at tables of two to four seats: the loyalty it enters with (CR 306.5b, 614.1c) and the loyalty a `+N` ability puts on it as its cost (CR 606.4) are changed by the effects that say "would put" or "would be put" (Vorinclex, Monstrous Raider, Pir, Imaginative Rascal), those that say "an effect would put" (Doubling Season) change the entering loyalty but not the cost, which is no effect (its ruling), a `0` ability puts nothing, a replacement that names who puts the counters looks at the player who cast or activated, a cost whose counters are all replaced away is still paid, and effects that cannot touch a planeswalker (Hardened Scales, Winding Constrictor, Solemnity) leave it alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::ReplacementChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{cost, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A planeswalker of three loyalty with "+1: You gain 1 life." and "0: You gain 2 life.", cast for {1}.
fn walker() -> Arc<CardDefinition> {
    let rules = [
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
        r#"{"Loyalty":{"cost":"Zero","effects":[{"GainLife":{"amount":2}}]}}"#,
    ];
    let mut card = CardDefinition::new("Walker")
        .with_type(CardType::Planeswalker)
        .with_mana_cost(cost(1, &[]))
        .with_loyalty(3);
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(card)
}

fn permanents(names: &[&str]) -> Vec<PresetPermanent> {
    names
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect()
}

/// Passes priority until the stack is empty, answering each question of the replacement step with the effect of the first
/// of `order` that is on offer (the first on offer when none is); returns how many times it was asked.
fn settle(driver: &mut Driver, order: &[&str]) -> Result<u32, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(asked);
        };
        match decision.kind {
            DecisionKind::ChooseReplacement(options) => {
                asked += 1;
                let at = order
                    .iter()
                    .find_map(|want| {
                        options
                            .candidates
                            .iter()
                            .position(|c| name_of(driver.state(), c.source) == *want)
                    })
                    .unwrap_or(0);
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(at)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn loyalty(driver: &Driver) -> u32 {
    let id = driver.id_of("Walker").unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::Loyalty))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// A table where seat `actor` holds the turn and the walker (in hand, or in play when `cast` is false), and `effects` are
/// the permanents of seat `owner`.
fn table(seats: usize, (actor, owner): (usize, usize), effects: &[&str], cast: bool) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, actor, Step::Main1);
    for seat in 0..seats {
        let mut held = if seat == owner {
            permanents(effects)
        } else {
            Vec::new()
        };
        if seat == actor {
            held.push(ready(if cast { island() } else { walker() }));
        }
        scenario = scenario.battlefield(seat, held);
    }
    if cast {
        scenario = scenario.hand(actor, [walker()]);
    }
    scenario
}

/// The loyalty after the walker enters, after its `+1` ability and after its `0` ability, each from a table of its own:
/// `put` is who casts and activates and `owner` whose permanents the effects are.
#[rstest]
#[case::none(&[], (0, 0), 3, 4, 3)]
#[case::vorinclex(&["Vorinclex, Monstrous Raider"], (0, 0), 6, 5, 3)]
#[case::pir(&["Pir, Imaginative Rascal"], (0, 0), 4, 5, 3)]
#[case::doubling_season_enters_but_is_no_cost(&["Doubling Season"], (0, 0), 6, 4, 3)]
#[case::vorinclex_and_season(&["Vorinclex, Monstrous Raider", "Doubling Season"], (0, 0), 12, 5, 3)]
#[case::hardened_scales(&["Hardened Scales"], (0, 0), 3, 4, 3)]
#[case::winding_constrictor(&["Winding Constrictor"], (0, 0), 3, 4, 3)]
#[case::solemnity(&["Solemnity"], (0, 0), 3, 4, 3)]
#[case::their_vorinclex_halves(&["Vorinclex, Monstrous Raider"], (0, 1), 1, 3, 3)]
#[case::their_pir(&["Pir, Imaginative Rascal"], (0, 1), 3, 4, 3)]
#[case::their_doubling_season(&["Doubling Season"], (0, 1), 3, 4, 3)]
#[case::the_second_seat_under_its_own_vorinclex(&["Vorinclex, Monstrous Raider"], (1, 1), 6, 5, 3)]
#[case::the_second_seat_under_the_first_seats_vorinclex(&["Vorinclex, Monstrous Raider"], (1, 0), 1, 3, 3)]
fn a_planeswalkers_loyalty_is_changed_as_it_enters_and_as_its_plus_cost_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] (actor, owner): (usize, usize),
    #[case] entered: u32,
    #[case] after_plus: u32,
    #[case] after_zero: u32,
) -> Result<(), ScriptError> {
    let setup = table(seats, (actor, owner), effects, true).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, (actor, owner), effects, true))?;
    driver.cast("Walker")?;
    settle(&mut driver, &[])?;
    assert_eq!(loyalty(&driver), entered, "entering");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());

    // The walker that is in play already holds its printed three.
    for (ability, expected, gained) in [(0, after_plus, 1), (1, after_zero, 2)] {
        let setup = table(seats, (actor, owner), effects, false)
            .setup()
            .unwrap();
        let mut driver = Driver::from_scenario(table(seats, (actor, owner), effects, false))?;
        let before = life(&driver, actor);
        driver.begin_ability("Walker", ability)?;
        settle(&mut driver, &[])?;
        assert_eq!(loyalty(&driver), expected, "ability {ability}");
        assert_eq!(
            life(&driver, actor) - before,
            gained,
            "ability {ability} resolved"
        );
        let replayed = replay(&setup, driver.engine().events()).unwrap();
        assert_eq!(replayed.state_hash(), driver.state().state_hash());
    }
    Ok(())
}

/// The loyalty the walker holds after it enters, or after its first ability is activated from the three it is printed
/// with, when two effects apply and the player puts `first` first: Pir adds one and Vorinclex doubles, so the order is two
/// different numbers (CR 616.1).
#[rstest]
#[case::pir_first("Pir, Imaginative Rascal", 8, 7)]
#[case::vorinclex_first("Vorinclex, Monstrous Raider", 7, 6)]
fn two_effects_that_change_loyalty_are_applied_in_the_order_the_player_picks(
    #[values(2, 3, 4)] seats: usize,
    #[case] first: &str,
    #[case] entered: u32,
    #[case] after_plus: u32,
) -> Result<(), ScriptError> {
    let effects = ["Pir, Imaginative Rascal", "Vorinclex, Monstrous Raider"];
    let mut driver = Driver::from_scenario(table(seats, (0, 0), &effects, true))?;
    driver.cast("Walker")?;
    let asked = settle(&mut driver, &[first])?;
    assert_eq!((loyalty(&driver), asked), (entered, 1), "entering");

    let mut driver = Driver::from_scenario(table(seats, (0, 0), &effects, false))?;
    driver.begin_ability("Walker", 0)?;
    let asked = settle(&mut driver, &[first])?;
    assert_eq!((loyalty(&driver), asked), (after_plus, 1), "the +1 cost");
    Ok(())
}

/// What the ability on the stack remembers of its source (CR 113.7a) is the loyalty it holds once the cost is paid, as the
/// effects changed it; `owner` is the seat of the effects.
#[rstest]
#[case::none(&[], 0, 4)]
#[case::vorinclex(&["Vorinclex, Monstrous Raider"], 0, 5)]
#[case::doubling_season_is_no_effect_on_a_cost(&["Doubling Season"], 0, 4)]
#[case::their_vorinclex_halves_the_cost_away(&["Vorinclex, Monstrous Raider"], 1, 3)]
fn the_ability_remembers_the_loyalty_its_source_holds_once_the_cost_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] owner: usize,
    #[case] remembered: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, (0, owner), effects, false))?;
    driver.begin_ability("Walker", 0)?;
    assert_eq!(loyalty(&driver), remembered);
    let ability = *driver.state().stack().last().unwrap();
    let held = driver
        .state()
        .object(ability)
        .and_then(|object| object.ability())
        .map(|ability| ability.snapshot().counters.clone())
        .unwrap();
    assert!(
        held.contains(&(CounterKind::Loyalty, remembered)),
        "{held:?}"
    );
    Ok(())
}
