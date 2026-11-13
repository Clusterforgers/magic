//! Goal: the r55-21 cards play as printed at two to four seats: Frodo, Adventurous Hobbit tempts only after 3 life gained
//! and draws only once the Ring has tempted its controller twice; Acererak the Archlich returns to its owner's hand and
//! ventures, and its attack makes a Zombie for each opponent who does not sacrifice a creature; Conspiracy Theorist
//! exiles a nonland card discarded in a batch, which may then be cast; King Solomon's Frogs exiles up to one permanent of
//! mana value 3 or greater of each opponent and that permanent's controller draws; Spikeshell Harrier returns an
//! opponent's creature; Breena, the Demagogue rewards an attack on an opponent with more life than another; Call the Spirit
//! Dragons grows a Dragon of each color and wins with five; Desert Warfare returns a sacrificed Desert at the end step. Each
//! log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const FRODO: &str = "Frodo, Adventurous Hobbit";

fn r5521_hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn r5521_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers every pending question, yes to each "may" when `yes`, until the stack and the waiting triggers are empty.
fn r5521_settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let state = driver.state();
        let busy = !state.stack().is_empty() || !state.pending_triggers().is_empty();
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) | DecisionKind::ChooseTargets(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// An artifact sacrificed for these steps (JSON), at no mana.
fn r5521_gadget(name: &str, effects: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["SacrificeSelf"],"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

fn r5521_use(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_ability(name, 0)?;
    r5521_settle(driver, true)
}

#[rstest]
fn r5521_frodo_tempts_after_life_and_draws_on_the_second_temptation(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 3)] gained: i32,
    #[values(0, 1)] tempted: u8,
) -> Result<(), ScriptError> {
    let mut gadgets = vec![
        card(FRODO).unwrap(),
        r5521_gadget(
            "Boon",
            &format!(r#"[{{"GainLife":{{"amount":{}}}}}]"#, gained.max(1)),
        ),
    ];
    gadgets.extend((0..tempted).map(|_| r5521_gadget("Ring", r#"[{"RingTempts":{}}]"#)));
    let scenario = Scenario::new(seats)
        .battlefield(0, gadgets.into_iter().map(ready))
        .library(0, (0..5).map(|n| free_creature(&format!("Page {n}"), 1, 1)));
    let mut driver = Driver::from_scenario(scenario)?;
    if gained > 0 {
        r5521_use(&mut driver, "Boon")?;
    }
    for _ in 0..tempted {
        r5521_use(&mut driver, "Ring")?;
    }
    let before = r5521_hand(&driver, 0);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(FRODO, 1)])?;
    r5521_settle(&mut driver, true)?;
    let tempts = gained >= 3;
    let level = driver.state().ring_level(pid(0));
    assert_eq!(level, tempted + u8::from(tempts));
    let draws = tempts && tempted >= 1;
    assert_eq!(r5521_hand(&driver, 0), before + usize::from(draws));
    r5521_replays(&driver);
    Ok(())
}

#[rstest]
fn r5521_acererak_returns_and_ventures(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [(); 3].map(|()| ready(swamp())))
        .hand(0, [card("Acererak the Archlich").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Acererak the Archlich")?;
    r5521_settle(&mut driver, true)?;
    let hand = common::names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(hand, vec!["Acererak the Archlich".to_owned()]);
    assert!(driver.state().w11d_marker(pid(0)).is_some(), "ventured");
    r5521_replays(&driver);
    Ok(())
}

#[rstest]
fn r5521_acererak_makes_a_zombie_for_each_opponent_who_keeps_their_creatures(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] sacrifice: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Acererak the Archlich").unwrap())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Ogre", 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Acererak the Archlich", 1)])?;
    r5521_settle(&mut driver, sacrifice)?;
    let zombies = common::names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Zombie")
        .count();
    let ogres = common::names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Ogre")
        .count();
    let opponents = seats - 1;
    assert_eq!(zombies, if sacrifice { 0 } else { opponents });
    assert_eq!(ogres, if sacrifice { 0 } else { opponents });
    r5521_replays(&driver);
    Ok(())
}

#[rstest]
fn r5521_conspiracy_theorist_exiles_a_discarded_nonland_card_to_cast(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] exile: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Conspiracy Theorist").unwrap()),
                ready(r5521_gadget("Rummage", r#"[{"Discard":{"count":2}}]"#)),
            ],
        )
        .hand(0, [free_creature("Squire", 1, 2), plains()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Rummage", 0)?;
    r5521_settle(&mut driver, exile)?;
    let exiled = common::names(driver.state(), Zone::Exile);
    if std::env::var("R5521_DEBUG").is_ok() {
        for event in driver.engine().events() {
            println!("{event:?}");
        }
    }
    assert_eq!(
        exiled,
        if exile {
            vec!["Squire".to_owned()]
        } else {
            Vec::new()
        }
    );
    if exile {
        driver.cast("Squire")?;
        r5521_settle(&mut driver, true)?;
        assert!(common::names(driver.state(), Zone::Battlefield).contains(&"Squire".to_owned()));
    }
    r5521_replays(&driver);
    Ok(())
}

#[rstest]
fn r5521_frogs_exile_one_permanent_of_each_opponent_who_then_draws(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rock = || {
        Arc::new(
            CardDefinition::new("Rock")
                .with_type(CardType::Artifact)
                .with_mana_cost(cost(3, &[])),
        )
    };
    let pebble = || {
        Arc::new(
            CardDefinition::new("Pebble")
                .with_type(CardType::Artifact)
                .with_mana_cost(cost(2, &[])),
        )
    };
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [(); 4].map(|()| ready(plains())))
        .hand(0, [card("King Solomon's Frogs").unwrap()]);
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(rock()), ready(pebble())])
            .library(
                seat,
                (0..3).map(|n| free_creature(&format!("Page {n}"), 1, 1)),
            );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let hands: Vec<usize> = (0..seats).map(|seat| r5521_hand(&driver, seat)).collect();
    driver.cast("King Solomon's Frogs")?;
    r5521_settle(&mut driver, true)?;
    // The enters trigger asks for its targets: every Rock (a Pebble's mana value is too low).
    if let DecisionKind::ChooseTargets(_) = &driver.decision()?.kind {
        let rocks: Vec<_> = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| driver.state().object(**id).unwrap().card().name == "Rock")
            .map(|id| {
                mtg_engine::choose::Target::Object(driver.state().object(*id).unwrap().reference())
            })
            .collect();
        driver.choose_targets(&rocks)?;
    }
    r5521_settle(&mut driver, true)?;
    let exiled = common::names(driver.state(), Zone::Exile);
    assert_eq!(exiled.len(), seats - 1, "{exiled:?}");
    assert!(exiled.iter().all(|name| name == "Rock"));
    for seat in 1..seats {
        assert_eq!(r5521_hand(&driver, seat), hands[seat] + 1, "seat {seat}");
    }
    r5521_replays(&driver);
    Ok(())
}

#[rstest]
fn r5521_harrier_returns_an_opponents_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [(); 5].map(|()| ready(mtg_testkit::cards::island())))
        .battlefield(seats - 1, [ready(free_creature("Ogre", 3, 3))])
        .hand(0, [card("Spikeshell Harrier").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Spikeshell Harrier")?;
    r5521_settle(&mut driver, true)?;
    let ogre = driver.target_named("Ogre")?;
    driver.choose_targets(&[ogre])?;
    r5521_settle(&mut driver, true)?;
    let hand = common::names(driver.state(), Zone::Hand(pid(seats - 1)));
    assert_eq!(hand, vec!["Ogre".to_owned()]);
    // Nobody has speed, so no speed changes.
    assert!((0..seats).all(|seat| driver.state().speed(pid(seat)) == 0));
    r5521_replays(&driver);
    Ok(())
}

#[rstest]
fn r5521_breena_rewards_an_attack_on_an_opponent_with_more_life(
    #[values(2, 3, 4)] seats: usize,
    #[values(5, 20)] defender_life: i32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Breena, the Demagogue").unwrap()),
                ready(free_creature("Ogre", 3, 3)),
            ],
        )
        .library(0, (0..3).map(|n| free_creature(&format!("Page {n}"), 1, 1)))
        .life(1, defender_life);
    for seat in 2..seats {
        scenario = scenario.life(seat, 10);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before = r5521_hand(&driver, 0);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 1)])?;
    r5521_settle(&mut driver, true)?;
    // Seat 1 has more life than another opponent only when a third seat at 10 life exists and seat 1 is at 20.
    let rewarded = seats >= 3 && defender_life == 20;
    assert_eq!(r5521_hand(&driver, 0), before + usize::from(rewarded));
    let counters: usize = ["Breena, the Demagogue", "Ogre"]
        .iter()
        .map(|name| {
            let id = driver.id_of(name).unwrap();
            let permanent = driver.state().object(id).unwrap().permanent().unwrap();
            permanent.counters().map(|(_, n)| n as usize).sum::<usize>()
        })
        .sum();
    assert_eq!(counters, if rewarded { 2 } else { 0 });
    r5521_replays(&driver);
    Ok(())
}

/// A Dragon of one color.
fn r5521_dragon(name: &str, color: mtg_core::color::Color) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Dragon")
            .with_mana_cost(cost(4, &[color]))
            .with_power_toughness(4, 4),
    )
}

#[rstest]
fn r5521_spirit_dragons_grow_one_dragon_of_each_color_and_win_with_five(
    #[values(2, 3, 4)] seats: usize,
    #[values(4, 5)] dragons: usize,
) -> Result<(), ScriptError> {
    use mtg_core::color::Color;
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut permanents = vec![ready(card("Call the Spirit Dragons").unwrap())];
    permanents.extend(
        colors
            .iter()
            .take(dragons)
            .enumerate()
            .map(|(n, color)| ready(r5521_dragon(&format!("Dragon {n}"), *color))),
    );
    let scenario = Scenario::new(seats)
        .at(2, seats - 1, Step::End)
        .battlefield(0, permanents)
        .library(0, (0..3).map(|n| free_creature(&format!("Page {n}"), 1, 1)));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Draw).ok();
    if dragons == 5 {
        assert_eq!(
            driver.engine().result(),
            Some(mtg_engine::GameResult::Winner(pid(0)))
        );
    } else {
        assert_eq!(driver.engine().result(), None);
        for n in 0..dragons {
            let id = driver.id_of(&format!("Dragon {n}")).unwrap();
            let permanent = driver.state().object(id).unwrap().permanent().unwrap();
            assert_eq!(
                permanent.counters().map(|(_, n)| n).sum::<u32>(),
                1,
                "Dragon {n}"
            );
        }
    }
    r5521_replays(&driver);
    Ok(())
}

#[rstest]
fn r5521_desert_warfare_returns_a_sacrificed_desert_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ability: Ability =
        serde_json::from_str(r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[]}}"#).unwrap();
    let desert = Arc::new(
        CardDefinition::new("Dune")
            .with_type(CardType::Land)
            .with_subtype("Desert")
            .with_rules(ability),
    );
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Desert Warfare").unwrap()), ready(desert)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Dune", 0)?;
    r5521_settle(&mut driver, true)?;
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(0))),
        vec!["Dune".to_owned()]
    );
    driver.pass_until(Step::End)?;
    r5521_settle(&mut driver, true)?;
    assert!(common::names(driver.state(), Zone::Battlefield).contains(&"Dune".to_owned()));
    assert!(common::names(driver.state(), Zone::Graveyard(pid(0))).is_empty());
    r5521_replays(&driver);
    Ok(())
}
