//! Goal: "{1}, Exile this card from your hand: Target land gains an ability until this card is cast from exile. You may
//! cast this card for as long as it remains exiled." exiles the card, grants the ability, lets its owner cast the card
//! from exile and ends the grant once it is cast; a card that leaves exile another way leaves the grant for good and is
//! no longer castable, and a count "as you cast this spell" is the count of the cast, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The hand ability: {1} and the card itself, the target land gains "{T}: You gain 1 life".
const HIDE: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"ExileSelf"],"from":"Hand","targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilCastFromExile","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}}]}},{"Rule":{"rule":{"MayCast":{"only":{"Arrived":{"of":"This","zone":"Exile"}},"from":"Exile"}},"duration":"Forever"}}]}}"#;

fn hideaway() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Hideaway")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(serde_json::from_str::<Ability>(HIDE).unwrap()),
    )
}

/// A land with no ability of its own.
fn field() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Field").with_type(CardType::Land))
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn find(driver: &Driver, zone: Zone, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
}

/// Whether the land has an ability seat 0 may activate now.
fn granted(driver: &Driver) -> bool {
    !driver.offered_abilities("Field").unwrap().is_empty()
}

fn castable(driver: &Driver, id: ObjectId) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&id),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Seat 0 hides the card from its hand at a table of `seats`, targeting the Field.
fn hidden(seats: usize) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(forest()), ready(forest()), ready(field())])
        .hand(0, [hideaway()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Hideaway")?;
    let land = driver.target_named("Field")?;
    driver.choose_targets(&[land])?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(&mut driver)?;
    Ok(driver)
}

#[rstest]
fn the_land_keeps_the_ability_until_the_card_is_cast_from_exile(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = hidden(seats)?;
    let exiled = find(&driver, Zone::Exile, "Hideaway").expect("the card is exiled");
    assert!(granted(&driver), "the land gained the ability");
    assert!(castable(&driver, exiled), "the exiled card may be cast");
    driver.cast("Hideaway")?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    assert!(!driver.state().stack().is_empty(), "the card is a spell");
    assert!(!granted(&driver), "the cast ends the grant");
    settle(&mut driver)?;
    assert!(find(&driver, Zone::Battlefield, "Hideaway").is_some());
    assert!(!granted(&driver), "the grant does not come back");
    Ok(())
}

#[rstest]
fn a_card_that_leaves_exile_another_way_leaves_the_grant_for_good(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = hidden(seats)?;
    let exiled = find(&driver, Zone::Exile, "Hideaway").expect("the card is exiled");
    driver.manual(ManualAction::Move {
        object: exiled,
        to: ManualZone::Graveyard,
    })?;
    let card = find(&driver, Zone::Graveyard(pid(0)), "Hideaway").expect("in the graveyard");
    driver.manual(ManualAction::Move {
        object: card,
        to: ManualZone::Exile,
    })?;
    let again = find(&driver, Zone::Exile, "Hideaway").expect("exiled again");
    assert!(
        !castable(&driver, again),
        "a new object in exile is not covered"
    );
    assert!(granted(&driver), "the land keeps the ability");
    Ok(())
}

/// An instant for {1}: destroy each creature, then gain 1 life for each creature you controlled as you cast it, or,
/// when `live`, for each you control now.
fn reckoning(live: bool) -> Arc<CardDefinition> {
    let count = r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;
    let amount = if live {
        count.to_owned()
    } else {
        format!(r#"{{"CountedAsCast":{count}}}"#)
    };
    let spell = format!(
        r#"{{"Spell":{{"effects":[{{"Destroy":{{"what":{{"Each":[{{"Type":"Creature"}}]}}}}}},{{"GainLife":{{"amount":{amount}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Reckoning")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(serde_json::from_str::<Ability>(&spell).unwrap()),
    )
}

fn bear(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2),
    )
}

#[rstest]
#[case::as_cast(false, 3, 23)]
#[case::now(true, 3, 20)]
#[case::none_as_cast(false, 0, 20)]
fn a_count_as_you_cast_this_spell_is_the_count_of_the_cast(
    #[values(2, 3, 4)] seats: usize,
    #[case] live: bool,
    #[case] bears: usize,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(forest())];
    mine.extend((0..bears).map(|at| ready(bear(&format!("Bear {at}")))));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, [ready(bear("Foe"))])
        .hand(0, [reckoning(live)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Reckoning")?;
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life);
    Ok(())
}
