//! Goal: the r28-5 engine pieces play as their rules say at tables of two to four seats: a spell cast only by a player attacked this combat, opponents held to the timing of a sorcery, a permanent that can't become untapped, creature cards that can't enter the battlefield from a graveyard, a macro keyword whose number is X, a creature that attacks as though it had haste, a rule from the command zone, and the turn's log of what an opponent destroyed or countered; each game replays to the state it reached.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use crate::macro_keywords::{play, table};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A free spell of the type with the rules given.
fn spell(name: &str, kind: CardType, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(cost(0, &[]));
    for rule in rules {
        card = card.with_rules(ability(rule));
    }
    Arc::new(card)
}

/// An artifact that holds the rule.
fn warden(rule: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Artifact)
            .with_rules(ability(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#))),
    )
}

/// Passes priority until `seat` holds it.
fn priority_to(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?;
        if matches!(decision.kind, DecisionKind::Priority(_)) && decision.player == pid(seat) {
            return Ok(());
        }
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("seat {seat} never received priority");
}

/// The object of the card named `name`, in whatever zone it is.
fn named(driver: &Driver, name: &str) -> mtg_engine::object::ObjectRef {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::reference)
        .unwrap()
}

/// The state replays from the log to the hash the game reached.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

const ATTACKED: &str = r#"{"CastOnly":{"condition":{"All":[{"Step":"DeclareAttackers"},{"AttackedThisCombat":"You"}]}}}"#;

#[rstest]
fn only_a_player_attacked_this_combat_casts_the_spell(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] attacked: usize,
    #[values(1, 2, 3)] caster: usize,
) -> Result<(), ScriptError> {
    if attacked >= seats || caster >= seats {
        return Ok(());
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(free_creature("Raider", 2, 2))]);
    for seat in 1..seats {
        scenario = scenario
            .hand(
                seat,
                vec![spell(
                    "Blade",
                    CardType::Instant,
                    &[ATTACKED, r#"{"Spell":{"effects":[]}}"#],
                )],
            )
            .library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario.library(0, (0..5).map(|_| forest())))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Raider", attacked)])?;
    priority_to(&mut driver, caster)?;
    assert_eq!(driver.cast("Blade").is_ok(), caster == attacked);
    replays(&driver);
    Ok(())
}

#[rstest]
fn no_one_casts_the_spell_outside_the_declare_attackers_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            1,
            vec![spell(
                "Blade",
                CardType::Instant,
                &[ATTACKED, r#"{"Spell":{"effects":[]}}"#],
            )],
        )
        .battlefield(0, vec![ready(free_creature("Raider", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    priority_to(&mut driver, 1)?;
    assert!(driver.cast("Blade").is_err());
    Ok(())
}

const SORCERY_TIMING: &str =
    r#"{"Cant":{"who":"Opponents","action":{"Cast":{"unless":"SorceryTiming"}}}}"#;

/// Whether `caster` may cast an instant in the main phase of `active`, with `waiting` spells on the stack first.
fn sorcery_timing_allows(seats: usize, active: usize, caster: usize, waiting: bool) -> bool {
    let instant = || spell("Quick", CardType::Instant, &[r#"{"Spell":{"effects":[]}}"#]);
    let mut scenario = Scenario::new(seats)
        .at(2, active, Step::Main1)
        .battlefield(0, vec![ready(warden(SORCERY_TIMING))]);
    for seat in 0..seats {
        scenario = scenario
            .hand(
                seat,
                vec![
                    instant(),
                    spell("Other", CardType::Instant, &[r#"{"Spell":{"effects":[]}}"#]),
                ],
            )
            .library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    priority_to(&mut driver, caster).unwrap();
    if waiting {
        driver.cast("Other").unwrap();
    }
    let allowed = driver.cast("Quick").is_ok();
    replays(&driver);
    allowed
}

#[rstest]
fn opponents_cast_only_as_a_sorcery_could_be_cast(#[values(2, 3, 4)] seats: usize) {
    for caster in 1..seats {
        assert!(
            sorcery_timing_allows(seats, caster, caster, false),
            "own main phase"
        );
        assert!(
            !sorcery_timing_allows(seats, caster, caster, true),
            "a spell waits"
        );
        assert!(
            !sorcery_timing_allows(seats, 0, caster, false),
            "another turn"
        );
    }
    assert!(
        sorcery_timing_allows(seats, seats - 1, 0, false),
        "the rule's controller"
    );
}

#[rstest]
fn a_permanent_that_cant_become_untapped_stays_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] ruled: bool,
) -> Result<(), ScriptError> {
    let rule = r#"{"Cant":{"who":{"Each":[{"Named":"Bear"}]},"action":"Untap"}}"#;
    let mut scenario = Scenario::new(seats).at(1, 0, Step::End).battlefield(
        1,
        vec![
            tapped(free_creature("Bear", 2, 2)),
            tapped(free_creature("Wolf", 2, 2)),
        ],
    );
    if ruled {
        scenario = scenario.battlefield(0, vec![ready(warden(rule))]);
    }
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Upkeep)?;
    let is_tapped = |name: &str| {
        let id = driver.id_of(name).unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    };
    assert_eq!(is_tapped("Bear"), ruled);
    assert!(!is_tapped("Wolf"));
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_creature_card_in_a_graveyard_cant_enter_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] ruled: bool,
) -> Result<(), ScriptError> {
    let rule = r#"{"Cant":{"who":{"Each":[{"Type":"Creature"},{"In":"Graveyard"}]},"action":"EnterBattlefield"}}"#;
    let raise = spell(
        "Raise",
        CardType::Sorcery,
        &[
            r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#,
        ],
    );
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![raise])
        .graveyard(0, vec![free_creature("Bear", 2, 2)]);
    if ruled {
        scenario = scenario.battlefield(seats - 1, vec![ready(warden(rule))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = Target::Object(named(&driver, "Bear"));
    driver.cast_at("Raise", &[bear])?;
    priority_to(&mut driver, 0)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let zone = driver
        .state()
        .object(named(&driver, "Bear").id)
        .unwrap()
        .zone();
    assert_eq!(zone == Zone::Battlefield, !ruled, "{zone:?}");
    replays(&driver);
    Ok(())
}

#[rstest]
fn firebending_x_adds_the_mana_its_amount_counts(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] others: usize,
) -> Result<(), ScriptError> {
    let keyword = Keyword::MacroX {
        name: "firebending".to_owned(),
        x: serde_json::from_str(r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#)
            .unwrap(),
    };
    let mut attackers = vec![keyworded("Bender", 2, 2, &[keyword])];
    attackers.extend((0..others).map(|at| free_creature(&format!("Friend {at}"), 1, 1)));
    let mut driver =
        Driver::from_scenario(table(seats, attackers, &[free_creature("Wall", 0, 9)]))?;
    play(&mut driver, &["Bender"], &[], &|_, _| None, Step::EndCombat)?;
    let total = driver.state().player(pid(0)).unwrap().mana().total();
    assert_eq!(total, u32::try_from(others + 1).unwrap());
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_creature_attacks_as_though_it_had_haste_only_under_the_rule(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] ruled: bool,
) -> Result<(), ScriptError> {
    let rule = r#"{"Combat":{"who":{"Each":[{"Named":"Fresh"}]},"quirk":"AttackAsThoughHaste"}}"#;
    let mut own = vec![mtg_testkit::sick(free_creature("Fresh", 2, 2))];
    if ruled {
        own.push(ready(warden(rule)));
    }
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, own);
    let driver = Driver::from_scenario(scenario)?;
    let fresh = driver.id_of("Fresh")?;
    let offered = match &driver.decision()?.kind {
        DecisionKind::DeclareAttackers(options) => options
            .attackers
            .iter()
            .any(|option| option.object == fresh),
        _ => false,
    };
    assert_eq!(offered, ruled);
    Ok(())
}

#[rstest]
fn a_spell_cost_rule_works_from_its_owners_command_zone(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] commanding: bool,
) -> Result<(), ScriptError> {
    let cost = r#"{"SpellCost":{"applies_to":["Spell",{"Type":"Sorcery"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}"}}"#;
    let eminent = Arc::new(
        CardDefinition::new("Eminent")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(cost_of(5))
            .with_rules(ability(&format!(
                r#"{{"Rule":{{"rule":{cost},"from":"Command"}}}}"#
            ))),
    );
    let costly = Arc::new(
        CardDefinition::new("Costly")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost_of(1))
            .with_rules(ability(r#"{"Spell":{"effects":[]}}"#)),
    );
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![costly]);
    if commanding {
        scenario = scenario
            .format(Arc::new(mtg_engine::format::Commander))
            .commanders(0, vec![eminent]);
    } else {
        scenario = scenario.hand(0, vec![eminent]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    eprintln!("DEBUG {:?}", driver.decision()?);
    let cast = driver.cast("Costly");
    assert_eq!(cast.is_ok(), commanding, "{cast:?}");
    Ok(())
}

/// A generic mana cost.
fn cost_of(generic: u32) -> mtg_core::mana::ManaCost {
    cost(generic, &[])
}

#[rstest]
fn the_log_names_what_an_opponents_spell_destroyed(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] caster: usize,
) -> Result<(), ScriptError> {
    let wreck = spell(
        "Wreck",
        CardType::Instant,
        &[
            r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        ],
    );
    let trap = spell(
        "Trap",
        CardType::Instant,
        &[
            r#"{"CastOnly":{"condition":{"Compare":{"left":{"Tally":{"event":"Destroyed","who":"Opponents","filter":[{"Not":{"Type":"Creature"}},{"ControlledBy":"You"}]}},"op":"Ge","right":1}}}}"#,
            r#"{"Spell":{"effects":[]}}"#,
        ],
    );
    let idol = Arc::new(CardDefinition::new("Idol").with_type(CardType::Artifact));
    let mut scenario = Scenario::new(seats)
        .at(1, caster, Step::Main1)
        .hand(caster, vec![wreck])
        .hand(0, vec![trap])
        .battlefield(0, vec![ready(idol)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let idol = Target::Object(
        driver
            .state()
            .object(driver.id_of("Idol")?)
            .unwrap()
            .reference(),
    );
    priority_to(&mut driver, caster)?;
    eprintln!(
        "DEBUG {:?} hand {:?}",
        driver.decision()?,
        driver
            .state()
            .zone(mtg_engine::zones::Zone::Hand(pid(caster)))
    );
    driver.cast_at("Wreck", &[idol])?;
    while !driver.state().stack().is_empty() {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    priority_to(&mut driver, 0)?;
    let cast = driver.cast("Trap");
    assert_eq!(cast.is_ok(), caster != 0, "{cast:?}");
    replays(&driver);
    Ok(())
}
