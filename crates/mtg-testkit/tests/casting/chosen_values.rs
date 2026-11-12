//! Goal: a value chosen while a spell resolves (a number, a color, a card type, a card name) is asked of its controller with the options of its kind, remembered under its name for the rest of the resolution and read by the effects after it, and a number or a card name chosen as a permanent enters is asked in the same way, remembered on the permanent and read by its abilities, at tables of two, three and four seats, with the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{NameChoice, NumberChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, creature, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn rules(json: &str) -> mtg_cards::ability::Ability {
    serde_json::from_str(json).unwrap()
}

/// A free sorcery with this spell ability.
fn sorcery(effects: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Choice")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(&format!(r#"{{"Spell":{{"effects":{effects}}}}}"#))),
    )
}

/// A free artifact that chooses as it enters and then gains life as these amounts say.
fn totem(choice: &str, gain: &str) -> Arc<CardDefinition> {
    let enters = format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"Choose":{choice}}}}}}}"#
    );
    let trigger = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"GainLife":{{"amount":{gain}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Totem")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(&enters))
            .with_rules(rules(&trigger)),
    )
}

/// Plays on until the stack is empty, answering each choice of option or number from the queue and everything else
/// passively; the answers that were not needed are left in the queue.
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
            DecisionKind::ChooseOption(_)
            | DecisionKind::ChooseNumber(_)
            | DecisionKind::ChooseName(_) => {
                asked += 1;
                let answer = answers.pop_front().expect("an answer for the question");
                driver.answer(answer)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn number(n: u32) -> Answer {
    Answer::ChooseNumber(NumberChoice::Number(n))
}

fn named(name: &str) -> Answer {
    Answer::ChooseName(NameChoice(name.to_owned()))
}

fn option(at: usize) -> Answer {
    Answer::ChooseOption(OptionChoice(at))
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn table() -> Vec<Arc<CardDefinition>> {
    vec![
        creature("Bear", 0, &[Color::Red], 2, 2),
        creature("Elf", 0, &[Color::Green], 1, 1),
        creature("Elf", 0, &[Color::Green], 1, 1),
        creature("Giant", 0, &[Color::Red], 4, 4),
        forest(),
    ]
}

const GAIN_MATCHING: &str = r#"{"GainLife":{"amount":{"Count":[{"ChosenAs":"it"}]}}}"#;

/// Colors are offered white, blue, black, red and green; card types alphabetically; names sorted.
#[rstest]
#[case::a_number(
    r#"[{"ChooseValue":{"kind":{"Number":[1,6]},"as":"it"}},{"GainLife":{"amount":{"ChosenAs":"it"}}}]"#,
    number(4),
    4
)]
#[case::the_lowest_number(
    r#"[{"ChooseValue":{"kind":{"Number":[1,6]},"as":"it"}},{"GainLife":{"amount":{"ChosenAs":"it"}}}]"#,
    number(1),
    1
)]
#[case::a_number_bounded_by_an_amount(
    r#"[{"ChooseValue":{"kind":{"Number":[0,{"Count":[{"Type":"Creature"}]}]},"as":"it"}},{"GainLife":{"amount":{"ChosenAs":"it"}}}]"#,
    number(4),
    4
)]
#[case::red_creatures(
    &format!(r#"[{{"ChooseValue":{{"kind":"Color","as":"it"}}}},{GAIN_MATCHING}]"#),
    option(3),
    2
)]
#[case::green_creatures_and_lands_alike(
    &format!(r#"[{{"ChooseValue":{{"kind":"Color","as":"it"}}}},{GAIN_MATCHING}]"#),
    option(4),
    2
)]
#[case::creature_cards(
    &format!(r#"[{{"ChooseValue":{{"kind":"CardType","as":"it"}}}},{GAIN_MATCHING}]"#),
    option(2),
    4
)]
#[case::lands(
    &format!(r#"[{{"ChooseValue":{{"kind":"CardType","as":"it"}}}},{GAIN_MATCHING}]"#),
    option(6),
    1
)]
#[case::a_card_name(
    &format!(r#"[{{"ChooseValue":{{"kind":{{"CardName":[{{"Type":"Creature"}}]}},"as":"it"}}}},{GAIN_MATCHING}]"#),
    named("Elf"),
    2
)]
#[case::a_name_in_play(
    &format!(r#"[{{"ChooseValue":{{"kind":{{"CardName":[{{"Type":"Creature"}}]}},"as":"it"}}}},{GAIN_MATCHING}]"#),
    named("Bear"),
    1
)]
#[case::a_name_no_card_in_the_game_has(
    &format!(r#"[{{"ChooseValue":{{"kind":{{"CardName":[]}},"as":"it"}}}},{GAIN_MATCHING}]"#),
    named("Black Lotus"),
    0
)]
#[case::naming_nothing(
    &format!(r#"[{{"ChooseValue":{{"kind":{{"CardName":[]}},"as":"it"}}}},{GAIN_MATCHING}]"#),
    named(""),
    0
)]
fn a_value_chosen_in_a_resolution_is_read_by_the_effects_after_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &str,
    #[case] answer: Answer,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [sorcery(effects)])
            .battlefield(0, table().into_iter().map(ready)),
    )?;
    let base = life(&driver);
    driver.cast("Choice")?;
    let asked = play_out(&mut driver, &mut VecDeque::from([answer]))?;
    assert_eq!(asked, 1);
    assert_eq!(life(&driver) - base, gained);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_number(
    r#"{"Number":{"min":1,"max":4}}"#,
    r#""Chosen""#,
    number(3),
    3,
    Chosen::Number(3)
)]
#[case::the_most(
    r#"{"Number":{"min":0,"max":9}}"#,
    r#""Chosen""#,
    number(9),
    9,
    Chosen::Number(9)
)]
#[case::a_card_name(
    r#""CardName""#,
    r#"{"Count":[{"Type":"Creature"},"ChosenName"]}"#,
    named("Elf"),
    2,
    Chosen::Name("Elf".to_owned())
)]
#[case::a_nonland_card_name(
    r#""NonlandCardName""#,
    r#"{"Count":[{"Type":"Creature"},"ChosenName"]}"#,
    named("Giant"),
    1,
    Chosen::Name("Giant".to_owned())
)]
fn a_value_chosen_as_a_permanent_enters_is_kept_on_it_and_read_by_its_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[case] choice: &str,
    #[case] gain: &str,
    #[case] answer: Answer,
    #[case] gained: i32,
    #[case] kept: Chosen,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [totem(choice, gain)])
            .battlefield(0, table().into_iter().map(ready)),
    )?;
    let base = life(&driver);
    driver.cast("Totem")?;
    let asked = play_out(&mut driver, &mut VecDeque::from([answer]))?;
    assert_eq!(asked, 1);
    assert_eq!(life(&driver) - base, gained);
    let totem = driver.id_of("Totem")?;
    let chosen = driver.state().object(totem).unwrap().permanent().unwrap();
    assert_eq!(chosen.chosen(), Some(&kept));
    replays(&driver);
    Ok(())
}

#[test]
fn a_choice_nobody_answers_takes_the_default_of_the_table() -> Result<(), ScriptError> {
    // The passive answer to a number is its lowest, to an option the first.
    let effects = r#"[{"ChooseValue":{"kind":{"Number":[2,6]},"as":"it"}},{"GainLife":{"amount":{"ChosenAs":"it"}}}]"#;
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [sorcery(effects)]))?;
    let base = life(&driver);
    driver.cast("Choice")?;
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(life(&driver) - base, 2);
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::ChoiceMade { .. }))
    );
    Ok(())
}
