//! Goal: New Blood gains control of the target for good and changes every instance of the creature type its caster chooses to Vampire in that creature's text (its type line and its rules), so a lord of Goblins it takes pumps Vampires of its new controller; the additional cost taps a Vampire; at tables of two, three and four seats, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::cost;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A 1/1 creature of the subtype with these rules.
fn creature(name: &str, subtype: &str, json: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_subtype(subtype)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(1, 1);
    if let Some(json) = json {
        card = card.with_rules(serde_json::from_str::<Ability>(json).unwrap());
    }
    Arc::new(card)
}

const LORD: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Subtype":"Goblin"},{"ControlledBy":"You"},"Other"]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;

/// Answers until the stack is empty: the option labelled `word`, the first card, the target `lord`.
fn settle(driver: &mut Driver, word: &str) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(
                        |option| matches!(option, ChoiceOption::Label(label) if label == word),
                    )
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from.first().copied().into_iter().collect();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

#[rstest]
#[case::the_lords_type("Goblin", true)]
#[case::a_type_it_lacks("Elf", false)]
fn new_blood_makes_the_creature_a_lord_of_vampires(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: &str,
    #[case] changed: bool,
) -> Result<(), ScriptError> {
    let mut new_blood = (*card("New Blood").unwrap()).clone();
    new_blood.mana_cost = Some(cost(0, &[]));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [Arc::new(new_blood)])
        .battlefield(0, [ready(creature("Thirsty", "Vampire", None))]);
    let theirs = seats - 1;
    for seat in 1..seats {
        let lord = if seat == theirs {
            vec![
                ready(creature("Lord", "Goblin", Some(LORD))),
                ready(creature("Grunt", "Goblin", None)),
            ]
        } else {
            vec![ready(creature("Bystander", "Goblin", None))]
        };
        scenario = scenario.battlefield(seat, lord);
    }
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let lord = driver.id_of("Lord")?;
    let thirsty = driver.id_of("Thirsty")?;
    let grunt = driver.id_of("Grunt")?;
    let lord_at = driver.target_named("Lord")?;
    driver.begin_cast("New Blood")?;
    driver.choose_targets(&[lord_at])?;
    settle(&mut driver, chosen)?;
    let state = driver.state();
    assert_eq!(
        mtg_engine::characteristics::controller_of(state, lord),
        Some(PlayerId::from_index(0)),
        "{:?}",
        driver
            .engine()
            .events()
            .iter()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
    );
    assert!(
        state.object(thirsty).unwrap().permanent().unwrap().tapped(),
        "the cost taps a Vampire"
    );
    let lord_now = characteristics(state, lord).unwrap();
    let printed = &state.object(lord).unwrap().card().subtypes;
    let is = |name: &str| lord_now.has_subtype(printed, &Subtype(name.into()));
    assert_eq!(is("Vampire"), changed);
    assert_eq!(is("Goblin"), !changed);
    let pumped = |id| characteristics(state, id).unwrap().power;
    assert_eq!(
        pumped(thirsty),
        Some(1 + i32::from(changed)),
        "a lord of Vampires now"
    );
    assert_eq!(
        pumped(grunt),
        Some(1),
        "the Goblin left behind is pumped no more"
    );
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
