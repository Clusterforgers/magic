//! Mana that sets something off when it is spent to cast a spell ("When that mana is spent to cast a creature spell that
//! shares a creature type with your commander, scry 1").
//!
//! The rider travels in the `only` of the mana (`SpendOn::When`, `SpendOn::Uncounterable`), which it does not restrict. A cast that spends such a mana
//! for a spell of the rider's kind sets up a delayed trigger of the permanent that made the mana, which triggers on the cast
//! of that spell in the same batch of events and puts its ability on the stack above the spell.

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::spend::{SpendOn, WhenSpent};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::decision::PayOptions;
use crate::effects::{ContinuousEffect, Duration, Modification, Selector as Affected};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::rules::{Rule, RuleEffect};
use crate::snapshot::snapshot;
use crate::spend::SpendKind;
use crate::state::GameState;
use crate::triggers::{DelayedCreated, DelayedId};
use crate::zones::Zone;

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
    let mut out: Vec<Event> = riders
        .filter_map(|rider| delayed(state, player, rider, spell.reference()))
        .enumerate()
        .map(|(at, mut created)| {
            created.id = DelayedId(state.next_delayed + u64::try_from(taken + at).unwrap_or(0));
            Event::DelayedTriggerCreated(Box::new(created))
        })
        .collect();
    let shields = uncounterable(state, player, options, events, spell.reference());
    let taken = events
        .iter()
        .filter(|event| matches!(event, Event::RuleCreated { .. }))
        .count()
        + shields.len();
    out.extend(shields);
    out.extend(enters_with(
        state,
        player,
        options,
        events,
        spell.reference(),
        taken,
    ));
    out
}

/// A rule when such mana is spent on a spell of its kind: the spell can't be countered. It begins as the spell is cast,
/// in the same batch, so no player can respond to it first.
fn uncounterable(
    state: &GameState,
    player: PlayerId,
    options: &PayOptions,
    events: &[Event],
    spell: ObjectRef,
) -> Vec<Event> {
    // The cast moves the spell to the stack as a new object (CR 400.7); the rule names it there.
    let spell = ObjectRef {
        counter: spell.counter + 1,
        ..spell
    };
    let taken = events
        .iter()
        .filter(|event| matches!(event, Event::RuleCreated { .. }))
        .count();
    events
        .iter()
        .filter_map(|event| match event {
            Event::ManaSpent { restricted, .. } => Some(restricted),
            _ => None,
        })
        .flatten()
        .flat_map(|unit| SpendOn::uncounterable(&unit.only))
        .filter(|filter| filter.holds(&options.spend.traits))
        .take(1)
        .enumerate()
        .map(|(at, _)| Event::RuleCreated {
            effect: Box::new(RuleEffect {
                id: EffectId(state.next_effect + u64::try_from(taken + at).unwrap_or(0)),
                rule: Rule::CantBeCountered(vec![spell]),
                duration: Duration::Forever,
                source: Some(spell),
                controller: player,
                turn: state.turn().number(),
            }),
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
        until: None,
        effects: rider.effects.clone(),
        targets: Vec::new(),
        bindings: vec![(SPELL.to_owned(), vec![Target::Object(spell)])],
        kept: None,
    })
}

/// A rule for each mana spent on a spell of its rider's kind that changes the permanent the spell becomes ("that creature
/// enters with an additional +1/+1 counter on it", "it gains haste until end of turn"); numbered after the rules `taken`
/// already makes.
fn enters_with(
    state: &GameState,
    player: PlayerId,
    options: &PayOptions,
    events: &[Event],
    spell: ObjectRef,
    taken: usize,
) -> Vec<Event> {
    let casts = r300b_command_casts(state, spell);
    let spell = ObjectRef {
        counter: spell.counter + 1,
        ..spell
    };
    events
        .iter()
        .filter_map(|event| match event {
            Event::ManaSpent { restricted, .. } => Some(restricted),
            _ => None,
        })
        .flatten()
        .flat_map(|unit| SpendOn::enters_bonuses(&unit.only))
        .filter(|bonus| bonus.spell.holds(&options.spend.traits))
        .enumerate()
        .map(|(at, bonus)| Event::RuleCreated {
            effect: Box::new(RuleEffect {
                id: EffectId(state.next_effect + u64::try_from(taken + at).unwrap_or(0)),
                rule: Rule::EntersWith {
                    spell,
                    counters: if bonus.per_command_cast {
                        casts
                    } else {
                        bonus.counters
                    },
                    keyword: bonus.keyword.clone(),
                    this_turn: bonus.this_turn,
                },
                duration: Duration::UntilEndOfTurn,
                source: Some(spell),
                controller: player,
                turn: state.turn().number(),
            }),
        })
        .collect()
}

/// How many times the spell `spell` will have been cast from the command zone once this cast completes: the casts
/// counted so far, and this one while the card still lies in the command zone it is cast from.
fn r300b_command_casts(state: &GameState, spell: ObjectRef) -> u32 {
    let this_cast = state.object(spell.id).is_some_and(|card| {
        matches!(card.zone(), Zone::Command(_)) && state.commanders().any(|id| id == spell.id)
    });
    state
        .commander_casts(spell.id)
        .saturating_add(u32::from(this_cast))
}

/// What the spell `id` gets as it resolves into a permanent from the riders of the mana spent to cast it: its
/// additional +1/+1 counters, proposed with the move (CR 614.1c), and the keywords it gains on the battlefield.
pub(crate) fn entering(state: &GameState, id: ObjectId) -> Vec<Event> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    let on_stack = object.reference();
    let permanent = ObjectRef {
        counter: on_stack.counter + 1,
        ..on_stack
    };
    let mut events = Vec::new();
    let mut counters = 0u32;
    let mut made = 0u64;
    for effect in state.rule_effects() {
        let Rule::EntersWith {
            spell,
            counters: more,
            keyword,
            this_turn,
        } = &effect.rule
        else {
            continue;
        };
        if *spell != on_stack {
            continue;
        }
        counters = counters.saturating_add(*more);
        if let Some(keyword) = keyword {
            let gained = ContinuousEffect::new(
                EffectId(state.next_effect + made),
                Affected::Objects(vec![permanent]),
                Modification::AddKeyword(keyword.clone()),
                if *this_turn {
                    Duration::UntilEndOfTurn
                } else {
                    Duration::Forever
                },
                state.next_timestamp + made,
            )
            .begun(effect.controller, state.turn().number());
            made += 1;
            events.push(Event::EffectCreated {
                effect: gained.into(),
            });
        }
    }
    if counters > 0 {
        events.insert(
            0,
            Event::CountersChanged(CountersChanged {
                on: Target::Object(permanent),
                counter: CounterKind::PlusOnePlusOne,
                delta: i32::try_from(counters).unwrap_or(i32::MAX),
            }),
        );
    }
    events
}
