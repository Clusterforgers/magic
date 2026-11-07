//! Goal: an ability an Equipment gives the creature it equips with "Unattach this Equipment" in its cost ("{T}, Unattach Heartseeker: Destroy target creature", Razor Boomerang's return to hand) unattaches that Equipment as the cost, the effects still name it, and a creature that has the words of its own cannot pay it; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, giant};
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::{names, pid};

const SEEK: &str = r#"{"Activated":{"cost":["Tap","Unattach"],"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const BOOMERANG: &str = r#"{"Activated":{"cost":["Tap","Unattach"],"effects":[{"Destroy":{"what":{"Target":0}}},{"Move":{"to":"Hand","what":"Granter"}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;

/// An Equipment named `name` that gives the equipped creature the ability `granted`.
fn equipment(name: &str, granted: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Static":{{"affected":"Attached","mods":[{{"GrantAbility":{granted}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_mana_cost(cost(1, &[]))
            .with_rules(rule),
    )
}

/// Seat 0's Grizzly Bears wears the Equipment; the last seat has a Craw Wurm.
fn table(seats: usize, gear: Arc<CardDefinition>) -> Scenario {
    Scenario::new(seats)
        .battlefield(0, vec![ready(bear()), attached_to(ready(gear), 0, 0)])
        .battlefield(seats - 1, vec![ready(giant())])
        .at(1, 0, Step::Main1)
}

#[rstest]
#[case::it_stays(SEEK, Zone::Battlefield)]
#[case::it_returns_to_hand(BOOMERANG, Zone::Hand(pid(0)))]
fn the_equipment_that_gave_the_ability_is_unattached_as_its_cost(
    #[case] granted: &str,
    #[case] ends_in: Zone,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, equipment("Seeker", granted)))?;
    let gear = driver.id_of("Seeker")?;
    driver.begin_activation("Grizzly Bears")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let giant = driver.target_named("Craw Wurm")?;
        driver.choose_targets(&[giant])?;
    }
    let unattached = driver
        .state()
        .object(gear)
        .and_then(|object| object.permanent())
        .is_some_and(|permanent| permanent.attached_to().is_none());
    assert!(unattached, "the cost unattached the Equipment");
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.pass()?;
    }
    let seat = pid(seats - 1);
    assert_eq!(names(driver.state(), Zone::Graveyard(seat)), ["Craw Wurm"]);
    assert!(names(driver.state(), ends_in).contains(&"Seeker".to_owned()));
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_creature_with_the_words_of_its_own_cannot_pay(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rule: Ability = serde_json::from_str(SEEK).unwrap();
    let def = CardDefinition::new("Plain Seeker")
        .with_type(CardType::Creature)
        .with_mana_cost(cost(1, &[]))
        .with_power_toughness(1, 1)
        .with_rules(rule);
    let scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(Arc::new(def))])
        .battlefield(seats - 1, vec![ready(giant())])
        .at(1, 0, Step::Main1);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        driver.offered_abilities("Plain Seeker")?,
        Vec::<usize>::new()
    );
    Ok(())
}
