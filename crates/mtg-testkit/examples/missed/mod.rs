//! The missed-play detector of `bot_tournament` (`MISSED=1`): at every pass of a wrapped seat in its main phase or an
//! opponent's end step it simulates each activatable ability and castable spell to the end of the turn and counts the
//! ones that beat the pass, by kind; it also counts loyalty abilities left unused and tap abilities of ready
//! attackers activated before combat.

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Decision, Engine};
use mtg_testkit::brain::priority::{Window, window};
use mtg_testkit::brain::{Horizon, Search, SeatBrain, Seen, Settings};

/// The counts by kind, over every wrapped seat.
pub static TALLY: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// What a play must gain over passing to count as missed.
const GAIN: f64 = 0.75;

fn bump(key: impl Into<String>) {
    *TALLY.lock().unwrap().entry(key.into()).or_default() += 1;
}

/// A brain whose passes are checked for missed plays.
pub struct Missed {
    /// The brain playing.
    pub inner: Box<dyn SeatBrain>,
    /// Its seat.
    pub seat: PlayerId,
}

impl SeatBrain for Missed {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let answer = self.inner.choose(decision, table);
        if let (Some(engine), DecisionKind::Priority(options)) = (table, &decision.kind)
            && engine.decision() == Some(decision)
            && !engine.announcing()
        {
            check(engine, self.seat, options, &answer);
        }
        answer
    }

    fn set_budget(&mut self, share: f64) {
        self.inner.set_budget(share);
    }
}

fn is_walker(engine: &Engine, id: mtg_core::ids::ObjectId) -> bool {
    mtg_engine::characteristics::characteristics(engine.state(), id)
        .is_some_and(|chars| chars.types.contains(CardType::Planeswalker))
}

fn check(
    engine: &Engine,
    seat: PlayerId,
    options: &mtg_engine::decision::PriorityOptions,
    answer: &Answer,
) {
    let seen = Seen::new(engine.state(), seat);
    let at = window(&seen);
    let step = engine.state().turn().step();
    let name = |id| {
        engine
            .state()
            .object(id)
            .map_or_else(String::new, |o| o.card().name.clone())
    };
    if let Answer::Priority(PriorityAction::Activate(ability)) = answer {
        let source = ability.object;
        bump(format!("activated {at:?}"));
        if step == Step::Main1
            && at == Window::Main
            && seen
                .body(source)
                .is_some_and(|b| b.is_creature() && !b.tapped && b.power() > 0)
            && !mtg_engine::keywords::summoning_sick(engine.state(), source)
        {
            let mut copy = engine.clone();
            if mtg_testkit::bot::apply_answer(
                &mut copy,
                &engine.decision().unwrap().clone(),
                answer.clone(),
            )
            .is_ok()
            {
                let tapped = copy
                    .state()
                    .object(source)
                    .and_then(|o| o.permanent())
                    .is_some_and(|p| p.tapped());
                if tapped {
                    bump("precombat tap of a ready attacker");
                    if std::env::var("MISSED_SHOW").is_ok() {
                        println!("precombat-tap {}", name(source));
                    }
                }
            }
        }
        return;
    }
    if !matches!(answer, Answer::Priority(PriorityAction::Pass)) {
        return;
    }
    if !matches!(at, Window::Main | Window::EndStep) || !engine.state().stack().is_empty() {
        return;
    }
    // Loyalty abilities left unused in the seat's own main phase.
    if at == Window::Main && step == Step::Main2 {
        let walkers: Vec<_> = options
            .abilities
            .iter()
            .filter(|a| is_walker(engine, a.object))
            .map(|a| a.object)
            .collect();
        if !walkers.is_empty() {
            bump("main2 pass with a loyalty ability unused");
            if std::env::var("MISSED_SHOW").is_ok() {
                let loyalty = engine
                    .state()
                    .object(walkers[0])
                    .and_then(|o| o.permanent())
                    .map_or(0, |p| p.counters_of(&CounterKind::Loyalty));
                println!("walker-idle {} loyalty {loyalty}", name(walkers[0]));
            }
        }
    }
    // `MISSED=fast` counts the unused walkers alone, without the simulations.
    if std::env::var("MISSED").is_ok_and(|v| v == "fast") {
        bump(format!("passes {at:?}"));
        return;
    }
    let mut search = Search::new(seat, Settings::NORMAL.untimed(), 0xD7);
    let horizon = Horizon::EndOfTurn;
    let Some(pass) = search.outcome(engine, Answer::Priority(PriorityAction::Pass), horizon) else {
        return;
    };
    let mut plays: Vec<(String, PriorityAction)> = Vec::new();
    for ability in &options.abilities {
        if is_walker(engine, ability.object) {
            continue;
        }
        plays.push(("ability".into(), PriorityAction::Activate(*ability)));
    }
    for id in &options.castable {
        let instant = seen
            .chars(*id)
            .is_some_and(|c| c.types.contains(CardType::Instant));
        if at == Window::EndStep && !instant {
            continue;
        }
        plays.push((
            if instant { "instant" } else { "spell" }.into(),
            PriorityAction::CastSpell(*id),
        ));
    }
    let mut seen_kinds: Vec<String> = Vec::new();
    for (kind, play) in plays {
        let mut search = Search::new(seat, Settings::NORMAL.untimed(), 0xD7);
        let Some(outcome) = search.outcome(engine, Answer::Priority(play.clone()), horizon) else {
            continue;
        };
        if outcome.score > pass.score + GAIN {
            let key = format!("missed {kind} at {at:?} {step:?}");
            if !seen_kinds.contains(&key) {
                bump(&*key);
                seen_kinds.push(key);
            }
            if std::env::var("MISSED_SHOW").is_ok() {
                let object = match &play {
                    PriorityAction::Activate(a) => a.object,
                    PriorityAction::CastSpell(id) => *id,
                    _ => continue,
                };
                println!(
                    "missed {kind} {at:?} {step:?} {} +{:.2}",
                    name(object),
                    outcome.score - pass.score
                );
            }
        }
    }
    bump(format!("passes {at:?}"));
}

/// Prints the counts.
pub fn report() {
    for (key, count) in TALLY.lock().unwrap().iter() {
        println!("missed-tally {key}: {count}");
    }
}
