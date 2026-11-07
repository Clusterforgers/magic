//! Goal: an Aura sacrificed to pay for its own ability still names the creature it enchanted ("enchanted creature",
//! last known information, CR 112.7a and 608.2h) at tables of two to four seats: Briar Shield pumps it, Carapace
//! regenerates it, Phantom Wings returns it to its owner's hand, a Crown pumps it and the creatures sharing a type with
//! it, and a creature that left before resolution is not named (CR 400.7); and Rising Waters untaps a land of the
//! player whose upkeep it is.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, tapped};
use rstest::rstest;

use crate::common::{names, pid};

/// An Aura that enchants a creature and carries these rules (as the card DSL writes them).
fn aura(name: &str, rules: &str) -> Arc<CardDefinition> {
    let rules: Vec<Ability> = serde_json::from_str(rules).unwrap();
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Enchantment)
        .with_subtype("Aura")
        .with_rules(
            serde_json::from_str(r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#).unwrap(),
        );
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

const BRIAR: &str = r#"[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Continuous":{"affected":"Attached","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":3,"toughness":3}}]}}]}},{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}]"#;
const CARAPACE: &str =
    r#"[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Regenerate":{"what":"Attached"}}]}}]"#;
const WINGS: &str = r#"[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Move":{"to":"Hand","what":"Attached"}}]}}]"#;

/// Seat 0 has Mine (a 2/2) wearing `aura`.
fn worn(seats: usize, aura: Arc<CardDefinition>) -> Scenario {
    Scenario::new(seats).battlefield(
        0,
        vec![
            ready(free_creature("Mine", 2, 2)),
            attached_to(ready(aura), 0, 0),
        ],
    )
}

/// Activates `name` and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("the activation".into()))
}

#[rstest]
fn briar_shield_pumps_the_creature_it_was_sacrificed_from(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(worn(seats, aura("Briar Shield", BRIAR)))?;
    run(&mut driver, "Briar Shield")?;
    let mine = driver.id_of("Mine").unwrap();
    let shown = characteristics(driver.state(), mine).unwrap();
    assert_eq!((shown.power, shown.toughness), (Some(5), Some(5)));
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Briar Shield".to_owned()));
    Ok(())
}

#[rstest]
fn phantom_wings_returns_the_creature_it_was_sacrificed_from(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(worn(seats, aura("Phantom Wings", WINGS)))?;
    run(&mut driver, "Phantom Wings")?;
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        vec!["Mine".to_owned()]
    );
    Ok(())
}

#[rstest]
fn carapace_gives_the_creature_it_was_sacrificed_from_a_shield(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(worn(seats, aura("Carapace", CARAPACE)))?;
    run(&mut driver, "Carapace")?;
    let mine = driver.id_of("Mine").unwrap();
    let shields = (driver.state().rule_effects().iter())
        .filter(|effect| matches!(effect.rule, Rule::Regenerate(on) if on.id == mine))
        .count();
    assert_eq!(shields, 1);
    Ok(())
}

/// A creature that moved after the Aura was sacrificed is a new object: the ability names nothing (CR 400.7).
#[rstest]
fn a_creature_that_left_is_not_named(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).battlefield(
        0,
        vec![
            ready(free_creature("Mine", 2, 2)),
            attached_to(ready(aura("Phantom Wings", WINGS)), 0, 0),
            attached_to(ready(aura("Briar Shield", BRIAR)), 0, 0),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    // Briar Shield's pump waits on the stack while Phantom Wings returns Mine; the pump then names nothing.
    driver.begin_activation("Briar Shield")?;
    run(&mut driver, "Phantom Wings")?;
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        vec!["Mine".to_owned()]
    );
    assert!(driver.state().effects().is_empty());
    Ok(())
}

/// Rising Waters as translated, with `count` on its choice of land.
fn waters(count: &str) -> Arc<CardDefinition> {
    let rules: Vec<Ability> = serde_json::from_str(&format!(
        r#"[{{"Triggered":{{"effects":[{{"Choose":{{"as":"untap",{count}"from":{{"Each":[{{"Type":"Land"}},{{"ControlledBy":"ActivePlayer"}}]}},"who":"Triggering"}}}},{{"Untap":{{"what":{{"Bound":"untap"}}}}}}],"when":{{"StepBegins":{{"step":"Upkeep"}}}}}}}},{{"Static":{{"affected":{{"Each":[{{"Type":"Land"}}]}},"mods":["DoesntUntap"]}}}}]"#
    ))
    .unwrap();
    let mut card = CardDefinition::new("Rising Waters").with_type(CardType::Enchantment);
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

/// The upkeep player of the next turn (seat 1) untaps one of their two tapped lands when the choice is exactly one;
/// "up to one" (the old reading) lets a passive player untap none.
#[rstest]
#[case::exactly_one("", 1)]
#[case::up_to_one(r#""count":{"max":1,"min":0},"#, 0)]
fn rising_waters_untaps_a_land_of_the_upkeep_player(
    #[values(2, 3, 4)] seats: usize,
    #[case] count: &str,
    #[case] expected: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::End)
        .battlefield(0, vec![ready(waters(count))])
        .battlefield(1, vec![tapped(mountain()), tapped(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Upkeep)?;
    for _ in 0..40 {
        if driver.state().turn().step() != Step::Upkeep {
            break;
        }
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    let untapped = (driver.state().zone(Zone::Battlefield).iter())
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.card().name == "Mountain" && !o.permanent().unwrap().tapped())
        .count();
    assert_eq!(untapped, expected);
    Ok(())
}

/// Crown of Fury as translated: the enchanted creature and every creature sharing a creature type with it.
const FURY: &str = r#"[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"AnyOf":[{"Is":"Attached"},{"SharesSubtype":"Attached"}]}]},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}]}}]"#;

fn typed(name: &str, subtype: Option<&str>) -> Arc<CardDefinition> {
    let mut card = (*free_creature(name, 2, 2)).clone();
    if let Some(subtype) = subtype {
        card = card.with_subtype(subtype);
    }
    Arc::new(card)
}

/// Sacrificing the Crown pumps its host (even one with no creature type) and each creature of any player that shares
/// a creature type with it, and no other creature.
#[rstest]
#[case::goblin_host(Some("Goblin"), [3, 3, 2, 3])]
#[case::host_without_a_type(None, [3, 2, 2, 2])]
fn a_crown_pumps_its_host_and_the_creatures_sharing_a_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] host: Option<&str>,
    #[case] powers: [i32; 4],
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            vec![
                ready(typed("Mine", host)),
                attached_to(ready(aura("Crown of Fury", FURY)), 0, 0),
                ready(typed("Kin", Some("Goblin"))),
                ready(typed("Stranger", Some("Elf"))),
            ],
        )
        .battlefield(1, vec![ready(typed("Theirs", Some("Goblin")))]);
    let mut driver = Driver::from_scenario(scenario)?;
    run(&mut driver, "Crown of Fury")?;
    for (name, power) in ["Mine", "Kin", "Stranger", "Theirs"]
        .into_iter()
        .zip(powers)
    {
        let id = driver.id_of(name).unwrap();
        assert_eq!(
            characteristics(driver.state(), id).unwrap().power,
            Some(power),
            "{name}"
        );
    }
    Ok(())
}
