//! The public driver: advance until a player must decide, then validate and apply answers.

use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::announce::{Answered, Next, Pending, What, announce};
use crate::answers::{Outcome, handle};
use crate::characteristics::controller_of_object;
use crate::choose::Choice;
use crate::decision::{Answer, Decision};
use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::error::{EngineError, PreviewError};
use crate::events::Event;
use crate::flow::{Progress, next};
use crate::fold::apply_event;
use crate::forecast::CombatForecast;
use crate::object::{GameObject, ObjectRef};
use crate::player::LossReason;
use crate::preview::preview_forecast;
use crate::replace::{self, Processed};
use crate::rules::{Rule, RuleEffect};
use crate::setup::GameSetup;
use crate::state::{GameResult, GameState};
use crate::triggers;

/// Internal progress steps allowed between decisions before the engine reports a stall.
const MAX_INTERNAL_STEPS: usize = 10_000;

/// Where an engine stood before a step; [`Engine::restore`] returns it to that point.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    mark: usize,
    decision: Option<Decision>,
    pending: Option<Pending>,
    held: Option<Held>,
}

/// A batch of events the replacement step has questions about: the batch waits, and the answers so far are kept, until
/// the step needs no more (`replace::process`).
#[derive(Debug, Clone)]
struct Held {
    proposed: Vec<Event>,
    journal: Vec<(PlayerId, Choice)>,
}

/// Replays `events` over the initial state of `setup`.
pub fn replay(setup: &GameSetup, events: &[Event]) -> Result<GameState, EngineError> {
    let mut state = GameState::from_setup(setup)?;
    for event in events {
        apply_event(&mut state, event)?;
    }
    Ok(state)
}

/// A running game: state, the event log that produced it, and the decision it is waiting on.
#[derive(Debug)]
pub struct Engine {
    setup: GameSetup,
    state: GameState,
    events: Vec<Event>,
    decision: Option<Decision>,
    pending: Option<Pending>,
    held: Option<Held>,
}

impl Engine {
    /// Starts a game and runs it to the first decision.
    pub fn new(setup: GameSetup) -> Result<Engine, EngineError> {
        Engine::resume(setup, Vec::new())
    }

    /// Rebuilds a game from its setup and event log, then runs to the next decision.
    pub fn resume(setup: GameSetup, events: Vec<Event>) -> Result<Engine, EngineError> {
        let state = replay(&setup, &events)?;
        let mut engine = Engine {
            setup,
            state,
            events,
            decision: None,
            pending: None,
            held: None,
        };
        engine.advance()?;
        Ok(engine)
    }

    /// The current state.
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Fingerprint of the current state.
    pub fn state_hash(&self) -> u64 {
        self.state.state_hash()
    }

    /// Every event so far, from the start of the game.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The setup the game was created from.
    pub fn setup(&self) -> &GameSetup {
        &self.setup
    }

    /// Whether the pending decision is a question of the replacement step: the events it is about are not applied yet, so
    /// the state is not settled (a creature that is to die is still in play).
    pub const fn replacing(&self) -> bool {
        self.held.is_some()
    }

    /// The pending decision; `None` once the game is over.
    pub fn decision(&self) -> Option<&Decision> {
        self.decision.as_ref()
    }

    /// The result once the game has ended.
    pub fn result(&self) -> Option<GameResult> {
        self.state.result()
    }

    /// Validates `answer` against the pending decision and advances to the next one.
    ///
    /// Illegal answers return an error and leave the engine untouched. On success returns every
    /// event produced, including the automatic steps that followed the answer.
    pub fn apply(&mut self, answer: Answer) -> Result<Vec<Event>, EngineError> {
        let decision = self.decision.as_ref().ok_or(EngineError::NoDecision)?;
        let outcome = handle(&self.state, self.pending.as_ref(), decision, answer)?;
        let player = decision.player;
        let checkpoint = self.checkpoint();
        let result = self.run(outcome, player).and_then(|()| self.advance());
        self.finish(checkpoint, result)
    }

    /// What combat damage would do if `seat` gave `answer` to the decision it faces now, without giving it.
    ///
    /// Only a pending attack or block can be previewed. The answer is validated exactly as [`apply`](Engine::apply)
    /// would, so an illegal one is an error and never a forecast. The engine is not touched: no event is logged,
    /// no random number is drawn and the state hash stays as it is.
    pub fn preview_forecast(
        &self,
        seat: PlayerId,
        answer: &Answer,
    ) -> Result<CombatForecast, PreviewError> {
        let decision = self.decision.as_ref().ok_or(PreviewError::NoDecision)?;
        if decision.player != seat {
            return Err(PreviewError::NotYourDecision(seat));
        }
        preview_forecast(&self.state, decision, answer)
    }

    /// Removes `player` from the game at once (CR 104.3a); a player may concede at any time.
    ///
    /// Returns the events produced: the loss, what follows from it (their objects leaving, the end
    /// of the turn if it was theirs, the end of the game if one player remains) and the advance to
    /// the next decision. A cast the engine was waiting to see paid for is abandoned. On error the
    /// engine is untouched.
    pub fn concede(&mut self, player: PlayerId) -> Result<Vec<Event>, EngineError> {
        if self.decision.is_none() {
            return Err(EngineError::NoDecision);
        }
        if !self.state.is_alive(player) {
            return Err(EngineError::NotInGame(player));
        }
        let checkpoint = self.checkpoint();
        self.pending = None;
        let lost = Event::PlayerLost {
            player,
            reason: LossReason::Conceded,
        };
        let result = self.commit(vec![lost]).and_then(|()| self.advance());
        self.finish(checkpoint, result)
    }

    /// Notes the current position so that the steps taken after it can be undone with [`restore`](Engine::restore).
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            mark: self.events.len(),
            decision: self.decision.clone(),
            pending: self.pending.clone(),
            held: self.held.clone(),
        }
    }

    /// Undoes every step taken since `checkpoint`: the log is cut back and the state rebuilt from it.
    ///
    /// The checkpoint must come from this engine and not lie beyond the current log. Callers use
    /// this when persisting a step fails, so the in-memory game never runs ahead of the store.
    pub fn restore(&mut self, checkpoint: Checkpoint) -> Result<(), EngineError> {
        if checkpoint.mark > self.events.len() {
            return Err(EngineError::InvalidEvent(format!(
                "checkpoint at event {} lies beyond the log of {} events",
                checkpoint.mark,
                self.events.len()
            )));
        }
        let state = replay(&self.setup, &self.events[..checkpoint.mark])?;
        self.events.truncate(checkpoint.mark);
        self.state = state;
        self.decision = checkpoint.decision;
        self.pending = checkpoint.pending;
        self.held = checkpoint.held;
        Ok(())
    }

    /// Registers a continuous effect that has no source and logs it.
    ///
    /// Spells and abilities create their effects when they resolve (`Effect::Continuous`); this is the hook tests
    /// use to prove buffs, debuffs, durations and layering. It belongs to the active player. A duration that follows
    /// the source (`WhileSourceOnBattlefield`) needs [`add_continuous_effect_from`](Engine::add_continuous_effect_from).
    pub fn add_continuous_effect(
        &mut self,
        selector: Selector,
        modification: Modification,
        duration: Duration,
    ) -> Result<Vec<Event>, EngineError> {
        self.register(None, selector, modification, duration)
    }

    /// Registers a continuous effect as if a spell or ability of the object `source` had created it, and logs it.
    pub fn add_continuous_effect_from(
        &mut self,
        source: ObjectId,
        selector: Selector,
        modification: Modification,
        duration: Duration,
    ) -> Result<Vec<Event>, EngineError> {
        let source = self
            .state
            .object(source)
            .map(GameObject::reference)
            .ok_or_else(|| EngineError::InvalidEvent(format!("{source:?} is not in the game")))?;
        self.register(Some(source), selector, modification, duration)
    }

    /// Registers a change to a rule of the game as if a spell or ability had made it, and logs it: the hook tests use to
    /// prove the rules and their durations. It belongs to the active player. A duration that follows the source
    /// (`WhileSourceOnBattlefield`) needs [`add_rule_from`](Engine::add_rule_from).
    pub fn add_rule(&mut self, rule: Rule, duration: Duration) -> Result<Vec<Event>, EngineError> {
        self.register_rule(None, rule, duration)
    }

    /// Registers a change to a rule of the game as if a spell or ability of the object `source` had made it, and logs it.
    pub fn add_rule_from(
        &mut self,
        source: ObjectId,
        rule: Rule,
        duration: Duration,
    ) -> Result<Vec<Event>, EngineError> {
        let source = self
            .state
            .object(source)
            .map(GameObject::reference)
            .ok_or_else(|| EngineError::InvalidEvent(format!("{source:?} is not in the game")))?;
        self.register_rule(Some(source), rule, duration)
    }

    fn register_rule(
        &mut self,
        source: Option<ObjectRef>,
        rule: Rule,
        duration: Duration,
    ) -> Result<Vec<Event>, EngineError> {
        if self.decision.is_none() {
            return Err(EngineError::NoDecision);
        }
        let controller = source
            .and_then(|source| self.state.object(source.id))
            .map_or_else(
                || self.state.active_or_next(),
                |object| controller_of_object(&self.state, object),
            );
        let effect = RuleEffect {
            id: EffectId(self.state.next_effect),
            rule,
            duration,
            source,
            controller,
            turn: self.state.turn().number(),
        };
        let checkpoint = self.checkpoint();
        self.pending = None;
        let result = self
            .commit(vec![Event::RuleCreated { effect }])
            .and_then(|()| self.advance());
        self.finish(checkpoint, result)
    }

    fn register(
        &mut self,
        source: Option<ObjectRef>,
        selector: Selector,
        modification: Modification,
        duration: Duration,
    ) -> Result<Vec<Event>, EngineError> {
        if self.decision.is_none() {
            return Err(EngineError::NoDecision);
        }
        let controller = source
            .and_then(|source| self.state.object(source.id))
            .map_or_else(
                || self.state.active_or_next(),
                |object| controller_of_object(&self.state, object),
            );
        let mut effect = ContinuousEffect::new(
            EffectId(self.state.next_effect),
            selector,
            modification,
            duration,
            self.state.next_timestamp,
        )
        .begun(controller, self.state.turn().number());
        if let Some(source) = source {
            effect = effect.from_source(source);
        }
        let checkpoint = self.checkpoint();
        self.pending = None;
        let result = self
            .commit(vec![Event::EffectCreated { effect }])
            .and_then(|()| self.advance());
        self.finish(checkpoint, result)
    }

    fn run(&mut self, outcome: Outcome, player: PlayerId) -> Result<(), EngineError> {
        match outcome {
            Outcome::Events(events) if self.held.is_some() => self.answer_held(events),
            Outcome::Events(events) => {
                self.pending = None;
                self.commit(events)
            }
            Outcome::BeginCast(spell) => {
                self.pending = Some(Pending::new(player, What::Spell(spell)));
                Ok(())
            }
            Outcome::BeginActivation(ability) => {
                self.pending = Some(Pending::new(player, What::Ability(ability)));
                Ok(())
            }
            Outcome::Modes(modes) => self.announced(Answered::Modes(modes)),
            Outcome::Way(pick) => self.announced(Answered::Way(pick)),
            Outcome::Targets(targets) => self.announced(Answered::Targets(targets)),
            Outcome::Number(number) => self.announced(Answered::Number(number)),
            Outcome::Costs(costs) => self.announced(Answered::Costs(costs)),
            Outcome::Cards(cards) => self.announced(Answered::Cards(cards)),
            Outcome::Division(shares) => self.announced(Answered::Division(shares)),
            Outcome::CancelCast => {
                self.pending = None;
                Ok(())
            }
            Outcome::DropTrigger => match self.pending.take() {
                Some(Pending {
                    what: What::Trigger(waiting),
                    ..
                }) => self.commit(vec![triggers::removing(&waiting)]),
                other => {
                    self.pending = other;
                    Err(EngineError::InvalidEvent(
                        "no triggered ability is being put on the stack".into(),
                    ))
                }
            },
        }
    }

    /// Begins `pending` as if the player had chosen it, to try an announcement a card cannot ask for yet.
    #[cfg(test)]
    pub(crate) fn begin(&mut self, pending: Pending) -> Result<(), EngineError> {
        self.pending = Some(pending);
        self.advance()
    }

    /// Hands the answer to a question of the announcement to the cast or activation that asked it.
    fn announced(&mut self, answered: Answered) -> Result<(), EngineError> {
        match &mut self.pending {
            Some(pending) => pending.accept(answered),
            None => Err(EngineError::InvalidEvent(
                "an announcement was answered with nothing announced".into(),
            )),
        }
    }

    /// Passes the events through the replacement step and commits what comes out. When the step has a question, the
    /// batch is held and the question is the decision; the answer reruns the step (`answer_held`).
    fn commit(&mut self, events: Vec<Event>) -> Result<(), EngineError> {
        self.replace_and_commit(events, Vec::new())
    }

    fn replace_and_commit(
        &mut self,
        events: Vec<Event>,
        journal: Vec<(PlayerId, Choice)>,
    ) -> Result<(), EngineError> {
        match replace::process(&self.state, events, &journal)? {
            Processed::Events(events) => {
                self.held = None;
                self.fold_and_log(events)
            }
            Processed::Ask { decision, proposed } => {
                self.held = Some(Held { proposed, journal });
                self.decision = Some(decision);
                Ok(())
            }
        }
    }

    /// Takes the answer to a question of the replacement step and reruns the step with it.
    fn answer_held(&mut self, answers: Vec<Event>) -> Result<(), EngineError> {
        let Some(Held {
            proposed,
            mut journal,
        }) = self.held.take()
        else {
            return Err(EngineError::NoDecision);
        };
        for answer in answers {
            if let Event::ChoiceMade { player, choice } = answer {
                journal.push((player, choice));
            }
        }
        self.replace_and_commit(proposed, journal)
    }

    /// Folds and logs the events, then the triggered abilities they set off (CR 603.2): those are found by comparing
    /// the state before the events with the state after, and their own events follow in the log.
    fn fold_and_log(&mut self, events: Vec<Event>) -> Result<(), EngineError> {
        // A batch that brings in the first thing that can trigger (a token, a granted ability) is watched too.
        let watching = self.state.watches() || triggers::introduces(&events);
        let before = (watching && triggers::needs_before(&events)).then(|| self.state.clone());
        let start = self.events.len();
        for event in events {
            apply_event(&mut self.state, &event)?;
            self.events.push(event);
        }
        if !watching {
            return Ok(());
        }
        let triggered = triggers::detect(
            before.as_ref().unwrap_or(&self.state),
            &self.events[start..],
            &self.state,
        );
        for trigger in triggered {
            let event = Event::AbilityTriggered(Box::new(trigger));
            apply_event(&mut self.state, &event)?;
            self.events.push(event);
        }
        Ok(())
    }

    /// Runs the game until a decision is needed or the game ends.
    fn advance(&mut self) -> Result<(), EngineError> {
        for _ in 0..MAX_INTERNAL_STEPS {
            // A batch waits for the answer to a question of the replacement step: that question is the decision.
            if self.held.is_some() {
                return Ok(());
            }
            if let Some(pending) = self.pending.as_mut() {
                match announce(&self.state, pending) {
                    Next::Ask(decision) => {
                        self.decision = Some(decision);
                        return Ok(());
                    }
                    Next::Commit(events) => {
                        self.pending = None;
                        self.commit(events)?;
                    }
                    // A cast the resolution of another spell or ability began that cannot go on tells it so (CR 608.2g).
                    Next::Drop => {
                        if let Some(Pending {
                            player,
                            granted: Some(_),
                            ..
                        }) = self.pending.take()
                        {
                            self.commit(vec![Event::ChoiceMade {
                                player,
                                choice: Choice::YesNo(false),
                            }])?;
                        }
                    }
                }
                continue;
            }
            match next(&self.state) {
                Progress::Decide(decision) => {
                    self.decision = Some(decision);
                    return Ok(());
                }
                Progress::Done => {
                    self.decision = None;
                    return Ok(());
                }
                Progress::Put(pending) => self.pending = Some(pending),
                Progress::Events(events) => self.commit(events)?,
                Progress::Fail(error) => return Err(error),
            }
        }
        Err(EngineError::Stalled(MAX_INTERNAL_STEPS))
    }

    /// The events since `checkpoint` on success; on failure the step is undone so it never half-applies.
    fn finish(
        &mut self,
        checkpoint: Checkpoint,
        result: Result<(), EngineError>,
    ) -> Result<Vec<Event>, EngineError> {
        let mark = checkpoint.mark;
        match result {
            Ok(()) => Ok(self.events[mark..].to_vec()),
            Err(error) => {
                // A prefix that already folded once cannot fail to replay; if it somehow does, the original error is the useful one.
                let _ = self.restore(checkpoint);
                Err(error)
            }
        }
    }
}
