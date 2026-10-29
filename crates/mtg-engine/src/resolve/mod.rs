//! Resolving a spell or ability: the effect list runs as a journal of steps, each a pure function of state.
//!
//! [`step`] reads the state and the journal and never changes either. It returns the events of one effect
//! (ending with `StepDone`, which moves the position on and clears the journal) or the decision the effect
//! needs; the answer is logged as `ChoiceMade`, joins the journal and the same step runs again to use it, so a
//! step with k decisions runs k + 1 times. State stays the fold of the log, and an engine rebuilt from the log
//! stands at the same step (`docs/cards-dsl.md`, Execution).

mod inline;
mod prim;
mod share;
mod tree;
mod value;

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

pub(crate) use inline::{run_inline, w5d_run_instead};
pub(crate) use prim::lower;
pub use value::{Bindings, ResolveError, Scope};
pub(crate) use value::{CastTargets, zone_kind};

use crate::abilities::{AbilityObject, AbilityRef};
use crate::ability_reader::abilities_of;
use crate::announce::Pending;
use crate::choose::{Choice, Target, announced_specs, targets_before};
use crate::decision::Decision;
use crate::error::EngineError;
use crate::events::{Event, ResolutionOutcome};
use crate::flow;
use crate::keywords::{awaken, cipher, gift};
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::zones::Zone;

/// The prefix of a bound name in a finished step that removes the name instead: the way a repeat frees what its body bound.
pub(crate) const CLEAR: &str = "~clear:";

/// The prefix of a bound name in a finished step that records the X its payment announced (`~x:3`) instead.
pub(crate) const W6C_PAID_X: &str = "~x:";

/// The prefix of the bound name (`~legal:<index>`) under which the first step records the targets of a specification
/// that are legal as the resolution begins: the later steps act on them even after an earlier step made them illegal
/// (CR 608.2b).
pub(crate) const W7D_LEGAL: &str = "~legal:";

/// The position of the first effect: a path of indices into the effect tree.
const START: [usize; 1] = [0];

/// A card a resolution showed to one seat, or to every seat, for as long as the resolution lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shown {
    /// The seat that may identify the card; every seat when `None`.
    pub to: Option<PlayerId>,
    /// The card as it was when it was shown: a new zone makes it another object (CR 400.7).
    pub card: ObjectRef,
}

/// A spell or ability that is resolving: how far it is, what it bound and the answers waiting to be used.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Resolving {
    pub(crate) object: ObjectId,
    pub(crate) position: Vec<usize>,
    pub(crate) bindings: Bindings,
    pub(crate) snapshots: Vec<Snapshot>,
    pub(crate) journal: Vec<(PlayerId, Choice)>,
    pub(crate) shown: Vec<Shown>,
    /// The life the resolution has made players lose so far, which "the life lost this way" reads.
    pub(crate) lost: u32,
    /// The dice rolled so far in the resolution, in the order logged, which "the result" reads.
    pub(crate) rolled: Vec<u32>,
    /// The permanents the resolution has destroyed so far, as the objects they were, which "destroyed this way" reads.
    pub(crate) destroyed: Vec<ObjectRef>,
    /// The permanents the resolution has phased out so far, which "phased out this way" reads.
    pub(crate) w11b_phased: Vec<ObjectRef>,
    /// The X announced as a payment in the resolution was made ("you may pay {X}{R}. If you do"), which the effects after it read.
    pub(crate) w6c_paid_x: Option<u32>,
}

impl Resolving {
    pub(crate) fn begin(object: ObjectId) -> Self {
        Resolving {
            object,
            position: START.to_vec(),
            bindings: Bindings::new(),
            snapshots: Vec::new(),
            journal: Vec::new(),
            shown: Vec::new(),
            lost: 0,
            rolled: Vec::new(),
            destroyed: Vec::new(),
            w11b_phased: Vec::new(),
            w6c_paid_x: None,
        }
    }

    /// The cards looked at, found or revealed so far in this resolution, and to whom.
    pub fn shown(&self) -> &[Shown] {
        &self.shown
    }

    /// The spell or ability.
    pub const fn object(&self) -> ObjectId {
        self.object
    }

    /// The path of the effect that runs next: indices into the effect tree.
    pub fn position(&self) -> &[usize] {
        &self.position
    }

    /// What the finished steps bound to names.
    pub const fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// Objects the finished steps moved away, as they were.
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    /// The answers given to the step that is running and not yet used by it.
    pub fn journal(&self) -> &[(PlayerId, Choice)] {
        &self.journal
    }
}

/// What one step of a resolution came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// The events of the step, ending with `StepDone`, or the end of the resolution.
    Done(Vec<Event>),
    /// The step needs an answer first.
    Ask(Decision),
    /// The step has a spell cast: its announcement is decided, with no event logged, and the step runs again once the
    /// spell is on the stack or cannot be (CR 608.2g).
    Cast(Cast),
}

/// A cast a resolution has a player make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cast {
    /// Who casts it.
    pub player: PlayerId,
    /// The card.
    pub card: ObjectId,
    /// The mana cost is not paid.
    pub free: bool,
    /// The alternative cost it is cast for.
    pub way: Option<mtg_cards::value::CastWay>,
}

/// Where a step goes after its effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// On to the next effect.
    Sibling,
    /// Into the effects of this branch of the effect (`then`, `paid`, a mode), or past it when it is empty.
    Enter(usize),
    /// The same effect again once the events are folded, for an effect that needs the objects it creates.
    Again,
}

/// What running one effect came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// The effect is done: these events are proposed, these names are bound and these objects were moved away.
    Done {
        /// The events the effect proposes; the replacement step sees them before the fold does.
        events: Vec<Event>,
        /// Names the effect binds.
        bound: Vec<(String, Vec<Target>)>,
        /// Objects the effect moved away, as they were.
        snapshots: Vec<Snapshot>,
        /// Where the resolution goes next.
        next: Next,
    },
    /// The effect needs an answer first.
    Ask(Decision),
    /// The effect has a spell cast; the step runs again once it is on the stack (`Progress::Cast`).
    Cast(Cast),
}

impl Run {
    /// An effect that is done and binds nothing.
    pub const fn events(events: Vec<Event>) -> Run {
        Run::Done {
            events,
            bound: Vec::new(),
            snapshots: Vec::new(),
            next: Next::Sibling,
        }
    }
}

/// What an effect may read while it runs.
#[derive(Debug, Clone)]
pub struct Cx<'a> {
    pub(crate) scope: Scope<'a>,
    pub(crate) resolving: &'a Resolving,
}

impl Cx<'_> {
    /// The game.
    pub fn state(&self) -> &GameState {
        self.scope.state
    }

    /// The player who controls the spell or ability.
    pub const fn controller(&self) -> PlayerId {
        self.scope.controller
    }

    /// The answers given to this step so far.
    pub fn journal(&self) -> &[(PlayerId, Choice)] {
        &self.resolving.journal
    }
}

/// The `Spell` ability of `id` with its address: what the card does as an instant or sorcery.
pub(crate) fn spell_ability(state: &GameState, id: ObjectId) -> Option<(AbilityRef, Arc<Ability>)> {
    let (address, rules) = abilities_of(state, id)
        .into_iter()
        .find(|(_, rules)| matches!(**rules, Ability::Spell { .. }))?;
    // Awaken adds a target and effects to the spell ability of its card.
    let rules = match state.object(id) {
        Some(card) => awaken::awakened(card, rules),
        None => rules,
    };
    Some((address, rules))
}

/// The rules a stack object runs: those of the ability it is, or the `Spell` ability of its card.
fn rules_of(state: &GameState, object: &GameObject) -> Option<Arc<Ability>> {
    match object.ability() {
        Some(ability) => Some(ability.shared_rules()),
        None => spell_ability(state, object.id())
            .map(|(_, rules)| cipher::encoded(object, gift::before(object, rules))),
    }
}

/// Whether resolving the object runs effects; a permanent spell, or a card whose text is left to the table, does not.
pub(crate) fn runs_effects(state: &GameState, object: &GameObject) -> bool {
    rules_of(state, object).is_some()
}

fn scope_of<'a>(
    state: &'a GameState,
    object: &'a GameObject,
    resolving: &'a Resolving,
    rules: &'a Ability,
) -> Scope<'a> {
    let ability = object.ability();
    let mut snapshots: Vec<&Snapshot> = resolving.snapshots.iter().collect();
    snapshots.extend(object.choices().cost_shots());
    if let Some(ability) = ability {
        snapshots.push(ability.snapshot());
        snapshots.extend(ability.cause().iter().flat_map(|c| c.snapshots.iter()));
    }
    // A modal ability announced the targets of the modes it chose, one mode after the other; inside a mode
    // `Target(0)` is that mode's first.
    let chosen = &object.choices().modes;
    let target_base = tree::mode_at(rules.effects(), chosen, &resolving.position)
        .map_or(0, |ordinal| targets_before(rules, chosen, ordinal));
    Scope {
        state,
        controller: object.controller(),
        source: ability.map_or_else(|| object.reference(), AbilityObject::source),
        choices: object.choices(),
        specs: announced_specs(rules, chosen),
        target_base,
        bindings: &resolving.bindings,
        snapshots,
        departed: &resolving.snapshots,
        cause: ability.and_then(|a| a.cause()),
        layered: None,
        casting: None,
        cast_targets: None,
    }
}

/// The events that end a resolution that did what it says.
fn finish(state: &GameState, object: &GameObject) -> Vec<Event> {
    let mut events = vec![Event::ResolutionEnded {
        object: object.id(),
        outcome: ResolutionOutcome::Resolved,
    }];
    if object.kind() == ObjectKind::Ability {
        return events;
    }
    if object.zone() == Zone::Stack {
        events.extend(crate::stack::resolved(state, object.id()));
    } else if let Some(player) = state.turn_order_from(state.turn().active()).first() {
        // A spell that left the stack by its own effect (cipher exiles it encoded) is not moved again; the active player
        // gets priority as after any resolution.
        events.push(Event::PriorityGranted { player: *player });
    }
    events
}

/// Runs the next step of the resolution in progress.
pub fn step(state: &GameState) -> Result<Progress, EngineError> {
    step_with(state, prim::run)
}

/// [`step`] with the interpreter's primitives replaced by `run`, so the journal can be tried on effects that ask.
fn step_with(
    state: &GameState,
    run: impl Fn(&Cx<'_>, &Effect) -> Result<Run, ResolveError>,
) -> Result<Progress, EngineError> {
    let resolving = state
        .resolving()
        .ok_or_else(|| EngineError::Resolution("nothing is resolving".into()))?;
    let Some(object) = state.object(resolving.object) else {
        // A copy of a spell that its own effect moved ("Exile Teferi's Protection") ceased to exist with that move
        // (CR 111.7, 707.10a): its resolution is over, and the active player gets priority as after any resolution.
        return if state.has_ceased(resolving.object) {
            let mut events = vec![Event::ResolutionEnded {
                object: resolving.object,
                outcome: ResolutionOutcome::Resolved,
            }];
            if let Some(player) = state.turn_order_from(state.turn().active()).first() {
                events.push(Event::PriorityGranted { player: *player });
            }
            Ok(Progress::Done(events))
        } else {
            Err(EngineError::Resolution(format!(
                "{:?} is not in the game",
                resolving.object
            )))
        };
    };
    if !state.is_alive(object.controller()) {
        return Ok(Progress::Done(vec![Event::ResolutionEnded {
            object: object.id(),
            outcome: ResolutionOutcome::Countered,
        }]));
    }
    let Some(rules) = rules_of(state, object) else {
        return Ok(Progress::Done(finish(state, object)));
    };
    let scope = scope_of(state, object, resolving, &rules);
    // CR 603.4: a triggered ability whose "if" is no longer true as it resolves does nothing.
    if resolving.position == START
        && let Ability::Triggered {
            intervening_if: Some(condition),
            ..
        } = &*rules
        && !scope.holds(condition)?
    {
        return Ok(Progress::Done(vec![Event::ResolutionEnded {
            object: object.id(),
            outcome: ResolutionOutcome::Lapsed,
        }]));
    }
    // CR 702.131a: an instant or sorcery spell with ascend gives the blessing before any of its effects.
    if resolving.position == START
        && let Some(blessing) = crate::citys_blessing::on_resolving(state, object)
    {
        return Ok(Progress::Done(vec![blessing]));
    }
    // CR 608.2b: a spell or ability whose targets have all become illegal does nothing at all.
    // A first step that was answered already began the resolution: a card it cast left its zone, and the spell or ability
    // goes on (CR 608.2g).
    if resolving.position == START && resolving.journal.is_empty() && scope.all_targets_illegal() {
        return Ok(Progress::Done(vec![Event::ResolutionEnded {
            object: object.id(),
            outcome: ResolutionOutcome::Fizzled,
        }]));
    }
    let effects = rules.effects();
    let chosen = &object.choices().modes;
    let Some(effect) = tree::effect_at(effects, chosen, &resolving.position) else {
        return Ok(Progress::Done(finish(state, object)));
    };
    let legal = scope.w7d_legal_at_start(resolving.position == START);
    let cx = Cx { scope, resolving };
    match run(&cx, effect)? {
        Run::Ask(decision) => Ok(Progress::Ask(decision)),
        Run::Cast(cast) => Ok(Progress::Cast(cast)),
        Run::Done {
            events,
            bound,
            snapshots,
            next,
        } => {
            let mut events = events;
            let mut bound = [bound, legal].concat();
            let position = tree::after(effects, chosen, &resolving.position, next);
            // CR 700.2d: a mode chosen again runs again; the names it binds are cleared for the new run.
            if let Some(mode) = tree::later_mode_begun(effects, chosen, &position) {
                bound.extend(
                    mtg_cards::effect::bound_names(mode)
                        .into_iter()
                        .map(|name| (format!("{CLEAR}{name}"), Vec::new())),
                );
            }
            events.push(Event::StepDone {
                position,
                bound,
                snapshots,
            });
            Ok(Progress::Done(events))
        }
    }
}

/// The next move of the engine while something resolves.
pub(crate) fn next(state: &GameState) -> flow::Progress {
    match step(state) {
        Ok(Progress::Done(events)) => flow::Progress::Events(events),
        Ok(Progress::Ask(decision)) => flow::Progress::Decide(decision),
        Ok(Progress::Cast(Cast {
            player,
            card,
            free,
            way,
        })) => flow::Progress::Put(Pending::during_resolution(player, card, free, way)),
        Err(error) => flow::Progress::Fail(error),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a step with k questions runs k + 1 times, each answer is logged and joins the journal until the step is done, and the log of the whole resolution replays to the state it was played to, mid-resolution included.
    use std::cell::Cell;
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Timing;
    use mtg_cards::cost::Cost;
    use mtg_cards::refs::{Ref, ZoneKind};
    use mtg_cards::value::Amount;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::abilities::PutOnStack;
    use crate::choose::{Choices, YesNoOptions};
    use crate::decision::DecisionKind;
    use crate::engine::replay;
    use crate::fixtures::{setup, state};
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;
    use crate::zones::Zone;

    fn gain(amount: i32) -> Effect {
        Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(amount),
        }
    }

    /// An artifact with a free ability of three steps: gain 1 life, gain 2 life, gain 4 life.
    fn totem() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Totem")
                .with_type(CardType::Artifact)
                .with_rules(Ability::Activated {
                    cost: Cost(Vec::new()),
                    timing: Timing::Instant,
                    condition: None,
                    limit: None,
                    from: ZoneKind::Battlefield,
                    targets: Vec::new(),
                    effects: vec![gain(1), gain(2), gain(4)],
                }),
        )
    }

    fn start() -> GameState {
        state(vec![vec![], vec![]], vec![vec![totem()], vec![]])
    }

    /// The events that put the totem's ability on the stack and begin resolving it.
    fn begin(state: &GameState) -> Vec<Event> {
        let source = state.zone(Zone::Battlefield)[0];
        let object = state.object(source).unwrap();
        let ability = state.next_object();
        vec![
            Event::AbilityPutOnStack(Box::new(PutOnStack {
                origin: crate::abilities::AbilitySource::default(),
                object: ability,
                source: object.reference(),
                snapshot: snapshot(state, source).unwrap(),
                ability: 0,
                controller: PlayerId(0),
                choices: Choices::default(),
                cause: None,
                delayed: None,
            })),
            Event::ResolutionBegan { object: ability },
        ]
    }

    /// Plays the resolution to its end, answering `answer` when the second step asks, and returns the log, the
    /// state, how many questions were asked and how many times the second step ran.
    fn play(answer: bool) -> (Vec<Event>, GameState, u32, u32) {
        let mut game = start();
        let mut log = begin(&game);
        for event in &log {
            apply_event(&mut game, event).unwrap();
        }
        let (asked, ran) = (Cell::new(0), Cell::new(0));
        let runner = |cx: &Cx<'_>, effect: &Effect| {
            if cx.resolving.position() != [1] {
                return prim::run(cx, effect);
            }
            ran.set(ran.get() + 1);
            match cx.journal() {
                [] => Ok(Run::Ask(Decision {
                    player: cx.controller(),
                    kind: DecisionKind::YesNo(YesNoOptions::new(cx.scope.source.id, "")),
                })),
                [(_, Choice::YesNo(yes))] => Ok(Run::events(if *yes {
                    vec![Event::LifeGained {
                        player: cx.controller(),
                        amount: 2,
                    }]
                } else {
                    Vec::new()
                })),
                other => Err(ResolveError::Invalid(format!(
                    "unexpected journal {other:?}"
                ))),
            }
        };
        loop {
            let events = match step_with(&game, runner).unwrap() {
                Progress::Done(events) => events,
                Progress::Ask(decision) => {
                    asked.set(asked.get() + 1);
                    assert_eq!(game.resolving().unwrap().journal(), []);
                    vec![Event::ChoiceMade {
                        player: decision.player,
                        choice: Choice::YesNo(answer),
                    }]
                }
                Progress::Cast(cast) => panic!("nothing was to be cast: {cast:?}"),
            };
            for event in &events {
                apply_event(&mut game, event).unwrap();
            }
            log.extend(events);
            if game.resolving().is_none() {
                break;
            }
        }
        (log, game, asked.get(), ran.get())
    }

    #[rstest]
    #[case::yes(true, 27)]
    #[case::no(false, 25)]
    fn a_question_runs_its_step_twice_and_the_answer_is_used_once(
        #[case] answer: bool,
        #[case] life: i32,
    ) {
        let (log, game, asked, ran) = play(answer);
        assert_eq!((asked, ran), (1, 2), "one question, so the step runs twice");
        assert_eq!(game.player(PlayerId(0)).unwrap().life(), life);
        let answers = log
            .iter()
            .filter(|e| matches!(e, Event::ChoiceMade { .. }))
            .count();
        assert_eq!(answers, 1, "the answer is logged");
        let ability = ObjectId(game.next_object().get() - 1);
        assert!(
            game.object(ability).is_none(),
            "the ability has ceased to exist"
        );
        assert_eq!(game.stack(), []);
    }

    #[test]
    fn the_journal_holds_the_answer_until_the_step_is_done() {
        let (log, ..) = play(true);
        let mut game = start();
        let mut at_answer = None;
        let mut steps = 0;
        for event in &log {
            apply_event(&mut game, event).unwrap();
            match event {
                Event::ChoiceMade { .. } => {
                    at_answer = Some(game.resolving().unwrap().journal().len());
                }
                Event::StepDone { position, .. } if position == &[2] => {
                    let resolving = game.resolving().unwrap();
                    assert_eq!(resolving.journal(), []);
                    assert_eq!(resolving.position(), [2]);
                    steps += 1;
                }
                _ => {}
            }
        }
        assert_eq!((at_answer, steps), (Some(1), 1));
    }

    #[rstest]
    #[case::before_the_ability_is_put_on_the_stack(0)]
    #[case::when_it_begins_to_resolve(2)]
    #[case::after_the_first_step(4)]
    #[case::at_the_question_answered(5)]
    #[case::after_the_second_step(7)]
    #[case::at_the_end(100)]
    fn the_log_replays_to_the_state_it_was_played_to_at_every_point(#[case] cut: usize) {
        let (log, ..) = play(true);
        let cut = cut.min(log.len());
        let replayed = replay(
            &setup(vec![vec![], vec![]], vec![vec![totem()], vec![]]),
            &log[..cut],
        )
        .unwrap();
        let mut played = start();
        for event in &log[..cut] {
            apply_event(&mut played, event).unwrap();
        }
        assert_eq!(replayed.state_hash(), played.state_hash());
        assert_eq!(replayed.resolving(), played.resolving());
    }
}
