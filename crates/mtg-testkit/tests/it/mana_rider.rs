//! Goal: mana that sets something off when it is spent to cast a spell (Path of Ancestry: scry 1 when its mana pays for a creature spell that shares a creature type with the commander) triggers once for each such mana spent, as the spell is cast and above it on the stack, for exactly the spells that satisfy the filter; mana that is not spent sets nothing off, the rider never restricts what the mana pays for, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::spend::SpendOn;
use mtg_core::card::{CardType, Supertype};
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

const SPELL: &str = "Spell";
const PATH: &str = "Path of Ancestry";

/// The card with the creature type `name`; `Changeling` stands for the keyword that makes it every type.
fn with_type_or_changeling(card: CardDefinition, name: &str) -> CardDefinition {
    if name == "Changeling" {
        card.with_rules(serde_json::from_str::<Ability>(r#"{"Keyword":"Changeling"}"#).unwrap())
    } else {
        card.with_subtype(name)
    }
}

/// The green commander of seat 0 with these creature types.
fn commander(types: &[&str]) -> Arc<CardDefinition> {
    named_commander("Cmd", types)
}

/// A green commander called `name` with these creature types.
fn named_commander(name: &str, types: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_mana_cost(ManaCost::parse("{G}").unwrap())
        .with_supertype(Supertype::Legendary)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for subtype in types {
        card = with_type_or_changeling(card, subtype);
    }
    Arc::new(card)
}

/// A spell of these card types and subtypes costing `cost`.
fn spell(types: &[CardType], subtypes: &[&str], cost: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(SPELL).with_mana_cost(ManaCost::parse(cost).unwrap());
    for card_type in types {
        card = card.with_type(*card_type);
    }
    for subtype in subtypes {
        card = with_type_or_changeling(card, subtype);
    }
    if types.contains(&CardType::Creature) {
        card = card.with_power_toughness(1, 1);
    }
    Arc::new(card)
}

/// A Commander table where seat 0 holds `spell`, has the commander in the command zone, `paths` Paths of Ancestry and a
/// Forest in play and a library to scry.
fn table(
    seats: usize,
    commander_types: &[&str],
    spell: Arc<CardDefinition>,
    paths: usize,
) -> Scenario {
    table_of(seats, vec![commander(commander_types)], spell, paths)
}

/// [`table`] with these commanders.
fn table_of(
    seats: usize,
    commanders: Vec<Arc<CardDefinition>>,
    spell: Arc<CardDefinition>,
    paths: usize,
) -> Scenario {
    let mut lands = vec![ready(forest())];
    lands.extend((0..paths).map(|_| ready(card(PATH).unwrap())));
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, commanders)
        .library(0, vec![forest(); 8])
        .hand(0, [spell])
        .battlefield(0, lands)
}

fn count(events: &[Event], pick: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|event| pick(event)).count()
}

fn delayed(events: &[Event]) -> usize {
    count(events, |e| matches!(e, Event::DelayedTriggerCreated(_)))
}

/// Passes priority and answers the scry until the stack is empty; every event on the way.
fn play_out(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..40 {
        let answer = match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(events),
            DecisionKind::Priority(_) => {
                events.extend(driver.pass()?);
                continue;
            }
            DecisionKind::ChooseCards(_) => {
                Answer::ChooseCards(mtg_engine::choose::CardsChoice(Vec::new()))
            }
            DecisionKind::OrderCards(options) => {
                Answer::OrderCards(mtg_engine::choose::OrderChoice(options.cards.clone()))
            }
            other => panic!("unexpected {}", other.name()),
        };
        events.extend(driver.answer(answer)?);
    }
    panic!("the stack did not resolve");
}

use CardType::{Artifact, Creature, Instant};

#[rstest]
#[case::a_shared_type(&["Elf", "Warrior"], &[Creature], &["Elf", "Druid"], 1)]
#[case::one_of_several_types(&["Elf", "Warrior"], &[Creature], &["Warrior"], 1)]
#[case::an_artifact_creature(&["Elf"], &[Artifact, Creature], &["Elf"], 1)]
#[case::another_type(&["Elf"], &[Creature], &["Goblin"], 0)]
#[case::a_spell_with_no_creature_type(&["Elf"], &[Creature], &[], 0)]
#[case::a_commander_with_no_creature_type(&[], &[Creature], &["Elf"], 0)]
#[case::a_noncreature_spell_of_the_type(&["Elf"], &[Instant], &["Elf"], 0)]
#[case::a_changeling_spell(&["Elf"], &[Creature], &["Changeling"], 1)]
#[case::a_changeling_commander(&["Changeling"], &[Creature], &["Goblin"], 1)]
#[case::both_changelings(&["Changeling"], &[Creature], &["Changeling"], 1)]
#[case::a_changeling_commander_and_a_spell_with_no_type(&["Changeling"], &[Creature], &[], 0)]
#[case::a_changeling_spell_and_a_commander_with_no_type(&[], &[Creature], &["Changeling"], 0)]
fn the_scry_comes_with_a_creature_spell_that_shares_a_type_with_the_commander(
    #[case] commander_types: &[&str],
    #[case] types: &[CardType],
    #[case] subtypes: &[&str],
    #[case] triggers: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, commander_types, spell(types, subtypes, "{G}"), 1);
    let setup: GameSetup = scenario.setup().unwrap();
    let mut driver = Driver::from_scenario(table(
        seats,
        commander_types,
        spell(types, subtypes, "{G}"),
        1,
    ))?;
    let cast = driver.cast_tapping(SPELL, &[PATH])?;
    assert_eq!(delayed(&cast), triggers, "{cast:?}");
    assert_eq!(
        driver.state().stack().len(),
        1 + triggers,
        "above the spell"
    );
    let played = play_out(&mut driver)?;
    assert_eq!(count(&played, |e| matches!(e, Event::Scried(_))), triggers);
    assert!(driver.state().delayed().is_empty(), "nothing waits on");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn two_such_mana_spent_give_two_triggers(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let make = || table(seats, &["Elf"], spell(&[Creature], &["Elf"], "{G}{G}"), 2);
    let setup = make().setup().unwrap();
    let mut driver = Driver::from_scenario(make())?;
    let cast = driver.cast_tapping(SPELL, &[PATH, PATH])?;
    assert_eq!(delayed(&cast), 2, "{cast:?}");
    // Two abilities of one player triggered together: the player orders them.
    assert_eq!(driver.trigger_options()?.triggers.len(), 2);
    driver.order_triggers(&[0, 1])?;
    assert_eq!(driver.state().stack().len(), 3);
    let played = play_out(&mut driver)?;
    assert_eq!(count(&played, |e| matches!(e, Event::Scried(_))), 2);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn only_the_mana_of_the_land_counts_when_another_pays_too() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        2,
        &["Elf"],
        spell(&[Creature], &["Elf"], "{G}{G}"),
        1,
    ))?;
    let cast = driver.cast_tapping(SPELL, &[PATH, "Forest"])?;
    assert_eq!(delayed(&cast), 1, "{cast:?}");
    Ok(())
}

#[test]
fn mana_in_the_pool_sets_off_the_scry_when_a_later_spell_spends_it() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(2, &["Elf"], spell(&[Creature], &["Elf"], "{G}"), 1))?;
    let tapped = driver.tap_for_mana(PATH)?;
    assert_eq!(delayed(&tapped), 0, "nothing is spent yet");
    let path = driver.id_of(PATH).unwrap();
    let pool = driver.state().player(pid(0)).unwrap().mana().clone();
    assert!(matches!(
        pool.restricted(),
        [unit] if unit.mana == ManaType::Green
            && matches!(&unit.only[..], [SpendOn::When(rider)] if rider.source == Some(path))
    ));
    let cast = driver.cast(SPELL)?;
    assert_eq!(delayed(&cast), 1, "{cast:?}");
    assert!(driver.state().player(pid(0)).unwrap().mana().is_empty());
    Ok(())
}

#[test]
fn mana_that_is_not_spent_sets_nothing_off() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(2, &["Elf"], spell(&[Creature], &["Elf"], "{G}"), 1))?;
    driver.tap_for_mana(PATH)?;
    driver.pass_until(Step::BeginCombat)?;
    assert!(driver.state().player(pid(0)).unwrap().mana().is_empty());
    assert_eq!(delayed(driver.engine().events()), 0);
    assert_eq!(driver.state().delayed(), []);
    assert_eq!(driver.state().stack(), []);
    Ok(())
}

#[test]
fn a_spell_that_costs_less_than_the_mana_spends_one_and_keeps_the_other() -> Result<(), ScriptError>
{
    let mut driver =
        Driver::from_scenario(table(2, &["Elf"], spell(&[Creature], &["Elf"], "{G}"), 2))?;
    driver.tap_for_mana(PATH)?;
    driver.tap_for_mana(PATH)?;
    let cast = driver.cast(SPELL)?;
    assert_eq!(delayed(&cast), 1, "{cast:?}");
    let pool = driver.state().player(pid(0)).unwrap().mana().clone();
    assert_eq!(pool.restricted().len(), 1, "the other mana is still there");
    assert_eq!(driver.state().stack().len(), 2);
    Ok(())
}

#[test]
fn the_rider_restricts_nothing_so_the_land_pays_for_any_spell() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(2, &["Elf"], spell(&[Instant], &["Goblin"], "{G}"), 1))?;
    let cast = driver.cast_tapping(SPELL, &[PATH])?;
    assert_eq!(delayed(&cast), 0);
    assert_eq!(driver.state().stack().len(), 1);
    assert!(
        driver.state().delayed().is_empty(),
        "no trigger is left waiting"
    );
    Ok(())
}

#[rstest]
#[case::the_first_partner(&["Elf"], &["Goblin"], &["Elf"], 1)]
#[case::the_second_partner(&["Elf"], &["Goblin"], &["Goblin"], 1)]
#[case::neither_partner(&["Elf"], &["Goblin"], &["Human"], 0)]
#[case::a_partner_with_no_type(&[], &["Goblin"], &["Goblin"], 1)]
fn either_partner_commander_counts(
    #[case] first: &[&str],
    #[case] second: &[&str],
    #[case] subtypes: &[&str],
    #[case] triggers: usize,
) -> Result<(), ScriptError> {
    let make = || {
        table_of(
            3,
            vec![
                named_commander("Cmd", first),
                named_commander("Cmd Two", second),
            ],
            spell(&[Creature], subtypes, "{G}"),
            1,
        )
    };
    let setup = make().setup().unwrap();
    let mut driver = Driver::from_scenario(make())?;
    let cast = driver.cast_tapping(SPELL, &[PATH])?;
    assert_eq!(delayed(&cast), triggers, "{cast:?}");
    replays(&driver, &setup);
    Ok(())
}

/// A land that adds {R} and scries 1 when that mana is spent to cast a red instant or sorcery spell: the rider names a
/// colour and two card types, not a commander.
fn pyre_land() -> Arc<CardDefinition> {
    let ability = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},
        "when_spent":{"spell":{"types":["Instant","Sorcery"],"colors":["Red"]},"effects":[{"Scry":{}}]}}}]}}"#;
    Arc::new(
        CardDefinition::new("Pyre Land")
            .with_type(CardType::Land)
            .with_rules(serde_json::from_str::<Ability>(ability).unwrap()),
    )
}

#[rstest]
#[case::a_red_instant(&[Instant], "{R}", &["Pyre Land"], 1)]
#[case::a_red_sorcery(&[CardType::Sorcery], "{R}", &["Pyre Land"], 1)]
#[case::a_red_creature(&[Creature], "{R}", &["Pyre Land"], 0)]
#[case::a_multicolored_instant_with_red(&[Instant], "{R}{G}", &["Pyre Land", "Forest"], 1)]
#[case::a_colorless_instant(&[Instant], "{1}", &["Pyre Land"], 0)]
#[case::a_green_instant(&[Instant], "{1}{G}", &["Pyre Land", "Forest"], 0)]
fn a_rider_can_name_a_color_and_card_types(
    #[case] types: &[CardType],
    #[case] cost: &str,
    #[case] lands: &[&str],
    #[case] triggers: usize,
) -> Result<(), ScriptError> {
    let make = || {
        Scenario::new(2)
            .library(0, vec![forest(); 8])
            .hand(0, [spell(types, &[], cost)])
            .battlefield(0, [ready(pyre_land()), ready(forest())])
    };
    let setup = make().setup().unwrap();
    let mut driver = Driver::from_scenario(make())?;
    let cast = driver.cast_tapping(SPELL, lands)?;
    assert_eq!(delayed(&cast), triggers, "{cast:?}");
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_companion_is_not_a_commander() -> Result<(), ScriptError> {
    let companion = Arc::new(
        CardDefinition::new("Companion")
            .with_type(Creature)
            .with_subtype("Elf")
            .with_power_toughness(1, 1),
    );
    let mut driver = Driver::from_scenario(
        table(2, &["Goblin"], spell(&[Creature], &["Elf"], "{G}"), 1).companion(0, companion),
    )?;
    let cast = driver.cast_tapping(SPELL, &[PATH])?;
    assert_eq!(delayed(&cast), 0, "{cast:?}");
    Ok(())
}
