//! Goal: the rules fix pass 9 writes play as printed at tables of two to four seats: "if a Werewolf is dealt damage this way" holds only when the damage was really dealt (not prevented), also when the source was sacrificed to pay for it, and "if it was a permanent card" tests the exiled card's types.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, instant};
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact with one activated ability: `cost` and `effects` as the card DSL writes them.
fn rig(name: &str, cost: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    artifact_with(
        name,
        vec![ability(&format!(
            r#"{{"Activated":{{"cost":{cost},"effects":{effects},"targets":{targets}}}}}"#
        ))],
    )
}

/// Activates `name`, aims at `aim`, and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str, aim: Target) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        let done = matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty();
        if done {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![aim]))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "40 answers",
    })
}

fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect()
}

/// Silver Bolt as translated: 1 damage (the 5/5 survives it), then destroy the target if it is a Werewolf the source dealt damage to.
const BOLT: &str = r#"[{"DealDamage":{"amount":1,"to":{"Target":0}}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[{"Subtype":"Werewolf"},{"DamagedBy":"This"}],"subject":{"Target":0}}},"then":[{"Destroy":{"what":{"Target":0}}}]}}]"#;

/// A 5/5 creature of the subtype; `shielded` prevents all damage dealt to it.
fn victim(subtype: &str, shielded: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Victim")
        .with_type(CardType::Creature)
        .with_subtype(subtype)
        .with_power_toughness(5, 5);
    if shielded {
        card = card.with_rules(ability(
            r#"{"Replacement":{"by":"Prevent","when":{"Damage":{"to":"This"}}}}"#,
        ));
    }
    Arc::new(card)
}

#[rstest]
fn a_werewolf_is_destroyed_only_when_it_was_really_dealt_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(r#"["Tap"]"#, r#"["SacrificeSelf"]"#)] cost: &str,
    #[values(("Werewolf", false, true), ("Werewolf", true, false), ("Wolf", false, false))] case: (
        &str,
        bool,
        bool,
    ),
) -> Result<(), ScriptError> {
    let (subtype, shielded, destroyed) = case;
    let owner = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![ready(rig(
                    "Bolt",
                    cost,
                    BOLT,
                    r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
                ))],
            )
            .battlefield(owner, vec![ready(victim(subtype, shielded))]),
    )?;
    let aim = driver.target_named("Victim")?;
    run(&mut driver, "Bolt", aim)?;
    let dead = names_in(&driver, Zone::Graveyard(pid(owner))).contains(&"Victim".to_owned());
    assert_eq!(dead, destroyed, "{subtype} shielded {shielded} cost {cost}");
    Ok(())
}

/// Lion Sash as translated: exile target card from a graveyard; if it was a permanent card, gain 3 life (for a counter).
const SASH: &str = r#"[{"Exile":{"as":"exiled","what":{"Target":0}}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}],"subject":{"Bound":"exiled"}}},"then":[{"GainLife":{"amount":3}}]}}]"#;

#[rstest]
fn only_an_exiled_permanent_card_counts(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] permanent: bool,
) -> Result<(), ScriptError> {
    let owner = seats - 1;
    let card = if permanent {
        free_creature("Dead", 2, 2)
    } else {
        instant("Dead", 1, &[])
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![ready(rig(
                    "Sash",
                    r#"["Tap"]"#,
                    SASH,
                    r#"[{"what":{"Object":[{"In":"Graveyard"}]}}]"#,
                ))],
            )
            .graveyard(owner, vec![card]),
    )?;
    let dead = driver.state().zone(Zone::Graveyard(pid(owner)))[0];
    let aim = Target::Object(driver.state().object(dead).unwrap().reference());
    run(&mut driver, "Sash", aim)?;
    assert!(names_in(&driver, Zone::Exile).contains(&"Dead".to_owned()));
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, if permanent { 23 } else { 20 });
    Ok(())
}
