//! Goal: "Your opponents can't cast spells during combat" (Megatron, Tyrant) keeps every opponent from casting in each step of the combat phase and lets them cast outside it, while its controller casts at any time, at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const COMBAT: [Step; 2] = [Step::BeginCombat, Step::EndCombat];

#[rstest]
fn opponents_cast_outside_combat_only(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Megatron, Tyrant // Megatron, Destructive Force").unwrap()),
            ready(mountain()),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.hand(seat, [burn("Shock", 2)]);
        if seat > 0 {
            scenario = scenario.battlefield(seat, [ready(mountain())]);
        }
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let mut seen = Vec::new();
    for _ in 0..400 {
        if driver.state().turn().number() > 1 {
            break;
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::Priority(options) = &decision.kind {
            let step = driver.state().turn().step();
            seen.push((step, decision.player, !options.castable.is_empty()));
        }
        driver.answer_passively()?;
    }
    for (step, player, castable) in &seen {
        let banned = *player != PlayerId(0) && COMBAT.contains(step);
        assert_eq!(*castable, !banned, "{player:?} in {step:?}");
    }
    for step in [Step::Main1, Step::BeginCombat, Step::EndCombat, Step::Main2] {
        assert!(
            seen.iter()
                .any(|(at, player, _)| *at == step && *player != PlayerId(0)),
            "an opponent had priority in {step:?}"
        );
    }
    Ok(())
}
