//! Goal: a word chosen as a permanent enters (a Siege's "choose Khans or Dragons", `ReplaceBy::ChooseWord`) is asked of its controller at any table size, kept on the permanent, and only the abilities under that word (`Condition::EnteredAs`) work: a trigger of the other word does nothing and a static of the other word changes nothing; a choice nobody answers takes the first word, and the game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_testkit::cards::{cost, creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn rules(json: &str) -> mtg_cards::ability::Ability {
    serde_json::from_str(json).unwrap()
}

/// A free Siege: Khans gains 3 life as it enters, Dragons gains 5 and gives the creatures of its controller +1/+0.
fn siege() -> Arc<CardDefinition> {
    let gain = |word: &str, amount: i32| {
        format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"if":{{"EnteredAs":"{word}"}},"effects":[{{"GainLife":{{"amount":{amount}}}}}]}}}}"#
        )
    };
    Arc::new(
        CardDefinition::new("Siege")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(
                r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"ChooseWord":["Khans","Dragons"]}}}"#,
            ))
            .with_rules(rules(&gain("Khans", 3)))
            .with_rules(rules(&gain("Dragons", 5)))
            .with_rules(rules(
                r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"condition":{"EnteredAs":"Dragons"},"mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}"#,
            )),
    )
}

/// Plays on until the stack is empty, answering each choice of option from the queue (or passively when the queue is
/// empty) and everything else passively; how many options were asked.
fn play_out(driver: &mut Driver, answers: &mut VecDeque<Answer>) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseOption(_) => {
                asked += 1;
                match answers.pop_front() {
                    Some(answer) => driver.answer(answer)?,
                    None => driver.answer_passively()?,
                };
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::khans(Some(0), "Khans", 3, 2)]
#[case::dragons(Some(1), "Dragons", 5, 3)]
#[case::nobody_answers(None, "Khans", 3, 2)]
fn e15f_only_the_chosen_word_works(
    #[values(2, 3, 4)] seats: usize,
    #[case] answer: Option<usize>,
    #[case] word: &str,
    #[case] gained: i32,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [siege()])
            .battlefield(0, [ready(creature("Bear", 0, &[Color::Green], 2, 2))]),
    )?;
    let base = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Siege")?;
    let mut answers: VecDeque<Answer> = answer
        .map(|at| Answer::ChooseOption(OptionChoice(at)))
        .into_iter()
        .collect();
    let asked = play_out(&mut driver, &mut answers)?;
    assert_eq!(asked, 1);
    assert_eq!(driver.state().player(pid(0)).unwrap().life() - base, gained);
    let id = driver.id_of("Siege")?;
    let kept = driver.state().object(id).unwrap().permanent().unwrap();
    assert_eq!(kept.chosen(), Some(&Chosen::Word(word.to_owned())));
    let bear = characteristics(driver.state(), driver.id_of("Bear")?).unwrap();
    assert_eq!(bear.power, Some(power));
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
