//! The queue of triggered abilities waiting for the stack, and the sweep that puts them there in APNAP order.
//!
//! The queue is state: `AbilityTriggered` joins it and `AbilityPutOnStack` or `TriggerRemoved` leaves it, so a log
//! folds to the same queue and an engine rebuilt from it carries on where it was.

use std::sync::Arc;

use mtg_cards::ability::{Ability, Limit};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{DelayedId, Triggered};
use crate::abilities::{AbilityRef, AbilitySource, Cause, PutOnStack};
use crate::ability_reader;
use crate::announce::Pending;
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
        match ability_reader::keyword_adding_as_it_was(state, address, &self.trigger.snapshot) {
            Some(keyword) => wording::keyword_trigger(&keyword.shown_name(), &said),
            None => said,
        }
    }

    /// What it does in the words of its card, the source by its name.
    fn effect(&self, state: &GameState) -> String {
        let name = state
            .source_object(self.trigger.source.id)
            .filter(|object| object.face_down().is_none())
            .map_or("this permanent", |object| object.card().name.as_str());
        effect_words(&self.rules, name)
    }

    /// The object whose event triggered it, while it lies where every player sees it.
    fn cause(&self, state: &GameState) -> Option<ObjectId> {
        public_cause(state, &self.trigger.cause)
    }
}

/// The first object `cause` is about that lies face up on the battlefield or on the stack, where every player sees it.
pub fn public_cause(state: &GameState, cause: &Cause) -> Option<ObjectId> {
    cause
        .subjects
        .iter()
        .filter_map(|subject| subject.object())
        .chain(cause.source)
        .find_map(|subject| {
            let object = state.object(subject.id)?;
            let open = matches!(
                object.zone(),
                crate::zones::Zone::Battlefield | crate::zones::Zone::Stack
            ) && object.face_down().is_none()
                && object.counter() == subject.counter;
            open.then_some(subject.id)
        })
}

/// The triggered abilities one player put on the stack together, in the order put: the last one resolves first.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggerRun {
    pub(crate) round: u32,
    pub(crate) controller: PlayerId,
    pub(crate) members: Vec<RunMember>,
}

/// One triggered ability of a [`TriggerRun`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunMember {
    /// The ability's object on the stack; it is gone once the ability has resolved or left the stack.
    pub object: ObjectId,
    /// The object the ability came from, as it was.
    pub source: ObjectRef,
}

impl TriggerRun {
    /// A number that tells this run from every other run on the stack.
    pub const fn id(&self) -> u32 {
        self.round
    }

    /// Who controls its abilities.
    pub const fn controller(&self) -> PlayerId {
        self.controller
    }

    /// Its abilities in the order they resolve: the one put on the stack last comes first.
    pub fn resolving_order(&self) -> impl DoubleEndedIterator<Item = RunMember> + '_ {
        self.members.iter().rev().copied()
    }
}

/// Adds `member` to the run of `round` and `controller`, and forgets the runs that have left the stack.
fn join_run(state: &mut GameState, round: u32, controller: PlayerId, member: RunMember) {
    let stack = state.zones.get(crate::zones::Zone::Stack).to_vec();
    state
        .trigger_runs
        .retain(|run| run.members.iter().any(|m| stack.contains(&m.object)));
    match state
        .trigger_runs
        .iter_mut()
        .find(|run| run.round == round && run.controller == controller)
    {
        Some(run) => run.members.push(member),
        None => state.trigger_runs.push(TriggerRun {
            round,
            controller,
            members: vec![member],
        }),
    }
}

/// What a triggered ability does in the words of its card, without the event it waits for: "You may pay {1}. If you
/// do, draw a card." The whole text for any other ability; empty for one the card writer cannot word.
pub fn effect_words(rules: &Ability, name: &str) -> String {
    let Some(text) = mtg_cards::compile::render_ability(rules) else {
        return String::new();
    };
    let text = text.replace('~', name);
    if matches!(rules, Ability::Triggered { .. }) {
        without_event(&text)
    } else {
        text
    }
}

/// The text of a triggered ability without the event it waits for, its first letter a capital; the whole text when no
/// event can be told from the effect.
fn without_event(text: &str) -> String {
    // The event ends at the first comma that is no part of a list ("creatures, planeswalkers, or battles"): one
    // before "or" or "and", or before an item of a word or two that such a comma follows.
    let joins = |rest: &str| rest.starts_with("or ") || rest.starts_with("and ");
    let mut from = 0;
    let effect = loop {
        let Some(at) = text[from..].find(", ").map(|at| at + from) else {
            break None;
        };
        let rest = &text[at + 2..];
        let item = rest.find(", ").is_some_and(|next| {
            joins(&rest[next + 2..]) && rest[..next].split_whitespace().count() <= 2
        });
        if joins(rest) || item {
            from = at + 2;
            continue;
        }
        break Some(rest);
    };
    let Some(effect) = effect.filter(|effect| !effect.is_empty()) else {
        return text.to_owned();
    };
    let mut effect = effect.to_owned();
    if let Some(first) = effect.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    effect
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
            let rules = ability_reader::ability_as_it_was(state, address, &t.snapshot)
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
    // "Do this only once each turn" is spent as its choice is done (`fold::resolution`), not as it triggers.
    if let Some(limit) = limit.filter(|limit| *limit != Limit::OncePerTurnDone)
        && !state.spend_limit(limit, t.source, t.origin, t.ability)
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
    let taken = state.pending_triggers.remove(at);
    let member = RunMember {
        object: put.object,
        source: put.source,
    };
    join_run(state, taken.round, put.controller, member);
    Ok(taken)
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
        cause: Box::new(t.cause.clone()),
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
                        origin: e.trigger.origin,
                        text: e.effect(state),
                        cause: e.cause(state),
                    }
                })
                .collect();
            return Some(Progress::Decide(Decision {
                player,
                kind: DecisionKind::OrderTriggers(TriggersOptions { triggers }),
            }));
        }
        return Some(Progress::Put(Box::new(Pending::trigger(
            player,
            state.pending_triggers[*first].clone(),
        ))));
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
                r115_causer: None,
                r121_made: Vec::new(),
                dealers: Vec::new(),
                subjects: vec![crate::choose::Target::Player(subject)],
                snapshots: Vec::new(),
                amount: 0,
                source: None,
                against: None,
                counters: Vec::new(),
                r111_paid: Vec::new(),
                r113_spent_from: Vec::new(),
                event: None,
                ability: None,
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
        let crate::announce::What::Trigger(first) = pending.what else {
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

    #[rstest]
    #[case::an_entering(
        "Whenever another creature enters, you may gain 1 life.",
        "You may gain 1 life."
    )]
    #[case::a_payment(
        "Whenever another creature you control with power 2 or less enters, you may pay {1}. If you do, draw a card.",
        "You may pay {1}. If you do, draw a card."
    )]
    #[case::a_list_in_the_event(
        "Whenever one or more creatures, planeswalkers, or battles you control are dealt damage, draw a card.",
        "Draw a card."
    )]
    #[case::a_condition_after_the_event(
        "At the beginning of your upkeep, if you control an artifact, scry 1.",
        "If you control an artifact, scry 1."
    )]
    #[case::an_effect_of_two_parts(
        "When Alpha enters, draw a card, and you gain 1 life.",
        "Draw a card, and you gain 1 life."
    )]
    #[case::no_event_to_tell("Draw a card.", "Draw a card.")]
    #[case::nothing_after_the_comma("When Alpha dies, ", "When Alpha dies, ")]
    fn the_effect_of_a_trigger_is_its_text_after_the_event(
        #[case] text: &str,
        #[case] effect: &str,
    ) {
        assert_eq!(without_event(text), effect);
    }

    #[test]
    fn the_words_of_a_trigger_name_its_source_and_leave_out_the_event() {
        let words = effect_words(&watching(2), "Alpha");
        let whole = mtg_cards::compile::render_ability(&watching(2)).unwrap();
        assert!(!words.is_empty() && !words.starts_with("When"), "{words}");
        assert!(
            words.chars().next().is_some_and(char::is_uppercase),
            "{words}"
        );
        assert!(
            whole.to_lowercase().ends_with(&words.to_lowercase()),
            "{words:?} ends {whole:?}"
        );
        assert!(words.len() < whole.len());
    }

    /// Puts the trigger that waits first for `player` on the stack, and gives its object.
    fn put_next(state: &mut GameState, player: PlayerId) -> ObjectId {
        let at = *group(state, player).first().unwrap();
        let waiting = state.pending_triggers()[at].trigger().clone();
        let object = state.next_object();
        let put = crate::abilities::PutOnStack {
            origin: waiting.origin,
            object,
            source: waiting.source,
            snapshot: waiting.snapshot.clone(),
            ability: waiting.ability,
            controller: player,
            choices: crate::choose::Choices::default(),
            cause: Some(waiting.cause.clone()),
            delayed: None,
        };
        apply_event(state, &Event::AbilityPutOnStack(Box::new(put))).unwrap();
        object
    }

    #[rstest]
    #[case::two(&[("Alpha", 0), ("Beta", 0)])]
    #[case::three(&[("Alpha", 0), ("Alpha", 1), ("Beta", 0)])]
    fn triggers_a_player_puts_on_the_stack_together_are_one_run_that_resolves_last_first(
        #[case] mine: &[(&str, usize)],
    ) {
        let mut state = table();
        queue(&mut state, mine);
        queue(&mut state, &[("Gamma", 0)]);
        let put: Vec<ObjectId> = mine.iter().map(|_| put_next(&mut state, ME)).collect();
        let theirs = put_next(&mut state, FOE);
        let run = state.trigger_run_of(put[0]).unwrap().clone();
        assert_eq!(run.controller(), ME);
        for object in &put {
            assert_eq!(state.trigger_run_of(*object).unwrap().id(), run.id());
        }
        let resolving: Vec<ObjectId> = run.resolving_order().map(|member| member.object).collect();
        let last_first: Vec<ObjectId> = put.iter().rev().copied().collect();
        assert_eq!(resolving, last_first);
        // Another player's triggers are a run of their own.
        let other = state.trigger_run_of(theirs).unwrap();
        assert_eq!(other.controller(), FOE);
        assert_eq!(other.resolving_order().count(), 1);
        // What triggers after a run began is no part of it.
        queue(&mut state, &[("Beta", 0)]);
        let late = put_next(&mut state, ME);
        assert_ne!(state.trigger_run_of(late).unwrap().id(), run.id());
        assert_eq!(
            state
                .trigger_run_of(put[0])
                .unwrap()
                .resolving_order()
                .count(),
            mine.len()
        );
        assert!(state.trigger_run_of(ObjectId(9_999_999)).is_none());
    }

    #[test]
    fn a_cause_is_public_only_while_its_object_lies_face_up_on_the_battlefield_or_the_stack() {
        let state = table();
        let alpha = state.object(id(&state, "Alpha")).unwrap().reference();
        let about = |subjects: Vec<crate::choose::Target>| Cause {
            subjects,
            ..Cause::default()
        };
        assert_eq!(
            public_cause(&state, &about(vec![crate::choose::Target::Object(alpha)])),
            Some(alpha.id)
        );
        // A player is no object, an object that is gone is none, and an older self of the object is not the object.
        assert_eq!(
            public_cause(&state, &about(vec![crate::choose::Target::Player(ME)])),
            None
        );
        let gone = ObjectRef {
            id: ObjectId(9_999_999),
            counter: 0,
        };
        assert_eq!(
            public_cause(&state, &about(vec![crate::choose::Target::Object(gone)])),
            None
        );
        let older = ObjectRef {
            id: alpha.id,
            counter: alpha.counter + 1,
        };
        assert_eq!(
            public_cause(&state, &about(vec![crate::choose::Target::Object(older)])),
            None
        );
        assert_eq!(public_cause(&state, &Cause::default()), None);
    }
}
