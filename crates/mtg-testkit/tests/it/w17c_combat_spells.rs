//! Goal: a spell cast only during combat (also only on an opponent's turn) is offered in the steps of combat and at no other moment, at tables of two, three and four seats; it is refused outside combat, cast after blockers are declared resolves, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const COMBAT: &str = r#""InCombat""#;
const THEIR_COMBAT: &str = r#"{"All":["InCombat",{"Not":"YourTurn"}]}"#;

/// A free instant that draws a card, cast only while `condition` (data) holds.
fn w17c_trick(condition: &str) -> Arc<CardDefinition> {
    let only: Ability =
        serde_json::from_str(&format!(r#"{{"CastOnly":{{"condition":{condition}}}}}"#)).unwrap();
    let spell: Ability = serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Combat Trick")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(only)
            .with_rules(spell),
    )
}

/// Seat 0 holds the trick; every seat has a creature and a library.
fn w17c_table(seats: usize, condition: &str) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .seed(17)
            .hand(0, [w17c_trick(condition)]),
        |scenario, seat| {
            scenario
                .battlefield(seat, [ready(free_creature("Bear", 2, 2))])
                .library(seat, (0..30).map(|_| forest()))
        },
    )
}

fn w17c_in_combat(step: Step) -> bool {
    matches!(
        step,
        Step::BeginCombat
            | Step::DeclareAttackers
            | Step::DeclareBlockers
            | Step::FirstStrikeDamage
            | Step::CombatDamage
            | Step::EndCombat
    )
}

#[rstest]
#[case::during_combat(COMBAT, false)]
#[case::during_combat_on_an_opponents_turn(THEIR_COMBAT, true)]
fn w17c_the_spell_is_offered_only_in_combat(
    #[values(2, 3, 4)] seats: usize,
    #[case] condition: &str,
    #[case] theirs: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(w17c_table(seats, condition))?;
    let trick = driver.state().zone(Zone::Hand(pid(0)))[0];
    let mut seen = Vec::new();
    for _ in 0..4000 {
        if driver.state().turn().number() > 2 * u32::try_from(seats).unwrap() + 1 {
            break;
        }
        let decision = driver.decision()?.clone();
        if let (true, DecisionKind::Priority(options)) = (decision.player == pid(0), &decision.kind)
        {
            let step = driver.state().turn().step();
            let mine = driver.state().turn().active() == pid(0);
            seen.push((step, mine, options.castable.contains(&trick)));
        }
        driver.answer(mtg_testkit::Driver::passive_answer(&decision))?;
    }
    assert!(
        seen.iter()
            .any(|(step, mine, _)| w17c_in_combat(*step) && !mine)
    );
    for (step, mine, offered) in seen {
        let open = w17c_in_combat(step) && !(theirs && mine);
        assert_eq!(offered, open, "{step:?} mine {mine}");
    }
    Ok(())
}

#[rstest]
fn w17c_the_spell_is_refused_before_combat_and_cast_after_blockers(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(w17c_table(seats, THEIR_COMBAT).at(2, 1, Step::Main1))?;
    let trick = driver.state().zone(Zone::Hand(pid(0)))[0];
    while driver.decision()?.player != pid(1) {
        driver.pass()?;
    }
    // Seat 0 gets priority in the opponent's main phase after the seats before it pass; a cast there is refused.
    driver.pass()?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert_eq!(driver.state().turn().step(), Step::Main1);
    assert!(
        driver
            .answer(Answer::Priority(PriorityAction::CastSpell(trick)))
            .is_err()
    );
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 0)])?;
    while !matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
        driver.pass()?;
    }
    driver.block(&[])?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert_eq!(driver.state().turn().step(), Step::DeclareBlockers);
    driver.cast("Combat Trick")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), 1);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 1);
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}
