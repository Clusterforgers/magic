//! Goal: a text change replaces every instance of one color word, basic land type or creature type of a permanent with another of its kind, chosen as it resolves, so a protection, a landwalk, a static ability's selector and the type line read the new word; it lasts as long as it says (for good, or until end of turn) and changes no other permanent, at tables of two, three and four seats, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Subtype};
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A 1/1 creature for {0} of this subtype with these rules.
fn creature(name: &str, subtype: &str, json: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_subtype(subtype)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(1, 1);
    if let Some(json) = json {
        card = card.with_rules(rules(json));
    }
    Arc::new(card)
}

/// An instant for {0} that changes the text of target permanent: the word of `from` replaced by one of `to`.
fn changer(name: &str, from: &str, to: &str, duration: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules(&format!(
                r#"{{"Spell":{{"targets":[{{"what":{{"Object":[]}}}}],"effects":[
                {{"ChooseValue":{{"kind":{from},"as":"from"}}}},
                {{"ChooseValue":{{"kind":{to},"as":"to"}}}},
                {{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"ChangeTextChosen":{{"from":"from","to":"to"}}}}],"duration":"{duration}"}}}}]}}}}"#
            ))),
    )
}

/// Seat 0 holds the changers and controls the permanents; every other seat controls a black Goblin.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [
                changer("Sleight", r#""Color""#, r#"{"LikeChosen":"from"}"#, "Forever"),
                changer(
                    "Spray",
                    r#""ColorOrBasicLandType""#,
                    r#"{"LikeChosen":"from"}"#,
                    "UntilEndOfTurn",
                ),
                changer("Hack", r#""BasicLandType""#, r#"{"LikeChosen":"from"}"#, "Forever"),
                changer(
                    "Evolution",
                    r#""CreatureType""#,
                    r#"{"CreatureTypeExcept":["Wall"]}"#,
                    "Forever",
                ),
            ],
        )
        .battlefield(
            0,
            [
                ready(creature(
                    "Knight",
                    "Knight",
                    Some(r#"{"Keyword":{"Protection":[{"Color":"Black"}]}}"#),
                )),
                ready(creature(
                    "Walker",
                    "Zombie",
                    Some(r#"{"Keyword":{"Landwalk":[{"Subtype":"Swamp"}]}}"#),
                )),
                ready(creature(
                    "Lord",
                    "Human",
                    Some(
                        r#"{"Static":{"affected":{"Each":[{"Subtype":"Goblin"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#,
                    ),
                )),
                ready(creature("Gob", "Goblin", None)),
                ready(creature("Elfie", "Elf", None)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(creature(&format!("Foe {seat}"), "Goblin", None))]);
    }
    scenario
}

/// Answers the word questions with `words` in order, and passes until the stack is empty.
fn resolve(driver: &mut Driver, words: &[&str]) -> Result<(), ScriptError> {
    let mut words = words.iter();
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            let word = words.next().expect("a word for every question");
            let at = options
                .options
                .iter()
                .position(|option| match option {
                    ChoiceOption::Color(color) => format!("{color:?}") == *word,
                    ChoiceOption::Label(label) => label == word,
                    _ => false,
                })
                .expect("the word is offered");
            driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn change(driver: &mut Driver, spell: &str, on: &str, words: &[&str]) -> Result<(), ScriptError> {
    let target = driver.target_named(on)?;
    driver.cast_at(spell, &[target])?;
    resolve(driver, words)
}

fn keywords(driver: &Driver, name: &str) -> String {
    let id = driver.id_of(name).unwrap();
    serde_json::to_string(&keywords_of(driver.state(), id)).unwrap()
}

fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::a_color_word("Sleight", "Knight", &["Black", "Green"], "Knight", r#"[{"Protection":[{"Color":"Green"}]}]"#)]
#[case::a_land_type("Hack", "Walker", &["Swamp", "Island"], "Walker", r#"[{"Landwalk":[{"Subtype":"Island"}]}]"#)]
#[case::a_land_type_by_the_either_kind("Spray", "Walker", &["Swamp", "Forest"], "Walker", r#"[{"Landwalk":[{"Subtype":"Forest"}]}]"#)]
#[case::a_word_the_text_lacks("Sleight", "Knight", &["Red", "Blue"], "Knight", r#"[{"Protection":[{"Color":"Black"}]}]"#)]
fn a_keyword_reads_the_new_word(
    #[case] spell: &str,
    #[case] on: &str,
    #[case] words: &[&str],
    #[case] checked: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    change(&mut driver, spell, on, words)?;
    assert_eq!(keywords(&driver, checked), expected);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::the_static_ability_reads_elves("Lord", (1, 1), (2, 2))]
#[case::the_type_line_turns_the_goblin_into_an_elf("Gob", (1, 1), (1, 1))]
fn a_creature_type_changes_in_the_rules_and_on_the_type_line(
    #[case] on: &str,
    #[case] gob: (i32, i32),
    #[case] elfie: (i32, i32),
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    assert_eq!(size(&driver, "Gob"), (2, 2));
    change(&mut driver, "Evolution", on, &["Goblin", "Elf"])?;
    assert_eq!(size(&driver, "Gob"), gob);
    assert_eq!(size(&driver, "Elfie"), elfie);
    let gob_types = characteristics(driver.state(), driver.id_of("Gob")?).unwrap().subtypes;
    assert_eq!(gob_types.contains(&Subtype("Elf".into())), on == "Gob");
    // The other seats' Goblins follow the lord's text, not their own.
    for seat in 1..seats {
        let expected = if on == "Lord" { (1, 1) } else { (2, 2) };
        assert_eq!(size(&driver, &format!("Foe {seat}")), expected);
    }
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_change_until_end_of_turn_ends_with_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    change(&mut driver, "Spray", "Knight", &["Black", "White"])?;
    assert_eq!(keywords(&driver, "Knight"), r#"[{"Protection":[{"Color":"White"}]}]"#);
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(keywords(&driver, "Knight"), r#"[{"Protection":[{"Color":"Black"}]}]"#);
    replays(&driver);
    Ok(())
}
