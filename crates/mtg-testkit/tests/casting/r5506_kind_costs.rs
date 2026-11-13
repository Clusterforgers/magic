//! Goal: a change of the cost of exhaust abilities, of boast abilities (counted per Dragon) and of buyback costs reaches
//! exactly that kind at 2 to 4 seats: Boom Scholar cheapens another permanent's exhaust ability and no other activated
//! ability, Dragonkin Berserker cheapens its boast for each Dragon its controller controls, and Memory Crystal takes {2}
//! off the buyback cost of any player's spell and nothing off the rest of the spell; Tectonic Split takes half its
//! caster's lands, rounded up, counted as it is cast, and none of the opponents'; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The corpus card called `name`, untapped and ready.
fn r5506_card(name: &str) -> mtg_engine::PresetPermanent {
    ready(mtg_testkit::corpus::card(name).unwrap())
}

fn r5506_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// An artifact with "{3}: Draw a card.", which is no exhaust ability.
fn r5506_rod() -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(r#"{"Activated":{"cost":"{3}","effects":[{"Draw":{}}]}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Rod")
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

/// How many abilities of `name` seat 0 is offered with two Forests, Boom Scholar in play or not.
fn r5506_exhaust_offers(seats: usize, scholar: bool, name: &str) -> usize {
    let mut mine = lands(2, forest);
    mine.push(ready(r5506_rod()));
    mine.push(r5506_card("Greenbelt Guardian"));
    if scholar {
        mine.push(r5506_card("Boom Scholar"));
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    let driver = Driver::from_scenario(scenario).unwrap();
    let offered = driver.offered_abilities(name).unwrap().len();
    r5506_replays(&driver);
    offered
}

#[rstest]
fn r5506_exhaust_abilities_of_other_permanents_cost_less(#[values(2, 3, 4)] seats: usize) {
    // {G}: trample is offered either way; the exhaust ({3}{G}) only once it costs {1}{G}.
    assert_eq!(r5506_exhaust_offers(seats, false, "Greenbelt Guardian"), 1);
    assert_eq!(r5506_exhaust_offers(seats, true, "Greenbelt Guardian"), 2);
    // An activated ability that is no exhaust ability keeps its {3}.
    assert_eq!(r5506_exhaust_offers(seats, true, "Rod"), 0);
}

/// Whether Dragonkin Berserker's boast ({4}{R}) is offered after it attacked, with three Mountains and `dragons` Dragons
/// under seat `owner`.
fn r5506_boast_offered(seats: usize, dragons: usize, owner: usize) -> Result<bool, ScriptError> {
    let dragon =
        || Arc::new(Arc::unwrap_or_clone(free_creature("Whelp", 1, 1)).with_subtype("Dragon"));
    let mut mine = lands(3, mountain);
    mine.push(r5506_card("Dragonkin Berserker"));
    let theirs: Vec<_> = (0..dragons).map(|_| ready(dragon())).collect();
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    if owner == 0 {
        mine.extend(theirs);
        scenario = scenario.battlefield(0, mine);
    } else {
        scenario = scenario.battlefield(0, mine).battlefield(owner, theirs);
    }
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Dragonkin Berserker", 1)])?;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::Main2 && decision.player.index() == 0 =>
            {
                let offered = !driver.offered_abilities("Dragonkin Berserker")?.is_empty();
                r5506_replays(&driver);
                return Ok(offered);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the second main phase".into()))
}

#[rstest]
#[case::no_dragon(0, 0, false)]
#[case::one_dragon_is_short(1, 0, false)]
#[case::two_dragons(2, 0, true)]
#[case::an_opponents_dragons_count_for_nothing(2, 1, false)]
fn r5506_boast_costs_less_for_each_dragon(
    #[values(2, 3, 4)] seats: usize,
    #[case] dragons: usize,
    #[case] owner: usize,
    #[case] offered: bool,
) {
    assert_eq!(r5506_boast_offered(seats, dragons, owner).unwrap(), offered);
}

/// The mana value of the total cost of Capsize ({1}{U}{U}, buyback {3}) with `crystals` Memory Crystals under seat
/// `holder`, its buyback paid or not.
fn r5506_capsize_cost(
    seats: usize,
    crystals: usize,
    holder: usize,
    bought: bool,
) -> Result<u32, ScriptError> {
    let mut mine = lands(6, island);
    let mut theirs = vec![ready(free_creature("Bear", 2, 2))];
    for _ in 0..crystals {
        let crystal = r5506_card("Memory Crystal");
        if holder == 0 {
            mine.push(crystal);
        } else {
            theirs.push(crystal);
        }
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Capsize"])
        .battlefield(0, mine)
        .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Capsize")?;
    driver.expect_decision(0, "choose-costs")?;
    let paid = if bought { vec![0] } else { Vec::new() };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(paid)))?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    let cost = driver.pay_options()?.cost.mana_value();
    r5506_replays(&driver);
    Ok(cost)
}

#[rstest]
#[case::no_crystal(0, 0, true, 6)]
#[case::the_buyback_costs_two_less(1, 0, true, 4)]
#[case::an_opponents_crystal_counts(1, 1, true, 4)]
#[case::two_crystals_take_all_of_it(2, 0, true, 3)]
#[case::without_the_buyback_nothing_changes(1, 0, false, 3)]
fn r5506_buyback_costs_cost_less(
    #[values(2, 3, 4)] seats: usize,
    #[case] crystals: usize,
    #[case] holder: usize,
    #[case] bought: bool,
    #[case] cost: u32,
) {
    assert_eq!(
        r5506_capsize_cost(seats, crystals, holder, bought).unwrap(),
        cost
    );
}

/// Seat 0 casts Tectonic Split ({4}{G}{G}) with `forests` Forests and the opponents three each: the lands asked for, how
/// many of them go to the graveyard and whether the enchantment resolves.
fn r5506_split(seats: usize, forests: usize) -> Result<(u32, usize, bool), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Tectonic Split"])
        .battlefield(0, lands(forests, forest));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, lands(3, forest));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Tectonic Split")?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!(
            "the lands to sacrifice are asked, not {} ({:?})",
            driver.decision().unwrap().kind.name(),
            driver
                .engine()
                .events()
                .iter()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
        );
    };
    assert_eq!(options.min, options.max);
    let asked = options.min;
    let picked: Vec<_> = options
        .from
        .iter()
        .copied()
        .take(usize::try_from(asked).unwrap())
        .collect();
    driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(picked)))?;
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let state = driver.state();
    let gone = crate::common::names(
        state,
        mtg_engine::zones::Zone::Graveyard(crate::common::pid(0)),
    )
    .iter()
    .filter(|name| *name == "Forest")
    .count();
    let resolved = crate::common::names(state, mtg_engine::zones::Zone::Battlefield)
        .iter()
        .any(|name| name == "Tectonic Split");
    for seat in 1..seats {
        let theirs = state
            .zone(mtg_engine::zones::Zone::Battlefield)
            .iter()
            .filter(|id| {
                state
                    .object(**id)
                    .is_some_and(|o| o.controller().index() == seat)
            })
            .count();
        assert_eq!(theirs, 3, "the opponents' lands stay");
    }
    r5506_replays(&driver);
    Ok((asked, gone, resolved))
}

#[rstest]
#[case::six_lands(6, 3)]
#[case::seven_rounds_up(7, 4)]
#[case::nine_rounds_up(9, 5)]
#[case::ten_lands(10, 5)]
fn r5506_half_the_lands_rounded_up_are_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[case] forests: usize,
    #[case] half: u32,
) {
    let (asked, gone, resolved) = r5506_split(seats, forests).unwrap();
    assert_eq!(asked, half);
    assert_eq!(gone, usize::try_from(half).unwrap());
    assert!(resolved);
}
