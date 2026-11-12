//! Goal: the damage pack g11b plays as printed at tables of two to four seats: damage that can't be prevented passes a
//! prevention effect on the creature and kills it, the same damage without the flag is prevented, an unshielded creature
//! dies either way, a shield of the next X damage prevents the number it counted as it began, a shield of the sources
//! of a chosen colour keeps that colour after the resolution, and a redirection sends all of the damage, or the next
//! damage only, to the target fixed as it began.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{OptionChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::cost;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact with one activated ability that costs `{T}`: `effects` and `targets` as the card DSL writes them.
fn rig(name: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    artifact_with(
        name,
        vec![ability(&format!(
            r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":{targets}}}}}"#
        ))],
    )
}

/// Activates `name`, aims at `aim`, picks the option at `pick` when asked, and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str, aim: Target, pick: usize) -> Result<(), ScriptError> {
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
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(pick)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("a finished resolution".to_owned()))
}

fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect()
}

const CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;

/// A 4/4 creature; `shield` is a replacement that prevents damage dealt to it, when given.
fn victim(shield: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Victim")
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4);
    if let Some(shield) = shield {
        card = card.with_rules(ability(shield));
    }
    Arc::new(card)
}

const PREVENT_ALL: &str = r#"{"Replacement":{"by":"Prevent","when":{"Damage":{"to":"This"}}}}"#;
const PREVENT_HALF: &str =
    r#"{"Replacement":{"by":"PreventHalf","when":{"Damage":{"to":"This"}}}}"#;

#[rstest]
fn damage_that_cant_be_prevented_passes_the_prevention(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(PREVENT_ALL), Some(PREVENT_HALF))] shield: Option<&str>,
    #[values(true, false)] unpreventable: bool,
) -> Result<(), ScriptError> {
    let owner = seats - 1;
    let flag = if unpreventable {
        r#","unpreventable":true"#
    } else {
        ""
    };
    let effects = format!(r#"[{{"DealDamage":{{"amount":5,"to":{{"Target":0}}{flag}}}}}]"#);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, vec![ready(rig("Bolt", &effects, CREATURE))])
            .battlefield(owner, vec![ready(victim(shield))]),
    )?;
    let aim = driver.target_named("Victim")?;
    run(&mut driver, "Bolt", aim, 0)?;
    let dead = names_in(&driver, Zone::Graveyard(pid(owner))).contains(&"Victim".to_owned());
    assert_eq!(dead, unpreventable || shield.is_none(), "{shield:?}");
    Ok(())
}

/// A 1/1 Cleric.
fn cleric() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Cleric")
            .with_type(CardType::Creature)
            .with_subtype("Cleric")
            .with_power_toughness(1, 1),
    )
}

/// Prevents the next X damage to target creature this turn, where X is the number of Clerics on the battlefield.
const MEDIC: &str = r#"[{"Shield":{"by":{"PreventCounted":{"Count":[{"Subtype":"Cleric"}]}},"duration":"UntilEndOfTurn","when":{"Damage":{"to":{"Target":0}}}}}]"#;

#[rstest]
fn a_counted_shield_prevents_the_number_it_counts_as_it_begins(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] clerics: usize,
    #[values(true, false)] unpreventable: bool,
) -> Result<(), ScriptError> {
    let owner = seats - 1;
    let flag = if unpreventable {
        r#","unpreventable":true"#
    } else {
        ""
    };
    let bolt = format!(r#"[{{"DealDamage":{{"amount":5,"to":{{"Target":0}}{flag}}}}}]"#);
    let mut mine = vec![
        ready(rig("Medic", MEDIC, CREATURE)),
        ready(rig("Bolt", &bolt, CREATURE)),
    ];
    mine.extend((0..clerics).map(|_| ready(cleric())));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, mine)
            .battlefield(owner, vec![ready(victim(None))]),
    )?;
    let aim = driver.target_named("Victim")?;
    run(&mut driver, "Medic", aim, 0)?;
    run(&mut driver, "Bolt", aim, 0)?;
    // 5 damage to a 4/4: two prevented save it.
    let dead = names_in(&driver, Zone::Graveyard(pid(owner))).contains(&"Victim".to_owned());
    assert_eq!(dead, unpreventable || clerics < 2, "{clerics} clerics");
    Ok(())
}

/// Prevents all damage that sources of the color of your choice would deal this turn.
const STRANDS: &str = r#"[{"ChooseValue":{"as":"chosen","kind":"Color"}},{"Shield":{"by":"Prevent","duration":"UntilEndOfTurn","when":{"Damage":{"source":[{"ChosenAs":"chosen"}]}}}}]"#;

/// A red artifact that deals 4 damage to target creature for `{T}`.
fn red_bolt() -> Arc<CardDefinition> {
    let card = (*rig(
        "Bolt",
        r#"[{"DealDamage":{"amount":4,"to":{"Target":0}}}]"#,
        CREATURE,
    ))
    .clone()
    .with_mana_cost(cost(0, &[Color::Red]));
    Arc::new(card)
}

/// Colors are offered white, blue, black, red and green.
#[rstest]
fn a_shield_keeps_the_color_chosen_as_it_began(
    #[values(2, 3, 4)] seats: usize,
    #[values((3, false), (4, true), (0, true))] case: (usize, bool),
) -> Result<(), ScriptError> {
    let (pick, dead) = case;
    let owner = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![ready(rig("Strands", STRANDS, "[]")), ready(red_bolt())],
            )
            .battlefield(owner, vec![ready(victim(None))]),
    )?;
    let aim = driver.target_named("Victim")?;
    run(&mut driver, "Strands", aim, pick)?;
    run(&mut driver, "Bolt", aim, 0)?;
    let died = names_in(&driver, Zone::Graveyard(pid(owner))).contains(&"Victim".to_owned());
    assert_eq!(died, dead, "option {pick}");
    Ok(())
}

/// A 3/3 creature with "{T}: damage that would be dealt to it this turn is dealt to target creature instead", the next
/// time only when `next`.
fn treefolk(next: bool) -> Arc<CardDefinition> {
    let by = if next {
        r#"{"Redirect":{"to":{"Target":0},"next":true}}"#
    } else {
        r#"{"Redirect":{"to":{"Target":0}}}"#
    };
    let shield = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"Shield":{{"by":{by},"duration":"UntilEndOfTurn","when":{{"Damage":{{"to":"This"}}}}}}}}],"targets":{CREATURE}}}}}"#
    );
    Arc::new(
        CardDefinition::new("Treefolk")
            .with_type(CardType::Creature)
            .with_power_toughness(3, 3)
            .with_rules(ability(&shield)),
    )
}

#[rstest]
fn a_redirection_sends_the_damage_to_the_target_fixed_as_it_began(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next: bool,
) -> Result<(), ScriptError> {
    let owner = seats - 1;
    let two = r#"[{"DealDamage":{"amount":2,"to":{"Target":0}}}]"#;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![
                    ready(treefolk(next)),
                    ready(rig("Bolt", two, CREATURE)),
                    ready(rig("Shock", two, CREATURE)),
                ],
            )
            .battlefield(owner, vec![ready(victim(None))]),
    )?;
    let victim = driver.target_named("Victim")?;
    let me = driver.target_named("Treefolk")?;
    run(&mut driver, "Treefolk", victim, 0)?;
    run(&mut driver, "Bolt", me, 0)?;
    run(&mut driver, "Shock", me, 0)?;
    // Two hits of 2: the 4/4 dies when both are sent to it, and survives the one a "next time" shield sends.
    let died = names_in(&driver, Zone::Graveyard(pid(owner))).contains(&"Victim".to_owned());
    assert_eq!(died, !next);
    let mine = names_in(&driver, Zone::Graveyard(pid(0)));
    assert!(!mine.contains(&"Treefolk".to_owned()), "{mine:?}");
    Ok(())
}
