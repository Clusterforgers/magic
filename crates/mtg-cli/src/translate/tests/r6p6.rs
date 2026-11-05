//! Goal: the parameters that restrict a target (the controller the targets share) or shape a zone change read once for every verb; each either means what the card prints or stays a gap named after the key.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

fn permanent(script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject").with_type(CardType::Artifact);
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn line(body: &str) -> String {
    format!("Name:T\nTypes:Sorcery\n{body}\n")
}

/// "Whenever a creature dies, return target card from its controller's graveyard to its owner's hand."
const DIES: &str = "Name:Subject\nTypes:Artifact\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ E\nSVar:E:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Card | TargetsWithDefinedController$ TriggeredCardController\n";

/// Kills `Theirs` (seat 1) and returns the driver at the trigger's target choice.
fn killed() -> Driver {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)])
        .graveyard(0, [free_creature("Mine In Yard", 1, 1)])
        .graveyard(1, [free_creature("Theirs In Yard", 1, 1)])
        .battlefield(0, [ready(permanent(DIES)), ready(mountain())])
        .battlefield(1, [ready(free_creature("Theirs", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Theirs").unwrap();
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    for _ in 0..40 {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseTargets(_)
        ) {
            return driver;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the trigger never asked for a target");
}

#[test]
fn the_card_of_the_controller_in_the_graveyard_is_a_legal_target() {
    let mut driver = killed();
    let theirs = find(&driver, 1, "Theirs In Yard");
    assert!(driver.choose_targets(&[theirs]).is_ok());
}

#[test]
fn a_card_of_another_player_is_no_legal_target() {
    let mut driver = killed();
    let mine = find(&driver, 0, "Mine In Yard");
    assert!(driver.choose_targets(&[mine]).is_err());
}

fn find(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let object = state
        .zone(mtg_engine::zones::Zone::Graveyard(
            mtg_core::ids::PlayerId::from_index(seat),
        ))
        .iter()
        .copied()
        .filter_map(|id| state.object(id))
        .find(|object| object.card().name == name)
        .unwrap();
    Target::Object(object.reference())
}

#[rstest]
#[case::trigger_word(
    "T:Mode$ DamageDone | ValidSource$ Creature | ValidTarget$ Player | Execute$ E | TriggerZones$ Battlefield\nSVar:E:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | TargetsWithDefinedController$ TriggeredTarget",
    &[]
)]
#[case::unknown_word(
    "A:SP$ GainLife | ValidTgts$ Player | LifeAmount$ 1 | SubAbility$ B\nSVar:B:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | TargetsWithDefinedController$ Foo",
    &["key:TargetsWithDefinedController=Foo"]
)]
#[case::parent_target_is_engine_work(
    "A:SP$ GainLife | ValidTgts$ Player | LifeAmount$ 1 | SubAbility$ B\nSVar:B:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | TargetsWithDefinedController$ ParentTarget",
    &["key:TargetsWithDefinedController=ParentTarget"]
)]
fn controller_restriction_words(#[case] body: &str, #[case] expected: &[&str]) {
    assert_eq!(gaps(&line(body)), expected);
}
