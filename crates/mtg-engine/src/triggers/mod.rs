//! Triggered abilities (CR 603): detected from the events of a batch, queued in the state and put on the stack in
//! APNAP order, with the delayed ones (CR 603.7) that effects set up.
//!
//! Detection is a function of events and data ([`detect()`]); the queue is state, so a log folds to the same queue
//! and an engine rebuilt from it carries on (`docs/cards-dsl.md`, Triggers).

mod delayed;
mod detect;
mod queue;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::abilities::Cause;
use crate::object::ObjectRef;
use crate::snapshot::Snapshot;

pub use delayed::{Delayed, DelayedCreated, DelayedId};
pub use detect::detect;
pub use queue::PendingTrigger;

pub(crate) use delayed::{fold_created, fold_expired};
pub(crate) use queue::{fold_ordered, fold_removed, fold_triggered, next as sweep, removing, take};

/// A triggered ability that triggered and waits to be put on the stack: what the `AbilityTriggered` event says.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Triggered {
    /// The object with the ability, as it was.
    pub source: ObjectRef,
    /// Its index among the rules of the source's card; for a delayed trigger, the ability that set it up.
    pub ability: usize,
    /// Who controls it: whoever controlled the source as the ability triggered (CR 603.3a).
    pub controller: PlayerId,
    /// What it triggered on.
    pub cause: Cause,
    /// The source as it was, which the ability can still read once the source has gone (CR 603.10a).
    pub snapshot: Snapshot,
    /// The delayed trigger it is one of, for a delayed triggered ability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub delayed: Option<DelayedId>,
}

/// Whether effects set up a delayed trigger anywhere in `effects`.
fn sets_up_delayed(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::DelayedTrigger { .. } => true,
        Effect::If {
            then, otherwise, ..
        } => sets_up_delayed(then) || sets_up_delayed(otherwise),
        Effect::May { effects, .. } => sets_up_delayed(effects),
        Effect::Pay { paid, unpaid, .. } => sets_up_delayed(paid) || sets_up_delayed(unpaid),
        Effect::Modal { modes, .. } => modes.iter().any(|mode| sets_up_delayed(&mode.effects)),
        _ => false,
    })
}

/// Whether `card` can ever trigger something: it has a triggered ability or an effect that sets one up.
pub(crate) fn can_trigger(card: &CardDefinition) -> bool {
    card.rules.iter().any(|rules| {
        matches!(**rules, Ability::Triggered { .. }) || sets_up_delayed(rules.effects())
    })
}
