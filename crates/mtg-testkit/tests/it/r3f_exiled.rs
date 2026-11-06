//! Goal: the cost part "put cards from exile into their owners' graveyards" is offered for an ability and for a spell's required additional cost exactly when enough exiled cards match its filter, takes the card of any owner the filter lets through, asks only where there is a choice, and puts each card into its owner's graveyard.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::GameState;
use mtg_engine::choose::CardsChoice;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

fn object(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|found| found.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn free() -> ManaCost {
    cost(0, &[])
}

/// An artifact with "<cost>: You gain 1 life."
fn gadget(cost: &str) -> Arc<CardDefinition> {
    let json =
        format!(r#"{{"Activated":{{"cost":[{cost}],"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(rules(&json)),
    )
}

/// A free sorcery whose additional cost is required: "As an additional cost, <cost>. You gain 1 life."
fn rite(cost: &str) -> Arc<CardDefinition> {
    let extra = format!(r#"{{"AdditionalCost":{{"cost":[{cost}],"required":true}}}}"#);
    Arc::new(
        CardDefinition::new("Rite")
            .with_type(CardType::Sorcery)
            .with_mana_cost(free())
            .with_rules(rules(&extra))
            .with_rules(rules(
                r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
            )),
    )
}

/// A free instant: "Exile target card from a graveyard."
fn banish() -> Arc<CardDefinition> {
    let json = r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#;
    Arc::new(
        CardDefinition::new("Banish")
            .with_type(CardType::Instant)
            .with_mana_cost(free())
            .with_rules(rules(json)),
    )
}

/// A game where the cards of seat 0 (`mine`) and seat 1 (`theirs`) have been exiled from their graveyards.
fn game(cost: &str, as_spell: bool, mine: &[&str], theirs: &[&str]) -> Result<Driver, ScriptError> {
    let card = |name: &&str| free_creature(name, 1, 1);
    let mut banishes: Vec<_> = (0..mine.len() + theirs.len()).map(|_| banish()).collect();
    let mut scenario = Scenario::new(2)
        .graveyard(0, mine.iter().map(card))
        .graveyard(1, theirs.iter().map(card));
    if as_spell {
        banishes.push(rite(cost));
    } else {
        scenario = scenario.battlefield(0, [ready(gadget(cost))]);
    }
    scenario = scenario.hand(0, banishes);
    let mut driver = Driver::from_scenario(scenario)?;
    for name in mine.iter().chain(theirs) {
        let id = object(driver.state(), name);
        let target = Target::Object(driver.state().object(id).unwrap().reference());
        driver.cast_at("Banish", &[target])?;
        settle(&mut driver)?;
    }
    Ok(driver)
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether the Gadget's ability or the Rite spell is offered.
fn offered(driver: &Driver, as_spell: bool) -> bool {
    if as_spell {
        let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
            return false;
        };
        options
            .castable
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Rite")
    } else {
        !driver.offered_abilities("Gadget").unwrap().is_empty()
    }
}

/// Pays the cost, answering the one question about cards with `picks` when it is asked.
fn pay(driver: &mut Driver, as_spell: bool, picks: &[&str]) -> Result<(), ScriptError> {
    if as_spell {
        driver.begin_cast("Rite")?;
    } else {
        driver.begin_activation("Gadget")?;
    }
    let mut asked = false;
    for _ in 0..50 {
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            assert!(options.from.len() > picks.len(), "asked without a choice");
            let ids: Vec<ObjectId> = picks
                .iter()
                .map(|name| object(driver.state(), name))
                .collect();
            asked = true;
            driver.answer(Answer::ChooseCards(CardsChoice(ids)))?;
        } else if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
        {
            break;
        } else if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert!(asked || picks.len() == 1, "a choice was expected");
    Ok(())
}

#[rstest]
#[case::enough_of_theirs(r#"{"OwnedBy":"Opponents"}"#, 1, &[], &["Theirs"], true)]
#[case::only_mine_does_not_match(r#"{"OwnedBy":"Opponents"}"#, 1, &["Mine"], &[], false)]
#[case::two_needed_one_there(r#"{"OwnedBy":"Opponents"}"#, 2, &["Mine"], &["Theirs"], false)]
#[case::two_needed_two_there(r#"{"OwnedBy":"Opponents"}"#, 2, &[], &["Theirs", "Their Elk"], true)]
#[case::any_owner(r#"{"Type":"Creature"}"#, 2, &["Mine"], &["Theirs"], true)]
#[case::nothing_exiled(r#"{"OwnedBy":"Opponents"}"#, 1, &[], &[], false)]
fn the_part_is_offered_exactly_when_enough_exiled_cards_match(
    #[case] filter: &str,
    #[case] count: u32,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] payable: bool,
    #[values(false, true)] as_spell: bool,
) -> Result<(), ScriptError> {
    let part = format!(r#"{{"ExiledToGraveyard":{{"filter":[{filter}],"count":{count}}}}}"#);
    let driver = game(&part, as_spell, mine, theirs)?;
    assert_eq!(offered(&driver, as_spell), payable);
    Ok(())
}

#[rstest]
#[case::one_card_unasked(&["Theirs"], &["Theirs"])]
#[case::a_choice_of_two(&["Theirs", "Their Elk"], &["Their Elk"])]
fn each_card_goes_to_its_owners_graveyard(
    #[case] theirs: &[&str],
    #[case] chosen: &[&str],
    #[values(false, true)] as_spell: bool,
) -> Result<(), ScriptError> {
    let part = r#"{"ExiledToGraveyard":{"filter":[{"OwnedBy":"Opponents"}]}}"#;
    let mut driver = game(part, as_spell, &["Mine"], theirs)?;
    assert!(offered(&driver, as_spell));
    pay(&mut driver, as_spell, chosen)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), chosen);
    assert!(names(driver.state(), Zone::Exile).contains(&"Mine".to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Exile).len(),
        theirs.len() - chosen.len() + 1
    );
    Ok(())
}
