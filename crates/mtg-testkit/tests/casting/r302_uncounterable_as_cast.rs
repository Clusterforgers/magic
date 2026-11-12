//! Goal: a spell that can't be countered "if you controlled a Dragon as you cast this spell" (a rule from the stack under
//! a condition read as the spell was cast) resists a counter when its caster controlled a Dragon then and is countered
//! when they did not, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{pid, spell};

/// "If you controlled a Dragon as you cast this spell, this spell can't be countered. Draw a card."
fn prerogative() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Rule":{"rule":{"CantBeCountered":"This"},"from":"Stack","condition":{"AsCast":{"Compare":{"left":{"Count":[{"Subtype":"Dragon"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}}"#,
    )
    .unwrap();
    let card = spell("Prerogative", CardType::Instant, "[]", r#"[{"Draw":{}}]"#);
    Arc::new((*card).clone().with_rules(rule))
}

/// "{T}: Counter target spell."
fn negator() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    artifact_with("Negator", vec![ability])
}

#[rstest]
fn a_dragon_controlled_as_the_spell_was_cast_keeps_it_from_being_countered(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] dragon: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(negator())];
    if dragon {
        mine.push(ready(Arc::new(
            (*free_creature("Dragon", 4, 4))
                .clone()
                .with_subtype("Dragon"),
        )));
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![prerogative()])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let before = hand(&driver);
    driver.cast("Prerogative")?;
    let spell = *driver.state().stack().last().unwrap();
    let spell = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.begin_activation("Negator")?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let idle = matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().resolving().is_none();
        if idle {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![spell]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    // The spell left the hand when cast; it drew a card only when it resolved.
    let drew = hand(&driver) + 1 - before;
    assert_eq!(drew, usize::from(dragon), "dragon {dragon}");
    Ok(())
}
