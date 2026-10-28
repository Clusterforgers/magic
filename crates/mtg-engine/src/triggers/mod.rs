//! Triggered abilities (CR 603): detected from the events of a batch, queued in the state and put on the stack in
//! APNAP order, with the delayed ones (CR 603.7) that effects set up.
//!
//! Detection is a function of events and data ([`detect()`]); the queue is state, so a log folds to the same queue
//! and an engine rebuilt from it carries on (`docs/cards-dsl.md`, Triggers).

mod delayed;
mod detect;
#[cfg(test)]
mod detect_batches;
#[cfg(test)]
mod detect_crewed;
#[cfg(test)]
mod detect_events;
#[cfg(test)]
mod detect_except;
#[cfg(test)]
mod detect_nth;
#[cfg(test)]
mod detect_turn_nth;
mod queue;
mod reads_x;
mod state;
#[cfg(test)]
mod w2e_detect;
#[cfg(test)]
mod w9a_expend;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::abilities::{AbilitySource, Cause};
use crate::ability_reader::definition_triggers;
use crate::events::Event;
use crate::keywords;
use crate::object::ObjectRef;
use crate::snapshot::Snapshot;

pub use delayed::{Delayed, DelayedCreated, DelayedId};
pub use detect::detect;
pub(crate) use detect::{needs_before, step_matches};
pub use queue::PendingTrigger;
pub(crate) use reads_x::read as x_read;

pub(crate) use delayed::{fold_created, fold_expired};
pub(crate) use queue::{fold_ordered, fold_removed, fold_triggered, next as sweep, removing, take};
pub(crate) use state::detect as state_triggers;

/// A triggered ability that triggered and waits to be put on the stack: what the `AbilityTriggered` event says.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Triggered {
    /// The object with the ability, as it was.
    pub source: ObjectRef,
    /// Its index among the rules of the source's card; for a delayed trigger, the ability that set it up; 0 for a
    /// granted ability; its place among those its keyword adds for one a keyword added.
    pub ability: usize,
    /// Where the ability comes from: the card, an effect that granted it, or a keyword.
    #[serde(default, skip_serializing_if = "AbilitySource::is_printed")]
    #[cfg_attr(feature = "ts", ts(as = "Option<AbilitySource>", optional))]
    pub origin: AbilitySource,
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

/// Whether anything in `effects` can make something trigger: it sets up a delayed trigger or gives an ability that
/// triggers.
fn effects_trigger(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::DelayedTrigger { .. } => true,
        Effect::Continuous { mods, .. } => grants_trigger(mods),
        Effect::If {
            then, otherwise, ..
        } => effects_trigger(then) || effects_trigger(otherwise),
        Effect::May { effects, .. } | Effect::RepeatEach { effects, .. } => {
            effects_trigger(effects)
        }
        Effect::RollDice { results, .. } => results.iter().any(|row| effects_trigger(&row.effects)),
        Effect::FlipCoin { win, lose, .. }
        | Effect::VillainousChoice {
            first: win,
            second: lose,
            ..
        } => effects_trigger(win) || effects_trigger(lose),
        Effect::Pay { paid, unpaid, .. } => effects_trigger(paid) || effects_trigger(unpaid),
        Effect::CastFromZone { otherwise, .. } => effects_trigger(otherwise),
        Effect::Modal { modes, .. } => modes.iter().any(|mode| effects_trigger(&mode.effects)),
        _ => false,
    })
}

/// Whether one of the modifications gives an ability that can make something trigger: a triggered ability, or a
/// keyword that adds one.
fn grants_trigger(mods: &[Modification]) -> bool {
    mods.iter().any(|modification| match modification {
        Modification::GrantAbility(granted) => triggers_something(granted),
        Modification::AddKeyword(keyword) => keyword_triggers(keyword),
        _ => false,
    })
}

/// Whether the keyword adds an ability that can make something trigger (ward, prowess, battle cry).
fn keyword_triggers(keyword: &Keyword) -> bool {
    keywords::rules(keyword)
        .adds_abilities(keyword)
        .iter()
        .any(triggers_something)
}

/// Whether an ability can make something trigger: it is a triggered ability, or sets up a delayed one, or gives an
/// ability that triggers, or is a keyword that adds one.
pub(crate) fn triggers_something(ability: &Ability) -> bool {
    match ability {
        Ability::Triggered { .. } => true,
        Ability::Keyword(keyword) => keyword_triggers(keyword),
        Ability::Static { mods, .. } => grants_trigger(mods),
        other => effects_trigger(other.effects()),
    }
}

/// Whether an ability is a state trigger (CR 603.8) or gives one, as an ability or through the effects of a spell.
pub(crate) fn has_state_trigger(ability: &Ability) -> bool {
    match ability {
        Ability::Triggered {
            when: TriggerPattern::State { .. },
            ..
        } => true,
        Ability::Static { mods, .. } => grants_state_trigger(mods),
        other => effects_state_trigger(other.effects()),
    }
}

/// Whether one of the modifications gives a state trigger.
fn grants_state_trigger(mods: &[Modification]) -> bool {
    mods.iter().any(|modification| {
        matches!(modification, Modification::GrantAbility(granted) if has_state_trigger(granted))
    })
}

/// Whether anything in `effects` gives a state trigger.
fn effects_state_trigger(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::Continuous { mods, .. } => grants_state_trigger(mods),
        Effect::If {
            then, otherwise, ..
        } => effects_state_trigger(then) || effects_state_trigger(otherwise),
        Effect::May { effects, .. } | Effect::RepeatEach { effects, .. } => {
            effects_state_trigger(effects)
        }
        Effect::RollDice { results, .. } => results
            .iter()
            .any(|row| effects_state_trigger(&row.effects)),
        Effect::FlipCoin { win, lose, .. }
        | Effect::VillainousChoice {
            first: win,
            second: lose,
            ..
        } => effects_state_trigger(win) || effects_state_trigger(lose),
        Effect::Pay { paid, unpaid, .. } => {
            effects_state_trigger(paid) || effects_state_trigger(unpaid)
        }
        Effect::CastFromZone { otherwise, .. } => effects_state_trigger(otherwise),
        Effect::Modal { modes, .. } => modes
            .iter()
            .any(|mode| effects_state_trigger(&mode.effects)),
        _ => false,
    })
}

/// Whether the events bring in something that can trigger where nothing could before: a token with a triggered
/// ability, an effect that gives one. The state notes it as it folds them, and this tells `commit` to look for what
/// the batch itself triggers.
pub(crate) fn introduces(events: &[Event]) -> bool {
    events.iter().any(|event| match event {
        Event::TokenCreated(created) => definition_triggers(&created.definition),
        Event::EffectCreated { effect } => match &effect.modification {
            crate::effects::Modification::GrantAbility(granted) => triggers_something(granted),
            crate::effects::Modification::AddKeyword(keyword) => keyword_triggers(keyword),
            _ => false,
        },
        _ => false,
    })
}
