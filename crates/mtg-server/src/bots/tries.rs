//! The answers a bot tries in turn for a decision the table refuses: the table's fallback as a seat may give it, then
//! the other legal options the decision names, so conceding is the last resort.

use mtg_engine::choose::{
    CardsChoice, CostsChoice, DivideChoice, ModeChoice, NumberChoice, OptionChoice,
    ReplacementChoice, TargetChoice, TriggersChoice, YesNoChoice,
};
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::{Answer, Decision, DecisionKind};

use crate::pace;

/// The most answers a bot tries for one decision before it concedes.
pub const MOST_TRIES: usize = 24;

/// The table's fallback as a seat may give it: a triggered ability's first legal targets instead of removing it, as
/// only the clock may remove one ([`pace::is_removal`]).
pub fn seat_fallback(decision: &Decision) -> Option<Answer> {
    match (&decision.kind, pace::fallback(decision)) {
        (DecisionKind::ChooseTargets(options), Some(answer)) if pace::is_removal(&answer) => {
            Some(Answer::ChooseTargets(pace::first_targets(options)))
        }
        (_, answer) => answer,
    }
}

/// The answers to try for `decision`, safest first and each once: [`seat_fallback`], then the other options the
/// decision offers, then backing out when it allows it; at most [`MOST_TRIES`].
pub fn tries(decision: &Decision) -> Vec<Answer> {
    let mut all: Vec<Answer> = seat_fallback(decision).into_iter().collect();
    all.extend(others(&decision.kind));
    if decision.kind.can_undo() {
        all.push(Answer::Undo);
    }
    let mut once: Vec<Answer> = Vec::new();
    for answer in all {
        if !pace::is_removal(&answer) && !once.contains(&answer) {
            once.push(answer);
        }
    }
    once.truncate(MOST_TRIES);
    once
}

/// The options of `kind` beyond the fallback, one answer each.
fn others(kind: &DecisionKind) -> Vec<Answer> {
    match kind {
        DecisionKind::Priority(_) => vec![Answer::Priority(PriorityAction::Pass)],
        DecisionKind::ChooseTargets(options) => {
            let mut all = vec![Answer::ChooseTargets(pace::first_targets(options))];
            if options.min <= 1 && options.max >= 1 {
                all.extend(
                    options
                        .c10_dice_choosable()
                        .iter()
                        .map(|target| Answer::ChooseTargets(TargetChoice::Targets(vec![*target]))),
                );
            }
            all
        }
        DecisionKind::OrderTriggers(options) => {
            let order: Vec<usize> = (0..options.triggers.len()).collect();
            let back = order.iter().rev().copied().collect();
            vec![
                Answer::OrderTriggers(TriggersChoice(order)),
                Answer::OrderTriggers(TriggersChoice(back)),
            ]
        }
        DecisionKind::YesNo(_) => [false, true]
            .map(|yes| Answer::YesNo(YesNoChoice(yes)))
            .to_vec(),
        DecisionKind::ChooseReplacement(options) => (0..options.candidates.len())
            .map(|at| Answer::ChooseReplacement(ReplacementChoice(at)))
            .collect(),
        DecisionKind::ChooseOption(options) => (0..options.options.len())
            .map(|at| Answer::ChooseOption(OptionChoice(at)))
            .collect(),
        DecisionKind::ChooseMode(options) => {
            let mut all: Vec<Answer> = options
                .modes
                .iter()
                .map(|mode| Answer::ChooseMode(ModeChoice::Modes(vec![mode.index])))
                .collect();
            if options.can_cancel {
                all.push(Answer::ChooseMode(ModeChoice::Cancel));
            }
            all
        }
        DecisionKind::ChooseNumber(options) => {
            let most = options.max.unwrap_or(options.min.saturating_add(3));
            let mut all: Vec<Answer> = (options.min..=most)
                .filter(|n| !options.except.contains(n))
                .take(4)
                .map(|n| Answer::ChooseNumber(NumberChoice::Number(n)))
                .collect();
            if options.can_cancel {
                all.push(Answer::ChooseNumber(NumberChoice::Cancel));
            }
            all
        }
        DecisionKind::ChooseCosts(options) => {
            let mut all = vec![Answer::ChooseCosts(CostsChoice::Costs(Vec::new()))];
            if options.can_cancel {
                all.push(Answer::ChooseCosts(CostsChoice::Cancel));
            }
            all
        }
        DecisionKind::DivideAmount(options) if options.can_cancel => {
            vec![Answer::DivideAmount(DivideChoice::Cancel)]
        }
        DecisionKind::ChooseCards(options) => {
            let options = options.one_per_group();
            let take = |count: u32| {
                let count = usize::try_from(count).unwrap_or(usize::MAX);
                Answer::ChooseCards(CardsChoice(
                    options.from.iter().copied().take(count).collect(),
                ))
            };
            vec![
                take(options.min),
                take(options.min.max(1)),
                take(options.max),
            ]
        }
        DecisionKind::PayMana(options) => {
            let mut all = Vec::new();
            if options.can_cancel {
                all.push(Answer::PayMana(PayChoice::Cancel));
            }
            if options.auto_available {
                all.push(Answer::PayMana(PayChoice::Auto));
            }
            all
        }
        _ => Vec::new(),
    }
}
