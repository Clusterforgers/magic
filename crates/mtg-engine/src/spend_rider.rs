//! Mana that sets something off when it is spent to cast a spell ("When that mana is spent to cast a creature spell that
//! shares a creature type with your commander, scry 1").
//!
//! The rider travels in the `only` of the mana (`SpendOn::When`), which it does not restrict. A cast that spends such a mana
//! for a spell of the rider's kind sets up a delayed trigger of the permanent that made the mana, which triggers on the cast
//! of that spell in the same batch of events and puts its ability on the stack above the spell.

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::spend::{SpendOn, WhenSpent};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::PlayerId;

use crate::choose::Target;
use crate::decision::PayOptions;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::snapshot::snapshot;
use crate::spend::SpendKind;
use crate::state::GameState;
use crate::triggers::{DelayedCreated, DelayedId};

/// The name by which the effects of a rider read the spell.
const SPELL: &str = "spell";

/// The delayed triggers for the riders of the mana that the payment in `events` spends, one for each such mana: those whose
/// spell filter the spell being cast satisfies. None unless `events` cast `options.spell`.
pub(crate) fn created(
    state: &GameState,
    player: PlayerId,
    options: &PayOptions,
    events: &[Event],
) -> Vec<Event> {
    if options.spend.kind != SpendKind::Spell {
        return Vec::new();
    }
    let cast = events
        .iter()
        .any(|event| matches!(event, Event::SpellCast { object, .. } if *object == options.spell));
    let Some(spell) = state.object(options.spell).filter(|_| cast) else {
        return Vec::new();
    };
    let taken = events
        .iter()
        .filter(|event| matches!(event, Event::DelayedTriggerCreated(_)))
        .count();
    let riders = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaSpent { restricted, .. } => Some(restricted),
            _ => None,
        })
        .flatten()
        .flat_map(|unit| SpendOn::riders(&unit.only))
        .filter(|rider| rider.spell.holds(&options.spend.traits));
    riders
        .filter_map(|rider| delayed(state, player, rider, spell.reference()))
        .enumerate()
        .map(|(at, mut created)| {
            created.id = DelayedId(state.next_delayed + u64::try_from(taken + at).unwrap_or(0));
            Event::DelayedTriggerCreated(Box::new(created))
        })
        .collect()
}

/// The delayed trigger of one rider for the spell `spell` its mana pays for; none when the permanent that made the mana is
/// gone from the game.
fn delayed(
    state: &GameState,
    player: PlayerId,
    rider: &WhenSpent,
    spell: ObjectRef,
) -> Option<DelayedCreated> {
    let source = state.object(rider.source?)?;
    Some(DelayedCreated {
        id: DelayedId(0),
        source: source.reference(),
        snapshot: snapshot(state, source.id())?,
        controller: player,
        ability: 0,
        when: TriggerPattern::SpellCast {
            what: Selector(vec![Pred::SameCard(Ref::Bound(SPELL.to_owned()))]),
            by: Ref::You,
        },
        once: true,
        this_turn: true,
        effects: rider.effects.clone(),
        bindings: vec![(SPELL.to_owned(), vec![Target::Object(spell)])],
    })
}
