//! Goal: "change the target of target spell or ability with a single target" (CR 115.7a) changes the target of an Aura spell (its enchant target, CR 303.4a) as it does an instant's at tables of two to four seats: the player of the effect is asked, may pick their own land, and the Aura enters attached to the new target; a target with no other legal choice stays, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{game, on_the_stack, replays, run};
use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::{burn, cost, free_creature, mountain};
use mtg_testkit::{Driver, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// Mode 1 of Untimely Malfunction as its draft writes it, free.
const MALFUNCTION: &str = r#"{"Spell":{"effects":[{"ChangeTargets":{"to":"Another","what":{"Target":0}}}],"targets":[{"what":{"Object":[{"AnyOf":["Spell",{"Ability":"Activated"},{"Ability":"Triggered"}]},"SingleTarget"]}}]}}"#;

fn card(name: &str, kind: CardType, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

/// A free Aura with these enchant words.
fn aura(name: &str, enchant: &str) -> Arc<CardDefinition> {
    let rules = format!(r#"{{"Keyword":{{"Enchant":{enchant}}}}}"#);
    Arc::new(Arc::unwrap_or_clone(card(name, CardType::Enchantment, &rules)).with_subtype("Aura"))
}

fn land(name: &str, subtype: Option<&str>) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name).with_type(CardType::Land);
    Arc::new(match subtype {
        Some(subtype) => card.with_subtype(subtype),
        None => card,
    })
}

/// Seat 0 casts `spell` at `target`; seat 1 casts Malfunction at it, and the stack runs to the first question.
#[rstest]
#[case::an_aura_onto_the_retargeting_players_land("Ring", "Home Field", Some("Far Field"))]
#[case::an_instant_onto_another_creature("Zap", "Bear A", Some("Bear B"))]
#[case::a_target_with_no_other_legal_choice("Forest Ring", "Home Field", None)]
fn change_the_target_of_a_spell(
    #[case] spell: &str,
    #[case] target: &str,
    #[case] new: Option<&str>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 0, 0)
            .hand(
                0,
                [
                    aura("Ring", r#"[{"Type":"Land"}]"#),
                    aura("Forest Ring", r#"[{"Type":"Land"},{"Subtype":"Forest"}]"#),
                    burn("Zap", 2),
                ],
            )
            .battlefield(
                0,
                vec![
                    ready(land("Home Field", Some("Forest"))),
                    ready(free_creature("Bear B", 1, 3)),
                    ready(mountain()),
                ],
            )
            .battlefield(
                1,
                vec![
                    ready(land("Far Field", None)),
                    ready(free_creature("Bear A", 1, 3)),
                ],
            )
            .hand(1, [card("Malfunction", CardType::Instant, MALFUNCTION)])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let old = driver.target_named(target)?;
    driver.cast_at(spell, &[old])?;
    for _ in 0..seats {
        if driver.decision()?.player == pid(1) {
            break;
        }
        driver.pass()?;
    }
    let on_stack = on_the_stack(&driver, spell);
    driver.cast_at("Malfunction", &[on_stack])?;
    run(&mut driver, seats)?;
    let landed = match new {
        Some(new) => {
            let new = driver.target_named(new)?;
            let decision = driver.decision()?;
            assert_eq!(decision.player, pid(1), "the player of Malfunction chooses");
            let DecisionKind::ChooseTargets(options) = &decision.kind else {
                panic!("{:?}", decision.kind.name());
            };
            assert!(options.legal.contains(&new), "{:?}", options.legal);
            assert!(!options.legal.contains(&old), "the old target is no choice");
            driver.choose_targets(&[new])?;
            run(&mut driver, seats)?;
            new
        }
        None => old,
    };
    let changed = driver
        .engine()
        .events()
        .iter()
        .any(|e| matches!(e, Event::TargetsChanged(_)));
    assert_eq!(changed, new.is_some());
    assert_eq!(driver.state().stack(), []);
    let Target::Object(landed) = landed else {
        panic!("an object")
    };
    if spell == "Zap" {
        assert_eq!(
            driver
                .state()
                .object(landed.id)
                .unwrap()
                .permanent()
                .unwrap()
                .damage(),
            2
        );
        assert_eq!(
            driver
                .state()
                .object(old.object().unwrap().id)
                .unwrap()
                .permanent()
                .unwrap()
                .damage(),
            0
        );
    } else {
        let aura = driver.state().object(driver.id_of(spell)?).unwrap();
        assert_eq!(
            aura.permanent().unwrap().attached_to().map(|to| to.id),
            Some(landed.id)
        );
        assert_eq!(aura.controller(), pid(0), "the Aura stays its caster's");
    }
    replays(&driver, &setup);
    Ok(())
}
