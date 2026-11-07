//! Goal: a bind that adds keeps the picks of every pass of a repeat for each player under one name ("each player chooses a creature; destroy the rest"), where a plain bind holds only the last pass's pick, and a rule that reads a card name chosen in the resolution keeps that name after the resolution is over ("spells with the chosen name can't be cast this turn"), at tables of two, three and four seats, with the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, NameChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free sorcery with these effects.
fn sorcery(name: &str, effects: &str) -> Arc<CardDefinition> {
    let json = format!(r#"{{"Spell":{{"effects":{effects}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

/// Plays on until the stack is empty, taking the first cards offered, naming `name` and passing otherwise.
fn play_out(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseCards(options) => {
                let picked = options.from[..options.min as usize].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            DecisionKind::ChooseName(_) => {
                driver.answer(Answer::ChooseName(NameChoice(name.to_owned())))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn creatures_left(driver: &Driver) -> usize {
    driver
        .state()
        .objects()
        .filter(|object| object.zone() == Zone::Battlefield)
        .count()
}

/// Each player picks one of their two creatures; everything not picked is destroyed.
const KEEP_ONE: &str = r#"[{"RepeatEach":{"over":"EachPlayer","as":"each","effects":[
    {"Choose":{"who":{"Bound":"each"},"from":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Bound":"each"}}]},"as":"pick"}},
    {"Bind":{"what":{"Bound":"pick"},"as":"picks"ADDS}}]}},
    {"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Bound":"picks"}}}]}}}]"#;

#[rstest]
#[case::adding_keeps_every_pick(r#","adds":true"#, true)]
#[case::a_plain_bind_keeps_the_last(r"", false)]
fn picks_gathered_over_a_repeat_survive_together(
    #[values(2, 3, 4)] seats: usize,
    #[case] adds: &str,
    #[case] every: bool,
) -> Result<(), ScriptError> {
    let mut scenario =
        Scenario::new(seats).hand(0, [sorcery("Keep", &KEEP_ONE.replace("ADDS", adds))]);
    for seat in 0..seats {
        scenario = scenario.battlefield(
            seat,
            [free_creature("Bear", 2, 2), free_creature("Elf", 1, 1)]
                .into_iter()
                .map(ready),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Keep")?;
    play_out(&mut driver, "")?;
    assert_eq!(creatures_left(&driver), if every { seats } else { 1 });
    replays(&driver);
    Ok(())
}

/// Names a card; spells with that name can't be cast this turn.
const BAN: &str = r#"[{"ChooseValue":{"kind":{"CardName":[]},"as":"name"}},
    {"Rule":{"rule":{"Cant":{"who":"EachPlayer","action":{"Cast":{"filter":[{"ChosenAs":"name"}]}}}},"duration":"UntilEndOfTurn"}}]"#;

#[rstest]
#[case::the_named_spell_is_refused("Bolt", true)]
#[case::another_name_leaves_it_free("Other", false)]
fn a_name_chosen_in_the_resolution_stays_in_the_rule(
    #[values(2, 3, 4)] seats: usize,
    #[case] named: &str,
    #[case] refused: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).hand(0, [sorcery("Ban", BAN), sorcery("Bolt", "[]")]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Ban")?;
    play_out(&mut driver, named)?;
    assert_eq!(driver.cast("Bolt").is_err(), refused);
    replays(&driver);
    Ok(())
}
