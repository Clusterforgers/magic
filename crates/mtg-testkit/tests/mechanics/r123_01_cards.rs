//! Goal: at 2 to 4 seats, Pavel Chekov lets its controller replace a draw by revealing until a land or a nonland card,
//! the others to the bottom, or draw as usual; Silhouette prevents the damage a spell targeting the creature causes it
//! while damage that does not target it, or that targets another creature, is dealt; Glava may make the X of a permanent
//! spell or of an activated ability 5, once each turn; Timesifter gives the extra turn to the player who exiled the
//! greatest mana value, a tie repeated among the tied players alone; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{NumberChoice, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, forest, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack and the pending triggers are empty: `pick` answers the decisions it knows, the rest are
/// answered passively.
fn r123_resolve(
    driver: &mut Driver,
    mut pick: impl FnMut(&Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let busy = !driver.state().stack().is_empty()
            || !driver.state().pending_triggers().is_empty()
            || driver.state().resolving().is_some();
        let decision = driver.decision()?.clone();
        if !matches!(decision.kind, DecisionKind::Priority(_))
            && let Some(answer) = pick(&decision)
        {
            driver.answer(answer)?;
            continue;
        }
        match &decision.kind {
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the resolution".into()))
}

/// The log replays to the live state.
fn r123_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A card of the given type and cost with one ability (JSON).
fn r123_card(name: &str, kind: CardType, generic: u32, rule: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(rule).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(generic, &[]))
            .with_rules(rule),
    )
}

/// The names of the cards in a zone, in its order.
fn r123_names(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect()
}

// Pavel Chekov, First Posting

#[rstest]
#[case::a_land(Some(0), &["Grizzly Bears", "Grizzly Bears", "Forest", "Island"], "Forest", &["Island", "Grizzly Bears", "Grizzly Bears"])]
#[case::a_nonland(Some(1), &["Forest", "Forest", "Grizzly Bears", "Island"], "Grizzly Bears", &["Island", "Forest", "Forest"])]
#[case::declined(None, &["Grizzly Bears", "Forest", "Island"], "Grizzly Bears", &["Forest", "Island"])]
fn r123_chekov_reveals_until_the_kind_chosen_or_draws(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: Option<usize>,
    #[case] library: &[&str],
    #[case] got: &str,
    #[case] left: &[&str],
) -> Result<(), ScriptError> {
    let cards: Vec<_> = library
        .iter()
        .map(|name| match *name {
            "Forest" => forest(),
            "Island" => island(),
            _ => bear(),
        })
        .collect();
    let peek = r123_card(
        "Peek",
        CardType::Sorcery,
        0,
        r#"{"Spell":{"effects":[{"Draw":{}}]}}"#,
    );
    let scenario = Scenario::new(seats)
        .hand(0, [peek])
        .library(0, cards)
        .battlefield(0, [ready(card("Pavel Chekov, First Posting").unwrap())])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Peek")?;
    r123_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(kind.is_some()))),
        DecisionKind::ChooseOption(_) => kind.map(|at| Answer::ChooseOption(OptionChoice(at))),
        _ => None,
    })?;
    assert_eq!(r123_names(&driver, Zone::Hand(pid(0))), [got]);
    let mut top_first = r123_names(&driver, Zone::Library(pid(0)));
    top_first.reverse();
    assert_eq!(top_first, left);
    r123_replays(&driver);
    Ok(())
}

// Silhouette

#[rstest]
#[case::a_spell_targeting_it("Zap", true, 2)]
#[case::damage_to_each_creature("Quake", false, 0)]
#[case::a_spell_targeting_another("Zap", false, 0)]
fn r123_silhouette_prevents_the_damage_of_a_spell_that_targets_the_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] at_bear: bool,
    #[case] bears: usize,
) -> Result<(), ScriptError> {
    let zap = r123_card(
        "Zap",
        CardType::Instant,
        0,
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
    );
    let quake = r123_card(
        "Quake",
        CardType::Sorcery,
        0,
        r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":3}}]}}"#,
    );
    let scenario = Scenario::new(seats)
        .hand(0, [card("Silhouette").unwrap(), zap, quake])
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(bear()),
                ready(bear()),
            ],
        )
        .battlefield(seats - 1, [ready(free_creature("Ogre", 3, 3))])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let bears_now: Vec<_> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == "Grizzly Bears")
        .collect();
    let shielded = mtg_engine::choose::Target::Object(
        driver.state().object(bears_now[0]).unwrap().reference(),
    );
    let other = mtg_engine::choose::Target::Object(
        driver.state().object(bears_now[1]).unwrap().reference(),
    );
    driver.begin_cast("Silhouette")?;
    r123_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseTargets(_) => {
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![shielded])))
        }
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    })?;
    driver.begin_cast(spell)?;
    let aim = if at_bear { shielded } else { other };
    r123_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseTargets(_) => {
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![aim])))
        }
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    })?;
    // The shielded Bear lives through the targeted Zap only; the other Bear dies to whatever hits it.
    let alive = r123_names(&driver, Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Grizzly Bears")
        .count();
    let expected = if spell == "Quake" {
        bears
    } else {
        1 + usize::from(at_bear)
    };
    assert_eq!(alive, expected);
    r123_replays(&driver);
    Ok(())
}

// Glava, Five-Advents Mage

#[rstest]
#[case::a_creature_spell_yes(true, true, 5)]
#[case::a_creature_spell_no(true, false, 1)]
#[case::an_ability_yes(false, true, 5)]
#[case::an_ability_no(false, false, 1)]
fn r123_glava_may_make_x_five_once_each_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: bool,
    #[case] accept: bool,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let well = r123_card(
        "Well",
        CardType::Artifact,
        0,
        r#"{"Activated":{"cost":"{X}","effects":[{"GainLife":{"amount":"X"}}]}}"#,
    );
    let scenario = Scenario::new(seats)
        .hand(
            0,
            [card("Endless One").unwrap(), card("Endless One").unwrap()],
        )
        .battlefield(
            0,
            [
                ready(card("Glava, Five-Advents Mage").unwrap()),
                ready(well),
                ready(forest()),
                ready(forest()),
            ],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    // Twice with X 1: the second time Glava's ability can no longer change it after a yes.
    let mut asked = 0;
    for _ in 0..2 {
        if spell {
            driver.begin_cast("Endless One")?;
        } else {
            driver.begin_activation("Well")?;
        }
        r123_resolve(&mut driver, |decision| match &decision.kind {
            DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(1))),
            DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
            DecisionKind::YesNo(_) => {
                asked += 1;
                Some(Answer::YesNo(YesNoChoice(accept)))
            }
            _ => None,
        })?;
    }
    assert_eq!(asked, if accept { 1 } else { 2 });
    if spell {
        let mut counters: Vec<u32> = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .filter(|object| object.card().name == "Endless One")
            .map(|object| {
                object
                    .permanent()
                    .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne))
            })
            .collect();
        counters.sort_unstable();
        assert_eq!(counters, [1, x]);
    } else {
        let gained = driver.state().player(pid(0)).unwrap().life() - life;
        assert_eq!(gained, i32::try_from(x + 1).unwrap());
    }
    r123_replays(&driver);
    Ok(())
}

// Timesifter

/// A free card of mana value `value`: a land for 0.
fn r123_valued(value: u32) -> Arc<CardDefinition> {
    if value == 0 {
        forest()
    } else {
        Arc::new(
            CardDefinition::new(format!("Rock {value}"))
                .with_type(CardType::Artifact)
                .with_mana_cost(cost(value, &[])),
        )
    }
}

#[rstest]
#[case::the_active_player_highest(&[5, 2, 1, 0], &[], 0)]
#[case::an_opponent_highest(&[2, 6, 1, 0], &[], 1)]
#[case::a_tie_broken_among_the_tied(&[3, 3, 1, 0], &[1, 4, 9, 9], 1)]
fn r123_timesifter_gives_the_extra_turn_to_the_greatest_mana_value(
    #[values(2, 3, 4)] seats: usize,
    #[case] tops: &[u32],
    #[case] seconds: &[u32],
    #[case] winner: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Timesifter").unwrap())])
        .at(2, 0, Step::Untap);
    for (seat, top) in tops.iter().enumerate().take(seats) {
        let mut library = vec![r123_valued(*top)];
        library.extend(seconds.get(seat).map(|value| r123_valued(*value)));
        library.extend((0..3).map(|_| forest()));
        scenario = scenario.library(seat, library);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    r123_resolve(&mut driver, |_| None)?;
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    let extra: Vec<_> = driver
        .state()
        .extra_turns()
        .iter()
        .map(|turn| turn.player)
        .collect();
    assert_eq!(extra, [pid(winner)]);
    let exiled = driver.state().zone(Zone::Exile).len();
    let tied = !seconds.is_empty();
    assert_eq!(exiled, seats + if tied { 2 } else { 0 });
    r123_replays(&driver);
    Ok(())
}
