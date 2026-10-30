//! Whether every play a priority window offers can be taken through to the stack: each cast, activation and special
//! action is begun on a copy of the game and its announcement answered until it is put on the stack (or taken) or
//! rewound with nothing done.

use std::fmt;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Engine, Event};

use crate::bot::{RandomBot, priority_actions};

/// Answers an announcement may take before the probe gives it up as stuck.
const MAX_STEPS: usize = 64;

/// A play the priority window offered that ended with nothing done whichever way it was answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unanswered {
    /// The seat that was offered the play.
    pub player: PlayerId,
    /// The turn number.
    pub turn: u32,
    /// The offered action.
    pub action: PriorityAction,
    /// The name of the card or permanent the action is about.
    pub name: String,
}

impl fmt::Display for Unanswered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} on turn {}: {:?} ({})",
            self.player, self.turn, self.action, self.name
        )
    }
}

/// Whether `action` is a play the announcement check covers: a cast, an activation or a special action.
const fn is_play(action: &PriorityAction) -> bool {
    !matches!(
        action,
        PriorityAction::Pass
            | PriorityAction::PlayLand(_)
            | PriorityAction::PlayLandFace(..)
            | PriorityAction::ActivateMana(_)
            | PriorityAction::Manual(_)
    )
}

/// The plays the priority decision of `engine` offers that cannot be taken: each is tried `tries` times with differently
/// seeded answers, the first paying automatically, and counts as answered once a try commits an event.
pub fn unanswered_plays(engine: &Engine, seed: u64, tries: u64) -> Vec<Unanswered> {
    let Some(decision) = engine.decision() else {
        return Vec::new();
    };
    let DecisionKind::Priority(options) = &decision.kind else {
        return Vec::new();
    };
    priority_actions(options)
        .into_iter()
        .filter(is_play)
        .filter(|action| {
            !(0..tries).any(|attempt| taken(engine, decision.player, action, seed, attempt))
        })
        .map(|action| Unanswered {
            player: decision.player,
            turn: engine.state().turn().number(),
            name: name_of(engine, &action),
            action,
        })
        .collect()
}

/// The name of the object `action` is about.
fn name_of(engine: &Engine, action: &PriorityAction) -> String {
    let object = match action {
        PriorityAction::CastSpell(id)
        | PriorityAction::Suspend(id)
        | PriorityAction::Foretell(id)
        | PriorityAction::TurnFaceUp(id)
        | PriorityAction::CastPrepared(id) => Some(*id),
        PriorityAction::Activate(ability) => Some(ability.object),
        _ => None,
    };
    object.and_then(|id| engine.state().object(id)).map_or_else(
        || "companion".to_owned(),
        |object| object.card().name.clone(),
    )
}

/// Whether `events`, committed by a try, did what was offered: anything but taking the announcement back.
fn did_something(events: &[Event]) -> bool {
    events
        .iter()
        .any(|event| !matches!(event, Event::AnnouncementRewound { .. }))
}

/// Whether one try at `action` on a copy of `engine` gets past its announcement: the announcement is answered by
/// `player`'s seeded bot, paying automatically on the first try and never cancelling.
fn taken(
    engine: &Engine,
    player: PlayerId,
    action: &PriorityAction,
    seed: u64,
    attempt: u64,
) -> bool {
    let mut copy = engine.clone();
    let mark = copy.events().len();
    if copy.apply(Answer::Priority(action.clone())).is_err() {
        return false;
    }
    let stream = u64::try_from(player.index()).unwrap_or(0) + 1;
    let mut bot = RandomBot::new(seed ^ attempt.rotate_left(17), stream);
    for _ in 0..MAX_STEPS {
        if copy.events().len() > mark {
            return did_something(&copy.events()[mark..]);
        }
        let Some(decision) = copy.decision().cloned() else {
            return true;
        };
        if decision.player != player || matches!(decision.kind, DecisionKind::Priority(_)) {
            return false;
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(options) if attempt == 0 || !options.can_cancel => {
                Answer::PayMana(PayChoice::Auto)
            }
            _ => match bot.choose(&decision) {
                Answer::PayMana(PayChoice::Cancel) => Answer::PayMana(PayChoice::Auto),
                answer => answer,
            },
        };
        if copy.apply(answer).is_err() {
            return false;
        }
    }
    did_something(&copy.events()[mark..])
}
