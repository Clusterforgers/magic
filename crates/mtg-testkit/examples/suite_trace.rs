//! Plays one case of the decision-quality suite and prints each answer seat 0 gives: `suite_trace <case> [level]`.
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::{Decision, Engine};
use mtg_testkit::brain::suite::{cases, run};
use mtg_testkit::brain::{Level, SeatBrain, brain};

/// A brain that prints its answers.
struct Traced(Box<dyn SeatBrain>);

impl SeatBrain for Traced {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let answer = self.0.choose(decision, table);
        if let Some(engine) = table {
            let state = engine.state();
            println!(
                "t{} {:?} {} -> {answer:?} (stack {})",
                state.turn().number(),
                state.turn().step(),
                decision.kind.name(),
                state.stack().len()
            );
            if let Answer::Priority(PriorityAction::CastSpell(id)) = &answer
                && let Some(object) = state.object(*id)
            {
                println!("   casts {}", object.card().name);
            }
        }
        answer
    }
}

fn main() {
    let name = std::env::args().nth(1).expect("a case name");
    let level = match std::env::args().nth(2).as_deref() {
        Some("easy") => Level::Easy,
        Some("hard") => Level::Hard,
        Some("veryhard") => Level::VeryHard,
        _ => Level::Normal,
    };
    let case = cases()
        .into_iter()
        .find(|case| case.name == name)
        .expect("no such case");
    let mut traced = Traced(brain(level, 7, PlayerId::from_index(0), false));
    let verdict = run(&case, &mut traced).unwrap();
    println!("passed: {}", verdict.passed);
}
