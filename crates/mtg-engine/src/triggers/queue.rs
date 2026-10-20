//! The queue of triggered abilities waiting for the stack, and the sweep that puts them there in APNAP order.
//!
//! The queue is state: `AbilityTriggered` joins it and `AbilityPutOnStack` or `TriggerRemoved` leaves it, so a log
//! folds to the same queue and an engine rebuilt from it carries on where it was.

use std::sync::Arc;

use mtg_cards::ability::{Ability, Limit};
use mtg_core::ids::PlayerId;

use super::{DelayedId, Triggered};
use crate::abilities::{AbilityRef, AbilitySource, Cause, PutOnStack};
use crate::ability_reader;
use crate::cast::Pending;
use crate::choose::{TriggerEntry, TriggersOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::flow::Progress;
use crate::fold::invalid;
use crate::object::ObjectRef;
use crate::resolve::Bindings;
use crate::stack::apnap_order;
use crate::state::GameState;
use crate::turn::Step;
use crate::wording;

/// A triggered ability that has triggered and waits to be put on the stack.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PendingTrigger {
    pub(crate) trigger: Triggered,
    pub(crate) rules: Arc<Ability>,
    pub(crate) bindings: Bindings,
    pub(crate) ordered: bool,
    pub(crate) round: u32,
}

impl PendingTrigger {
    /// What triggered, and on what.
    pub const fn trigger(&self) -> &Triggered {
        &self.trigger
    }

    /// The rules it runs once it is on the stack.
    pub fn rules(&self) -> &Ability {
        &self.rules
    }

    /// Whether its controller has put it in order with the others they put on the stack together.
    pub const fn ordered(&self) -> bool {
        self.ordered
    }

    /// Whether this is the trigger named by these fields.
    fn is(
        &self,
        source: ObjectRef,
        origin: AbilitySource,
        ability: usize,
        delayed: Option<DelayedId>,
        cause: &Cause,
    ) -> bool {
        let t = &self.trigger;
        t.source == source
            && t.origin == origin
            && t.ability == ability
            && t.delayed == delayed
            && t.cause == *cause
    }

    /// The words of the event it waits for, such as "When it enters the battlefield"; a keyword's trigger is
    /// introduced by the keyword ("Battle cry: whenever it attacks"), so it is told from the others of its source.
    fn words(&self, state: &GameState) -> String {
        let Ability::Triggered { when, .. } = &*self.rules else {
            return String::new();
        };
        let said = wording::pattern(when);
        let address = AbilityRef {
            object: self.trigger.source.id,
            ability: self.trigger.ability,
            source: self.trigger.origin,
        };
        match ability_reader::keyword_adding(state, address) {
            Some(keyword) => wording::keyword_trigger(keyword.name(), &said),
            None => said,
        }
    }
}

/// Queues a trigger that triggered, and uses up what limits it.
pub(crate) fn fold_triggered(state: &mut GameState, t: &Triggered) -> Result<(), EngineError> {
    let (rules, bindings) = match t.delayed {
        None => {
            if state.source_object(t.source.id).is_none() {
                return Err(invalid(format!("{:?} triggered and is gone", t.source)));
            }
            let address = AbilityRef {
                object: t.source.id,
                ability: t.ability,
                source: t.origin,
            };
            let rules = ability_reader::ability(state, address)
                .ok_or_else(|| invalid(format!("{:?} has no ability {}", t.source, t.ability)))?;
            (rules, Bindings::new())
        }
        Some(id) => {
            let delayed = state
                .delayed
                .iter()
                .find(|d| d.id == id)
                .ok_or_else(|| invalid(format!("{id:?} is not waiting")))?;
            (Arc::clone(&delayed.rules), delayed.bindings.clone())
        }
    };
    let Ability::Triggered { limit, .. } = &*rules else {
        return Err(invalid(format!(
            "ability {} of {:?} is not a triggered ability",
            t.ability, t.source
        )));
    };
    if limit == &Some(Limit::OncePerTurn)
        && !state.limits_spent.insert((t.source, t.origin, t.ability))
    {
        return Err(invalid(format!(
            "ability {} of {:?} already triggered this turn",
            t.ability, t.source
        )));
    }
    if let Some(id) = t.delayed {
        // A delayed trigger that triggers once is gone the moment it has.
        state.delayed.retain(|d| d.id != id || !d.once);
    }
    state.pending_triggers.push(PendingTrigger {
        trigger: t.clone(),
        rules,
        bindings,
        ordered: false,
        round: state.trigger_round,
    });
    Ok(())
}

/// The queue positions of the triggers `player` puts on the stack next, in queue order.
///
/// Triggers that triggered while others were being put on the stack wait for the next round (CR 603.3b).
pub(crate) fn group(state: &GameState, player: PlayerId) -> Vec<usize> {
    let Some(round) = state.pending_triggers.iter().map(|e| e.round).min() else {
        return Vec::new();
    };
    state
        .pending_triggers
        .iter()
        .enumerate()
        .filter(|(_, e)| e.round == round && e.trigger.controller == player)
        .map(|(at, _)| at)
        .collect()
}

/// Puts the next group of `player` in the order they chose: `order[k]` is the position, within the group, of the
/// trigger that goes on the stack k-th.
pub(crate) fn fold_ordered(
    state: &mut GameState,
    player: PlayerId,
    order: &[usize],
) -> Result<(), EngineError> {
    let slots = group(state, player);
    let mut seen = vec![false; slots.len()];
    let permutation = order.len() == slots.len()
        && order.iter().all(|at| {
            seen.get_mut(*at)
                .is_some_and(|s| !std::mem::replace(s, true))
        });
    if !permutation {
        return Err(invalid(format!(
            "{order:?} does not put the {} triggers of {player:?} in order",
            slots.len()
        )));
    }
    let waiting: Vec<PendingTrigger> = slots
        .iter()
        .map(|at| state.pending_triggers[*at].clone())
        .collect();
    for (slot, from) in slots.into_iter().zip(order) {
        let mut entry = waiting[*from].clone();
        entry.ordered = true;
        state.pending_triggers[slot] = entry;
    }
    Ok(())
}

/// Takes the trigger a `PutOnStack` names out of the queue.
pub(crate) fn take(state: &mut GameState, put: &PutOnStack) -> Result<PendingTrigger, EngineError> {
    let cause = put
        .cause
        .as_ref()
        .ok_or_else(|| invalid("a trigger is put on the stack with the event that caused it"))?;
    let at = state
        .pending_triggers
        .iter()
        .position(|e| e.is(put.source, put.origin, put.ability, put.delayed, cause))
        .ok_or_else(|| invalid(format!("no trigger of {:?} is waiting", put.source)))?;
    // Whatever triggers from here on waits for the next round.
    state.trigger_round += 1;
    Ok(state.pending_triggers.remove(at))
}

/// Takes a trigger out of the queue without putting it on the stack (CR 603.3d).
pub(crate) fn fold_removed(
    state: &mut GameState,
    source: ObjectRef,
    origin: AbilitySource,
    ability: usize,
    delayed: Option<DelayedId>,
    cause: &Cause,
) -> Result<(), EngineError> {
    let at = state
        .pending_triggers
        .iter()
        .position(|e| e.is(source, origin, ability, delayed, cause))
        .ok_or_else(|| invalid(format!("no trigger of {source:?} is waiting")))?;
    state.pending_triggers.remove(at);
    Ok(())
}

/// The event that takes `entry` out of the queue without putting it on the stack.
pub(crate) fn removing(entry: &PendingTrigger) -> Event {
    let t = &entry.trigger;
    Event::TriggerRemoved {
        source: t.source,
        ability: t.ability,
        origin: t.origin,
        delayed: t.delayed,
        cause: t.cause.clone(),
    }
}

/// Whether the order the controller puts `group` on the stack in can matter.
///
/// Triggers of one source that run the same rules do the same whichever goes first: those of one ability, and the
/// instances of a keyword (two ward, two prowess). Every other group is the controller's to order, once.
fn order_matters(state: &GameState, group: &[usize]) -> bool {
    let entries = || group.iter().map(|at| &state.pending_triggers[*at]);
    let first = entries().next();
    group.len() >= 2
        && entries().any(|e| !e.ordered)
        && entries().any(|e| {
            first.is_some_and(|first| {
                let same_ability = (e.trigger.origin, e.trigger.ability)
                    == (first.trigger.origin, first.trigger.ability);
                e.trigger.source.id != first.trigger.source.id
                    || e.trigger.delayed != first.trigger.delayed
                    || !(same_ability || e.rules() == first.rules())
            })
        })
}

/// What the engine does next about the triggers waiting for the stack, if any are.
///
/// Called where a player would receive priority, after state-based actions (CR 117.5, 603.3): the active player's
/// triggers go on the stack, then each other player's in turn order (CR 101.4), one at a time with its targets
/// chosen as it goes on (CR 603.3d), until none waits.
pub(crate) fn next(state: &GameState) -> Option<Progress> {
    if state.pending_triggers.is_empty() {
        return None;
    }
    // A trigger of a player who left the game, or of an object that left with them, is gone.
    if let Some(gone) = state.pending_triggers.iter().find(|e| {
        !state.is_alive(e.trigger.controller) || state.source_object(e.trigger.source.id).is_none()
    }) {
        return Some(Progress::Events(vec![removing(gone)]));
    }
    // Nobody receives priority in the untap step, so what triggers there waits for the upkeep (CR 502.4).
    if state.turn().step() == Step::Untap {
        return None;
    }
    for player in apnap_order(state) {
        let group = group(state, player);
        let Some(first) = group.first() else {
            continue;
        };
        if order_matters(state, &group) {
            let triggers = group
                .iter()
                .map(|at| {
                    let e = &state.pending_triggers[*at];
                    TriggerEntry {
                        source: e.trigger.source,
                        ability: e.trigger.ability,
                        when: e.words(state),
                    }
                })
                .collect();
            return Some(Progress::Decide(Decision {
                player,
                kind: DecisionKind::OrderTriggers(TriggersOptions { triggers }),
            }));
        }
        return Some(Progress::Put(Pending::trigger(
            player,
            state.pending_triggers[*first].clone(),
        )));
    }
    None
}

#[cfg(test)]
mod tests {
    //! Goal: the queue puts a player's triggers in the order they chose and marks them ordered, takes one out as it goes on the stack or is removed, groups them by round so what triggers during a sweep waits for the next, asks for an order only for two or more triggers of different sources or abilities, and refuses what is not a permutation.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::effect::Effect;
    use mtg_cards::refs::{Ref, Selector, ZoneKind};
    use mtg_cards::trigger::TriggerPattern;
    use mtg_cards::value::Amount;
    use mtg_core::card::CardType;
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// "Whenever this attacks, you gain `life` life."
    fn watching(life: i32) -> Ability {
        Ability::Triggered {
            when: TriggerPattern::Attacks {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::GainLife {
                who: Ref::You,
                amount: Amount::Fixed(life),
            }],
        }
    }

    /// Seat 0 controls Alpha (two triggered abilities), Beta (one); seat 1 controls Gamma (one).
    fn table() -> GameState {
        let card = |name: &str, abilities: usize| {
            let mut card = CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1);
            // The abilities of one card differ, as those of two sources do not need to.
            for life in 1..=abilities {
                card = card.with_rules(watching(i32::try_from(life).unwrap()));
            }
            Arc::new(card)
        };
        state(
            vec![vec![], vec![]],
            vec![
                vec![card("Alpha", 2), card("Beta", 1), same_twice("Delta")],
                vec![card("Gamma", 1)],
            ],
        )
    }

    /// A card with the very same ability printed twice: two instances of one keyword.
    fn same_twice(name: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(watching(1))
                .with_rules(watching(1)),
        )
    }

    fn id(state: &GameState, name: &str) -> ObjectId {
        state
            .objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    fn trigger_of(state: &GameState, name: &str, ability: usize, subject: PlayerId) -> Triggered {
        let object = state.object(id(state, name)).unwrap();
        Triggered {
            origin: crate::abilities::AbilitySource::default(),
            source: object.reference(),
            ability,
            controller: object.controller(),
            cause: Cause {
                subjects: vec![crate::choose::Target::Player(subject)],
                snapshots: Vec::new(),
            },
            snapshot: snapshot(state, object.id()).unwrap(),
            delayed: None,
        }
    }

    fn queue(state: &mut GameState, triggers: &[(&str, usize)]) {
        for (name, ability) in triggers {
            let t = trigger_of(state, name, *ability, ME);
            fold_triggered(state, &t).unwrap();
        }
    }

    fn waiting(state: &GameState) -> Vec<(String, usize)> {
        state
            .pending_triggers()
            .iter()
            .map(|w| {
                let name = state
                    .object(w.trigger().source.id)
                    .unwrap()
                    .card()
                    .name
                    .clone();
                (name, w.trigger().ability)
            })
            .collect()
    }

    #[test]
    fn a_trigger_joins_the_queue_in_the_order_it_triggered() {
        let mut state = table();
        queue(&mut state, &[("Beta", 0), ("Gamma", 0), ("Alpha", 1)]);
        assert_eq!(
            waiting(&state),
            [
                ("Beta".to_owned(), 0),
                ("Gamma".to_owned(), 0),
                ("Alpha".to_owned(), 1)
            ]
        );
        assert!(state.pending_triggers().iter().all(|w| !w.ordered()));
    }

    #[test]
    fn ordering_puts_only_that_players_triggers_in_the_chosen_order_and_marks_them() {
        let mut state = table();
        queue(
            &mut state,
            &[("Alpha", 0), ("Gamma", 0), ("Beta", 0), ("Alpha", 1)],
        );
        assert_eq!(group(&state, ME), [0, 2, 3]);
        fold_ordered(&mut state, ME, &[2, 0, 1]).unwrap();
        assert_eq!(
            waiting(&state),
            [
                ("Alpha".to_owned(), 1),
                ("Gamma".to_owned(), 0),
                ("Alpha".to_owned(), 0),
                ("Beta".to_owned(), 0)
            ],
            "seat 1's trigger stays where it was; the others fill the places seat 0's held"
        );
        let ordered: Vec<bool> = state
            .pending_triggers()
            .iter()
            .map(PendingTrigger::ordered)
            .collect();
        assert_eq!(ordered, [true, false, true, true]);
    }

    #[rstest]
    #[case::too_few(&[0, 1])]
    #[case::too_many(&[0, 1, 2, 0])]
    #[case::a_repeat(&[0, 0, 1])]
    #[case::one_that_is_not_there(&[0, 1, 3])]
    fn an_order_that_is_not_a_permutation_does_not_fold(#[case] order: &[usize]) {
        let mut state = table();
        queue(&mut state, &[("Alpha", 0), ("Alpha", 1), ("Beta", 0)]);
        let before = waiting(&state);
        assert!(fold_ordered(&mut state, ME, order).is_err());
        assert_eq!(waiting(&state), before, "a refused order changes nothing");
        assert!(state.pending_triggers().iter().all(|w| !w.ordered()));
    }

    #[test]
    fn a_trigger_leaves_the_queue_as_it_goes_on_the_stack_and_what_triggers_later_waits_for_the_next_round()
     {
        let mut state = table();
        queue(&mut state, &[("Alpha", 0), ("Beta", 0)]);
        let first = state.pending_triggers()[0].trigger().clone();
        let put = crate::abilities::PutOnStack {
            origin: crate::abilities::AbilitySource::default(),
            object: state.next_object(),
            source: first.source,
            snapshot: first.snapshot.clone(),
            ability: first.ability,
            controller: ME,
            choices: crate::choose::Choices::default(),
            cause: Some(first.cause.clone()),
            delayed: None,
        };
        apply_event(&mut state, &Event::AbilityPutOnStack(Box::new(put))).unwrap();
        assert_eq!(waiting(&state), [("Beta".to_owned(), 0)]);
        // A trigger that arrives now is of the next round: it is not in the group of the first.
        let late = trigger_of(&state, "Gamma", 0, ME);
        fold_triggered(&mut state, &late).unwrap();
        assert_eq!(group(&state, ME), [0], "Beta alone");
        assert_eq!(
            group(&state, FOE),
            Vec::<usize>::new(),
            "Gamma waits for its round"
        );
        let beta = state.pending_triggers()[0].trigger().clone();
        fold_removed(
            &mut state,
            beta.source,
            beta.origin,
            beta.ability,
            None,
            &beta.cause,
        )
        .unwrap();
        assert_eq!(group(&state, FOE), [0], "now it is Gamma's round");
        assert!(
            fold_removed(
                &mut state,
                beta.source,
                beta.origin,
                beta.ability,
                None,
                &beta.cause
            )
            .is_err(),
            "it is gone"
        );
    }

    #[rstest]
    #[case::one_trigger(&[("Alpha", 0)], false)]
    #[case::one_source_and_ability_twice(&[("Beta", 0), ("Beta", 0)], false)]
    #[case::two_abilities_of_a_source(&[("Alpha", 0), ("Alpha", 1)], true)]
    #[case::two_instances_of_one_ability_under_two_names(&[("Delta", 0), ("Delta", 1)], false)]
    #[case::two_sources(&[("Alpha", 0), ("Beta", 0)], true)]
    #[case::three_of_which_two_are_alike(&[("Beta", 0), ("Beta", 0), ("Alpha", 0)], true)]
    fn the_order_is_asked_for_only_when_it_can_matter(
        #[case] triggers: &[(&str, usize)],
        #[case] asked: bool,
    ) {
        let mut state = table();
        queue(&mut state, triggers);
        match next(&state) {
            Some(Progress::Decide(decision)) => {
                assert!(asked);
                assert_eq!(decision.player, ME);
                assert!(matches!(decision.kind, DecisionKind::OrderTriggers(_)));
            }
            Some(Progress::Put(_)) => assert!(!asked),
            _ => panic!("the sweep has something to do"),
        }
    }

    #[test]
    fn once_ordered_a_group_is_not_asked_again_and_goes_on_the_stack_one_by_one() {
        let mut state = table();
        queue(&mut state, &[("Alpha", 0), ("Beta", 0)]);
        fold_ordered(&mut state, ME, &[1, 0]).unwrap();
        let Some(Progress::Put(pending)) = next(&state) else {
            panic!("the first of the group goes on the stack");
        };
        let crate::cast::What::Trigger(first) = pending.what else {
            panic!("a trigger");
        };
        assert_eq!(first.trigger().source.id, id(&state, "Beta"));
    }

    #[test]
    fn triggers_of_the_active_player_come_before_the_others_whatever_the_queue_order() {
        let mut state = table();
        let late = trigger_of(&state, "Alpha", 0, ME);
        let early = trigger_of(&state, "Gamma", 0, ME);
        fold_triggered(&mut state, &early).unwrap();
        fold_triggered(&mut state, &late).unwrap();
        let Some(Progress::Put(pending)) = next(&state) else {
            panic!("a trigger goes on the stack");
        };
        assert_eq!(pending.player, ME, "seat 0 is the active player");
    }

    #[test]
    fn nothing_is_put_on_the_stack_in_the_untap_step_and_a_dead_controllers_trigger_is_dropped() {
        let mut state = table();
        queue(&mut state, &[("Beta", 0)]);
        apply_event(
            &mut state,
            &Event::StepBegan {
                step: crate::turn::Step::Untap,
            },
        )
        .unwrap();
        assert!(
            next(&state).is_none(),
            "nobody has priority in the untap step"
        );
        apply_event(
            &mut state,
            &Event::StepBegan {
                step: crate::turn::Step::Upkeep,
            },
        )
        .unwrap();
        assert!(matches!(next(&state), Some(Progress::Put(_))));
        apply_event(
            &mut state,
            &Event::PlayerLost {
                player: ME,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        match next(&state) {
            Some(Progress::Events(events)) => {
                assert!(matches!(events[..], [Event::TriggerRemoved { .. }]));
            }
            other => panic!("{other:?}"),
        }
    }
}
