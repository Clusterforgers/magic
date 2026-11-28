//! Going back a step: a player takes back their passes to an earlier step while nothing irreversible happened since
//! (`docs/engine.md`, Going back). The game is rebuilt from the log before their pass; this module keeps the passes it
//! may return to and judges which events allow that.

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::events::Event;
use crate::take_back::{StepFacts, TakeBackRefusal};
use crate::turn::Step;

/// How many passes the engine keeps to go back to.
const KEPT_PASSES: usize = 64;

/// Why a player cannot go back a step now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Error)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum GoBackRefusal {
    /// The table plays without take-backs (`HouseRules::take_backs`).
    #[error("this table plays without take-backs")]
    Off,
    /// The player passed in no earlier step that is still within reach.
    #[error("there is no earlier step to go back to")]
    Nothing,
    /// A spell or ability is being announced or a question is open mid-action.
    #[error("an action is under way")]
    Busy,
    /// A card was played or an ability activated or triggered since.
    #[error("a card was played or an ability used since")]
    Acted,
    /// Attackers or blockers were declared since.
    #[error("attackers or blockers were declared since")]
    Declared,
    /// A choice was made since.
    #[error("a choice was made since")]
    Chose,
    /// Life, a zone or a permanent changed since.
    #[error("the game changed since")]
    Changed,
    /// A card was drawn since.
    #[error("a card was drawn since")]
    CardDrawn,
    /// A hidden card became known since.
    #[error("a hidden card became known since")]
    CardRevealed,
    /// A library was searched since.
    #[error("a library was searched since")]
    LibrarySearched,
    /// A library was shuffled since.
    #[error("a library was shuffled since")]
    LibraryShuffled,
    /// Something was decided at random since.
    #[error("something random happened since")]
    Random,
    /// Another player made a decision of their own since (the server judges which passes were chosen).
    #[error("another player has acted since")]
    OtherPlayerActed,
    /// The game is over.
    #[error("the game is over")]
    MovedOn,
}

/// The step a player may go back to: the events from `at` on, their pass there included, are undone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GoBackOffer {
    /// Who would go back.
    pub player: PlayerId,
    /// The step they would return to.
    pub step: Step,
    /// Whose turn that step is in.
    pub active: PlayerId,
    /// Where their pass begins in the log.
    pub at: u64,
}

/// An answer a player gave: where it begins in the log, the generator's position there and the step it was given in.
/// A pass at priority is a point to go back to; any other answer only stops going back over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PassPoint {
    pub(crate) player: PlayerId,
    pub(crate) pass: bool,
    pub(crate) from: usize,
    pub(crate) rng: u64,
    pub(crate) turn: u32,
    pub(crate) step: Step,
    pub(crate) active: PlayerId,
}

impl PassPoint {
    /// The offer of going back to this pass.
    pub(crate) fn offer(&self) -> GoBackOffer {
        GoBackOffer {
            player: self.player,
            step: self.step,
            active: self.active,
            at: u64::try_from(self.from).unwrap_or(u64::MAX),
        }
    }
}

/// Where a step was answered: the turn, the step and its active player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Place {
    pub(crate) turn: u32,
    pub(crate) step: Step,
    pub(crate) active: PlayerId,
}

/// The points after `step`, answered at `place`: every pass at priority, and every other answer that logged events.
pub(crate) fn next_passes(
    mut passes: Vec<PassPoint>,
    step: &StepFacts<'_>,
    place: Place,
) -> Vec<PassPoint> {
    let pass = step.priority && step.pass;
    if !pass && step.events.is_empty() {
        return passes;
    }
    passes.push(PassPoint {
        player: step.player,
        pass,
        from: step.mark,
        rng: step.rng,
        turn: place.turn,
        step: place.step,
        active: place.active,
    });
    if passes.len() > KEPT_PASSES {
        passes.remove(0);
    }
    passes
}

/// Why `player` cannot go back over `events` and the answers `after` their pass, if anything stops it. Every
/// irreversible event refuses: only turn-based steps that reveal nothing and decide nothing pass; an answer other than
/// a pass is a choice.
pub(crate) fn refusal_in(
    events: &[Event],
    after: &[PassPoint],
    player: PlayerId,
) -> Option<GoBackRefusal> {
    let mut found = events.iter().filter_map(|event| refusal(event, player));
    let first = found.next();
    // A tap or a zone change comes before the action that caused it: the action names the refusal.
    let named = match first {
        Some(GoBackRefusal::Changed) => found
            .find(|refusal| *refusal != GoBackRefusal::Changed)
            .or(first),
        other => other,
    };
    named.or_else(|| {
            after.iter().find(|point| !point.pass).map(|point| {
                if point.player == player {
                    GoBackRefusal::Chose
                } else {
                    GoBackRefusal::OtherPlayerActed
                }
            })
        })
}

/// The refusal one event makes, if any.
fn refusal(event: &Event, player: PlayerId) -> Option<GoBackRefusal> {
    use GoBackRefusal as G;
    if matches!(
        event,
        Event::PriorityGranted { .. }
            | Event::PriorityPassed { .. }
            | Event::StepBegan { .. }
            | Event::TurnBegan { .. }
            | Event::PermanentUntapped { .. }
            | Event::PermanentPhasedIn { .. }
            | Event::PermanentPhasedOut { .. }
            | Event::ManaPoolEmptied { .. }
            | Event::DamageCleared
            | Event::CombatEnded
            | Event::EffectExpired { .. }
            | Event::RuleExpired { .. }
            | Event::DelayedTriggerExpired { .. }
            | Event::ExertEnded { .. }
            | Event::NothingToDo { .. }
            | Event::WentBack { .. }
    ) {
        return None;
    }
    // A declaration the rules made for a player with nothing to declare decides nothing; one a player answered is a
    // point of its own.
    match event {
        Event::AttackersDeclared { attackers, .. } if attackers.is_empty() => return None,
        Event::BlockersDeclared { blocks, .. } if blocks.is_empty() => return None,
        _ => {}
    }
    let hidden = match crate::take_back::class(event, player) {
        Some(TakeBackRefusal::CardDrawn) => Some(G::CardDrawn),
        Some(TakeBackRefusal::CardRevealed) => Some(G::CardRevealed),
        Some(TakeBackRefusal::LibrarySearched) => Some(G::LibrarySearched),
        Some(TakeBackRefusal::LibraryShuffled) => Some(G::LibraryShuffled),
        Some(TakeBackRefusal::Random) => Some(G::Random),
        _ => None,
    };
    if hidden.is_some() {
        return hidden;
    }
    Some(match event {
        Event::SpellCast { .. }
        | Event::AbilityActivated { .. }
        | Event::ManaAbilityActivated { .. }
        | Event::LandPlayed { .. }
        | Event::AbilityTriggered(_)
        | Event::AbilityPutOnStack(_)
        | Event::TookBack { .. }
        | Event::ManaUndone { .. } => G::Acted,
        Event::AttackersDeclared { .. }
        | Event::AttackersChosen { .. }
        | Event::R5081AttackersChosen { .. }
        | Event::BlockersDeclared { .. }
        | Event::AttackersBlocked { .. } => G::Declared,
        Event::ChoiceMade { .. } | Event::TriggersOrdered { .. } => G::Chose,
        Event::GameEnded { .. } | Event::PlayerLost { .. } => G::MovedOn,
        _ => G::Changed,
    })
}

#[cfg(test)]
#[path = "go_back_tests.rs"]
mod tests;
