//! Goal: the engine pieces of ep32 play by the rules at tables of two and three seats. "If a spell or ability an opponent controls causes you to discard ~, put it onto the battlefield [with two +1/+1 counters on it] instead of putting it into your graveyard" puts the card onto its owner's battlefield, with its counters, when an opponent's ability makes its owner discard it, and lets it go to the graveyard when its owner's own ability does. Paradigm exiles the spell as it resolves and, once per name, casts one free copy of the card at the beginning of each of its caster's first main phases, none on another player's turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{names, on_battlefield};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "{T}: Each player discards a card."
fn hammer() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"Discard":{"who":"EachPlayer"}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Hammer")
            .with_type(CardType::Artifact)
            .with_rules(rule),
    )
}

/// A creature that enters from its owner's hand, with `counters` +1/+1 counters, when an opponent makes them discard it.
fn smiter(counters: u32) -> Arc<CardDefinition> {
    let with = if counters == 0 {
        String::new()
    } else {
        format!(r#""counters":[["+1/+1",{counters}]]"#)
    };
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Replacement":{{"when":{{"Discarded":{{"what":["This"],"by":"Opponents"}}}},"by":{{"Instead":[{{"Move":{{"what":"This","to":{{"Battlefield":{{{with}}}}}}}}}]}},"from":"Hand"}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Smiter")
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4)
            .with_rules(rule),
    )
}

/// Answers every question with the first cards offered and passes until the stack is empty.
fn settle(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats * 6 {
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            let first = options.from[..1].to_vec();
            driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            continue;
        }
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn a_discard_an_opponent_causes_puts_the_card_onto_the_battlefield(
    #[values(2, 3)] seats: usize,
    #[values(0, 2)] counters: u32,
    #[values(true, false)] by_an_opponent: bool,
) -> Result<(), ScriptError> {
    let actor = usize::from(!by_an_opponent);
    let scenario = Scenario::new(seats)
        .at(1, actor, Step::Main1)
        .battlefield(actor, vec![ready(hammer())])
        .hand(1, vec![smiter(counters)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Hammer")?;
    settle(&mut driver, seats)?;
    let state = driver.state();
    let on = on_battlefield(state, "Smiter");
    assert_eq!(on.is_some(), by_an_opponent);
    let graveyard = names(state, Zone::Graveyard(PlayerId(1)));
    assert_eq!(graveyard.contains(&"Smiter".to_owned()), !by_an_opponent);
    if let Some(id) = on {
        let object = state.object(id).unwrap();
        assert_eq!(object.controller(), PlayerId(1));
        assert_eq!(
            object
                .permanent()
                .unwrap()
                .counters_of(&CounterKind::PlusOnePlusOne),
            counters
        );
    }
    Ok(())
}

/// "You gain 2 life. Paradigm" for {0}.
fn lesson() -> Arc<CardDefinition> {
    let spell: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"GainLife":{"amount":2}}]}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Lesson")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Keyword(Keyword::Paradigm))
            .with_rules(spell),
    )
}

/// Answers until turn `turn` reaches its postcombat main phase: every "may" is taken, every other question answered
/// passively.
fn take_casts_until(driver: &mut Driver, turn: u32) -> Result<(), ScriptError> {
    for _ in 0..2_000 {
        let state = driver.state();
        if state.turn().number() == turn && state.turn().step() == Step::Main2 {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck(format!("turn {turn}")))
}

/// How many cards named "Lesson" lie in exile.
fn exiled_lessons(driver: &Driver) -> usize {
    names(driver.state(), Zone::Exile)
        .iter()
        .filter(|name| *name == "Lesson")
        .count()
}

#[rstest]
fn paradigm_exiles_the_spell_and_casts_one_copy_each_first_main_phase(
    #[values(2, 3)] seats: usize,
    #[values(1, 2)] lessons: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![lesson(); lessons]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..lessons {
        driver.cast("Lesson")?;
        for _ in 0..seats {
            driver.pass()?;
        }
    }
    let gained = 2 * i32::try_from(lessons).unwrap();
    let life = |driver: &Driver| driver.state().player(PlayerId(0)).unwrap().life();
    assert_eq!(life(&driver), 20 + gained);
    assert_eq!(exiled_lessons(&driver), lessons);
    let seats_u32 = u32::try_from(seats).unwrap();
    for round in 1..=2 {
        take_casts_until(&mut driver, 1 + round * seats_u32)?;
        assert_eq!(
            life(&driver),
            20 + gained + 2 * i32::try_from(round).unwrap(),
            "one copy at the first main phase of each of the caster's turns"
        );
    }
    assert_eq!(exiled_lessons(&driver), lessons);
    Ok(())
}
