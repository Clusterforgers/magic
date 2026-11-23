//! Goal: a copy of an Aura spell whose effect lets its controller choose new targets (CR 707.10c) asks for them at tables of two to four seats: a new enchant target chosen with the target decision is where the copy token Aura enters attached, a kept target keeps it on the original's, the original stays on its own, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{game, on_the_stack, replays, run, say, spell_copies};
use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// "Copy target spell. You may choose new targets for the copy.", free.
const COPIER: &str = r#"{"Spell":{"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}],"targets":[{"what":{"Object":["Spell"]}}]}}"#;

fn card(name: &str, kind: CardType, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

/// A free "Enchant land" Aura.
fn ring() -> Arc<CardDefinition> {
    let rules = r#"{"Keyword":{"Enchant":[{"Type":"Land"}]}}"#;
    Arc::new(Arc::unwrap_or_clone(card("Ring", CardType::Enchantment, rules)).with_subtype("Aura"))
}

fn land(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Land))
}

/// What the permanent named `name` that is (or is not) a token is attached to.
fn host(driver: &Driver, name: &str, token: bool) -> Option<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .map(|id| state.object(id).unwrap())
        .find(|object| object.card().name == name && (object.kind() == ObjectKind::Token) == token)
        .and_then(|object| object.permanent()?.attached_to().map(|to| to.id))
}

/// Seat 0 casts Ring at Home Field and copies it; the copy gets `new` as its target, or keeps Home Field.
#[rstest]
#[case::a_new_target_on_an_opponents_land(Some("Far Field"))]
#[case::a_new_target_on_the_casters_other_land(Some("Back Field"))]
#[case::the_target_kept(None)]
fn a_copy_of_an_aura_spell_may_choose_a_new_target(
    #[case] new: Option<&str>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 0, 0)
            .hand(0, [ring(), card("Copier", CardType::Instant, COPIER)])
            .battlefield(
                0,
                vec![ready(land("Home Field")), ready(land("Back Field"))],
            )
            .battlefield(1, vec![ready(land("Far Field"))])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let home = driver.target_named("Home Field")?;
    driver.cast_at("Ring", &[home])?;
    let spell = on_the_stack(&driver, "Ring");
    driver.cast_at("Copier", &[spell])?;
    run(&mut driver, seats)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    assert_eq!(question.text, "Choose new targets for the copy of Ring?");
    say(&mut driver, new.is_some())?;
    let landed = if let Some(new) = new {
        let new = driver.target_named(new)?;
        let decision = driver.decision()?;
        assert_eq!(decision.player, pid(0), "the copy's controller chooses");
        let DecisionKind::ChooseTargets(options) = &decision.kind else {
            panic!("{:?}", decision.kind.name());
        };
        assert!(options.legal.contains(&new), "{:?}", options.legal);
        driver.choose_targets(&[new])?;
        new
    } else {
        home
    };
    let copies = spell_copies(&driver);
    assert_eq!(copies.len(), 1);
    assert_eq!(
        driver.state().object(copies[0]).unwrap().choices().targets,
        vec![vec![landed]]
    );
    run(&mut driver, seats)?;
    assert_eq!(driver.state().stack(), []);
    let id = |target: Target| target.object().unwrap().id;
    assert_eq!(
        host(&driver, "Ring", true),
        Some(id(landed)),
        "the copy token Aura"
    );
    assert_eq!(
        host(&driver, "Ring", false),
        Some(id(home)),
        "the original Aura"
    );
    replays(&driver, &setup);
    Ok(())
}
