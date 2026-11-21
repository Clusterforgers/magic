//! Goal: an exchange of text boxes (`Modification::R612ExchangeTextBoxes`, CR 612.1, layer 3) at 2 to 4 seats: Exchange
//! of Words gives each of its two targets the rules text of the other, keywords, static and activated abilities, while
//! names stay; a lord's static then works for its new holder's controller; the exchange ends when the enchantment
//! leaves or one of the creatures does; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::cards::cost;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

const LORD: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Subtype":"Goblin"},{"ControlledBy":"You"},"Other"]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const PING: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}"#;

/// A 1/1 creature of the subtype with these rules.
fn creature(name: &str, subtype: &str, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_subtype(subtype)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(1, 1);
    for json in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(json).unwrap());
    }
    Arc::new(card)
}

/// The corpus card with no mana cost.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut card = (*card(name).unwrap()).clone();
    card.mana_cost = Some(cost(0, &[]));
    Arc::new(card)
}

/// Seat 0 has Flyer (flying, "{T}: gain 1 life") and a Goblin, with Exchange of Words, Unravel the Aether and Deadpool in hand,
/// each free; the last seat has a Goblin lord and a Goblin.
fn table(seats: usize) -> Scenario {
    let last = seats - 1;
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [
                "Exchange of Words",
                "Unravel the Aether",
                "Deadpool, Trading Card",
            ]
            .map(free),
        )
        .battlefield(
            0,
            [
                ready(creature(
                    "Flyer",
                    "Bird",
                    &[r#"{"Keyword":"Flying"}"#, PING],
                )),
                ready(creature("Mine", "Goblin", &[])),
            ],
        )
        .battlefield(
            last,
            [
                ready(creature("Lord", "Goblin", &[LORD])),
                ready(creature("Theirs", "Goblin", &[])),
            ],
        );
    for seat in 1..last {
        scenario = scenario.battlefield(seat, [ready(creature("Bystander", "Goblin", &[]))]);
    }
    scenario
}

/// Plays on until the stack is empty, choosing `targets` when asked, `pick` when a card is to be chosen, and answering the
/// rest the default way.
fn settle(
    driver: &mut Driver,
    targets: &[Target],
    pick: Option<ObjectId>,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(targets)?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(_) => {
                driver.answer(Answer::ChooseCards(CardsChoice(pick.into_iter().collect())))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Power, whether it flies, how many activated abilities it has, and its name.
fn shown(driver: &Driver, id: ObjectId) -> (i32, bool, usize, String) {
    let state = driver.state();
    let object = state.object(id).unwrap();
    let chars = characteristics(state, id).unwrap();
    let flies = keywords_of(state, id).contains(&&Keyword::Flying);
    let activated = mtg_engine::ability_reader::abilities_of(state, id)
        .iter()
        .filter(|(_, body)| matches!(**body, Ability::Activated { .. }))
        .count();
    (
        chars.power.unwrap(),
        flies,
        activated,
        chars.name_for(&object.card().name),
    )
}

/// Casts Exchange of Words at Flyer and Lord.
fn exchange(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_cast("Exchange of Words")?;
    let targets = [driver.target_named("Flyer")?, driver.target_named("Lord")?];
    settle(driver, &targets, None)
}

#[rstest]
fn exchange_of_words_swaps_the_rules_text_and_keeps_the_names(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let [flyer, lord, mine, theirs] =
        ["Flyer", "Lord", "Mine", "Theirs"].map(|name| driver.id_of(name).unwrap());
    assert_eq!(shown(&driver, theirs).0, 2, "the lord pumps its own Goblin");
    exchange(&mut driver)?;
    assert_eq!(shown(&driver, flyer), (1, false, 0, "Flyer".to_owned()));
    assert_eq!(shown(&driver, lord), (1, true, 1, "Lord".to_owned()));
    assert_eq!(shown(&driver, mine).0, 2, "Flyer now holds the lord's text");
    assert_eq!(shown(&driver, theirs).0, 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_exchange_ends_when_the_enchantment_leaves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let [flyer, lord, theirs] = ["Flyer", "Lord", "Theirs"].map(|name| driver.id_of(name).unwrap());
    exchange(&mut driver)?;
    let enchantment = driver.target_named("Exchange of Words")?;
    driver.begin_cast("Unravel the Aether")?;
    settle(&mut driver, &[enchantment], None)?;
    assert_eq!(shown(&driver, flyer), (1, true, 1, "Flyer".to_owned()));
    assert!(!shown(&driver, lord).1);
    assert_eq!(shown(&driver, theirs).0, 2);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::with_the_lord(true)]
#[case::declined(false)]
fn deadpool_may_exchange_his_text_box_as_he_enters(
    #[values(2, 3, 4)] seats: usize,
    #[case] exchanged: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let [lord, mine, theirs] = ["Lord", "Mine", "Theirs"].map(|name| driver.id_of(name).unwrap());
    driver.begin_cast("Deadpool, Trading Card")?;
    settle(&mut driver, &[], exchanged.then_some(lord))?;
    let deadpool = driver.id_of("Deadpool, Trading Card")?;
    // The lord's static works for Deadpool's controller, and the lord holds Deadpool's sacrifice ability.
    let (pumped, kept) = if exchanged { (2, 1) } else { (1, 2) };
    assert_eq!(shown(&driver, mine).0, pumped);
    assert_eq!(shown(&driver, theirs).0, kept);
    assert_eq!(shown(&driver, lord).2, usize::from(exchanged));
    assert_eq!(shown(&driver, deadpool).2, usize::from(!exchanged));
    assert_eq!(shown(&driver, deadpool).0, 5, "power stays");
    replays(&driver, &setup);
    Ok(())
}
