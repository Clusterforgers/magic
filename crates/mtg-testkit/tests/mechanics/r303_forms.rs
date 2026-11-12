//! Goal: a permanent that mills as it enters (`ReplaceBy::Mill`) mills that many cards of its controller's library for each kick at any table size and still enters; a permanent that becomes its controller's choice of forms as it enters (Primal Clay, `ReplaceBy::BecomesChoice`) is asked once at any table size, takes the chosen form's power, toughness, subtypes and keywords and keeps its other types, a choice nobody answers takes the first form, a permanent that becomes a copy of it later is that form (CR 707.2), and the game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, CostsChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The corpus card with no mana cost, so it is cast without lands.
fn free(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        card(name)
            .unwrap()
            .as_ref()
            .clone()
            .with_mana_cost(cost(0, &[])),
    )
}

/// Plays on until the stack is empty: each choice of option takes the next answer of the queue (or the passive one when
/// it is empty), a choice of cards takes `pick`, everything else is answered passively; how many options were asked.
fn play_out(
    driver: &mut Driver,
    answers: &mut VecDeque<Answer>,
    pick: Option<&str>,
) -> Result<usize, ScriptError> {
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
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(_) if pick.is_some() => {
                let id = driver.id_of(pick.unwrap_or_default())?;
                driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// What a form is: power, toughness, flying, defender and Wall.
type Form = (i32, i32, bool, bool, bool);

/// Whether the permanent named has the form, still as an artifact creature.
fn assert_form(driver: &Driver, name: &str, (power, toughness, flying, defender, wall): Form) {
    let id = driver.id_of(name).unwrap();
    let seen = characteristics(driver.state(), id).unwrap();
    assert_eq!(
        (seen.power, seen.toughness),
        (Some(power), Some(toughness)),
        "{name}"
    );
    assert_eq!(
        has_keyword(driver.state(), id, &Keyword::Flying),
        flying,
        "{name}"
    );
    assert_eq!(
        has_keyword(driver.state(), id, &Keyword::Defender),
        defender,
        "{name}"
    );
    let printed = &driver.state().object(id).unwrap().card().subtypes;
    assert_eq!(
        seen.has_subtype(printed, &Subtype("Wall".into())),
        wall,
        "{name}"
    );
    assert!(
        seen.has_subtype(printed, &Subtype("Shapeshifter".into())),
        "{name} keeps its types"
    );
    assert!(seen.types.contains(CardType::Creature), "{name}");
}

#[rstest]
#[case::three_three(Some(0), (3, 3, false, false, false))]
#[case::flier(Some(1), (2, 2, true, false, false))]
#[case::wall(Some(2), (1, 6, false, true, true))]
#[case::nobody_answers(None, (3, 3, false, false, false))]
fn r303_the_chosen_form_is_what_it_becomes(
    #[values(2, 3, 4)] seats: usize,
    #[case] answer: Option<usize>,
    #[case] form: Form,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(0, [free("Primal Clay")]))?;
    driver.cast("Primal Clay")?;
    let mut answers: VecDeque<Answer> = answer
        .map(|at| Answer::ChooseOption(OptionChoice(at)))
        .into_iter()
        .collect();
    assert_eq!(play_out(&mut driver, &mut answers, None)?, 1);
    assert_form(&driver, "Primal Clay", form);
    let id = driver.id_of("Primal Clay")?;
    assert!(
        characteristics(driver.state(), id)
            .unwrap()
            .types
            .contains(CardType::Artifact)
    );
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r303_a_copy_of_it_is_the_chosen_form(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [free("Primal Clay"), free("Cytoshape")])
            .battlefield(0, [ready(creature("Bear", 0, &[Color::Green], 2, 2))]),
    )?;
    driver.cast("Primal Clay")?;
    let mut wall: VecDeque<Answer> = VecDeque::from([Answer::ChooseOption(OptionChoice(2))]);
    play_out(&mut driver, &mut wall, None)?;
    let copy = driver.id_of("Bear")?;
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Cytoshape", &[bear])?;
    let asked = play_out(&mut driver, &mut VecDeque::new(), Some("Primal Clay"))?;
    assert_eq!(
        asked, 0,
        "becoming a copy is no entering: nothing is chosen"
    );
    let seen = characteristics(driver.state(), copy).unwrap();
    assert_eq!((seen.power, seen.toughness), (Some(1), Some(6)));
    assert!(has_keyword(driver.state(), copy, &Keyword::Defender));
    Ok(())
}

/// A free 0/1 that mills three cards for each time it was kicked as it enters, with two free kickers.
fn miller() -> Arc<CardDefinition> {
    let rule = |json: &str| -> mtg_cards::ability::Ability { serde_json::from_str(json).unwrap() };
    Arc::new(
        CardDefinition::new("Miller")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rule(r#"{"Keyword":{"Kicker":"{0}"}}"#))
            .with_rules(rule(r#"{"Keyword":{"Kicker":"{0}"}}"#))
            .with_rules(rule(
                r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Mill":{"Times":{"by":3,"of":"Kicks"}}}}}"#,
            ))
            .with_power_toughness(0, 1),
    )
}

#[rstest]
#[case::not_kicked(&[], 0)]
#[case::kicked_once(&[0], 3)]
#[case::kicked_twice(&[0, 1], 6)]
fn r303_it_mills_as_it_enters_for_each_kick(
    #[values(2, 3, 4)] seats: usize,
    #[case] kicks: &[usize],
    #[case] milled: usize,
) -> Result<(), ScriptError> {
    let library: Vec<Arc<CardDefinition>> = (0..8)
        .map(|at| creature(&format!("Card {at}"), 0, &[Color::Green], 1, 1))
        .collect();
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).hand(0, [miller()]).library(0, library))?;
    driver.begin_cast("Miller")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(kicks.to_vec())))?;
    play_out(&mut driver, &mut VecDeque::new(), None)?;
    let me = mtg_core::ids::PlayerId(0);
    assert_eq!(driver.state().zone(Zone::Graveyard(me)).len(), milled);
    assert_eq!(driver.state().zone(Zone::Library(me)).len(), 8 - milled);
    assert_eq!(
        driver
            .id_of("Miller")
            .map(|id| driver.state().object(id).unwrap().zone()),
        Ok(Zone::Battlefield)
    );
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
