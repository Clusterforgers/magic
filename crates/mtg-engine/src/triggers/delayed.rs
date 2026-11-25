//! Delayed triggered abilities (CR 603.7): an effect sets one up, and it triggers on a later event or step.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::refs::{TargetSpec, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Condition;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::abilities::AbilitySource;
use crate::choose::Target;
use crate::copies::KeptSpell;
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::object::ObjectRef;
use crate::resolve::Bindings;
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::turn::Step;

/// Names a delayed triggered ability from the moment it is set up until it has triggered or run out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DelayedId(pub u64);

/// A delayed triggered ability was set up: what the `DelayedTriggerCreated` event says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DelayedCreated {
    /// The name the delayed trigger goes by.
    pub id: DelayedId,
    /// The object whose spell or ability set it up.
    pub source: ObjectRef,
    /// The source as it was then, which the ability can still read once the source has gone.
    pub snapshot: Snapshot,
    /// Who controls it: the controller of the spell or ability that set it up (CR 603.7d).
    pub controller: PlayerId,
    /// The ability of the source's card whose resolution set it up.
    pub ability: usize,
    /// The event it waits for.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub when: TriggerPattern,
    /// Whether it triggers once and is gone, else it lasts until the end of the turn it was set up in.
    pub once: bool,
    /// Whether it waits only until the end of the turn it was set up in, also when it triggers once.
    #[serde(default)]
    pub this_turn: bool,
    /// How long it waits instead ("until your next turn"), also when it triggers once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub until: Option<Duration>,
    /// The targets it chooses as it goes on the stack (a reflexive trigger).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub targets: Vec<TargetSpec>,
    /// What it does.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub effects: Vec<Effect>,
    /// Its intervening "if" (CR 603.4), checked as it would trigger and as it resolves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "unknown", optional))]
    pub intervening_if: Option<Condition>,
    /// The names the resolution had bound when it set the trigger up, which the delayed ability reads as they were.
    pub bindings: Vec<(String, Vec<Target>)>,
    /// The spell that set it up, as it resolved, when the ability copies it after it has left the stack (epic).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub kept: Option<Box<KeptSpell>>,
    /// Where the ability that set it up comes from, which its trigger names as its own (the Ring's, say, not its
    /// bearer's); the card's own while it is printed.
    #[serde(default, skip_serializing_if = "AbilitySource::is_printed")]
    #[cfg_attr(feature = "ts", ts(as = "Option<AbilitySource>", optional))]
    pub origin: AbilitySource,
}

/// A delayed triggered ability waiting for its event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delayed {
    pub(crate) id: DelayedId,
    pub(crate) source: ObjectRef,
    pub(crate) snapshot: Snapshot,
    pub(crate) controller: PlayerId,
    pub(crate) ability: usize,
    pub(crate) rules: Arc<Ability>,
    pub(crate) bindings: Bindings,
    pub(crate) once: bool,
    pub(crate) duration: Duration,
    pub(crate) turn: u32,
    pub(crate) kept: Option<Arc<KeptSpell>>,
    pub(crate) origin: AbilitySource,
}

// The rules follow from the log (the creating event), so they are hashed with the other fields once, here.
impl Hash for Delayed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.source.hash(state);
        self.snapshot.hash(state);
        self.controller.hash(state);
        self.ability.hash(state);
        self.rules.hash(state);
        self.bindings.hash(state);
        self.once.hash(state);
        self.duration.hash(state);
        self.turn.hash(state);
        // Left out while there is none, so the hash of a game without epic is the one it always had.
        if let Some(kept) = &self.kept {
            kept.hash(state);
        }
        if !self.origin.is_printed() {
            self.origin.hash(state);
        }
    }
}

impl Delayed {
    /// The name it goes by.
    pub const fn id(&self) -> DelayedId {
        self.id
    }

    /// The object that set it up.
    pub const fn source(&self) -> ObjectRef {
        self.source
    }

    /// Who controls it.
    pub const fn controller(&self) -> PlayerId {
        self.controller
    }

    /// Where the ability that set it up comes from.
    pub const fn origin(&self) -> AbilitySource {
        self.origin
    }

    /// The event it waits for.
    pub fn pattern(&self) -> Option<&TriggerPattern> {
        match &*self.rules {
            Ability::Triggered { when, .. } => Some(when),
            _ => None,
        }
    }

    /// Its intervening "if", if it has one (CR 603.4).
    pub fn intervening_if(&self) -> Option<&Condition> {
        match &*self.rules {
            Ability::Triggered { intervening_if, .. } => intervening_if.as_ref(),
            _ => None,
        }
    }

    /// The spell that set it up, as it resolved, when the ability copies it (epic).
    pub fn kept(&self) -> Option<&KeptSpell> {
        self.kept.as_deref()
    }

    /// How long it lasts: until it triggers (`Forever`, for one that triggers once) or the end of the turn.
    pub const fn duration(&self) -> &Duration {
        &self.duration
    }

    /// Whether its duration has run out in `state`, so it no longer waits for anything.
    ///
    /// A delayed trigger of a player who has left the game is gone with them.
    pub fn lapsed(&self, state: &GameState) -> bool {
        if !state.is_alive(self.controller) {
            return true;
        }
        let turn = state.turn();
        match &self.duration {
            Duration::Forever => false,
            Duration::UntilEndOfTurn => turn.number() != self.turn,
            Duration::UntilEndOfCombat => {
                turn.number() != self.turn || turn.step() > Step::EndCombat
            }
            // No delayed trigger is made for the end of a later turn; it ends when that turn begins.
            Duration::UntilYourNextTurn | Duration::UntilEndOfYourNextTurn => {
                turn.number() != self.turn && state.is_active(self.controller)
            }
            Duration::UntilEndOfCombatOnYourNextTurn => {
                turn.number() != self.turn
                    && turn.active() == self.controller
                    && turn.step() > Step::EndCombat
            }
            Duration::UntilYourNextEndStep => {
                turn.number() != self.turn
                    && state.is_active(self.controller)
                    && turn.step() >= Step::End
            }
            Duration::UntilYourNextUpkeep => {
                turn.number() != self.turn
                    && state.is_active(self.controller)
                    && turn.step() >= Step::Upkeep
            }
            Duration::UntilEndOfYourNextUpkeep => {
                turn.number() != self.turn
                    && state.is_active(self.controller)
                    && turn.step() > Step::Upkeep
            }
            // Ends once the player the name was bound to has finished a turn after the one it was set up in.
            Duration::R6700UntilEndOfTheirNextTurn(who) => {
                let player = match who {
                    mtg_cards::refs::Ref::Bound(name) => {
                        self.bindings.get(name).and_then(|bound| {
                            bound.iter().find_map(|target| match target {
                                crate::choose::Target::Player(player) => Some(*player),
                                crate::choose::Target::Object(_) => None,
                            })
                        })
                    }
                    _ => None,
                };
                player.is_none_or(|player| {
                    !state.is_alive(player)
                        || state
                            .r379_since
                            .ended_turn(player)
                            .is_some_and(|ended| ended > self.turn)
                })
            }
            Duration::WhileSourceOnBattlefield => !state.object(self.source.id).is_some_and(|o| {
                o.counter() == self.source.counter && o.zone().is_permanent_zone()
            }),
            // Only an exile lasts until an opponent becomes the monarch and only a continuous effect while its controller
            // controls the source, and only a permanent that doesn't untap waits for an untap step; a delayed trigger never
            // does.
            Duration::UntilOpponentBecomesMonarch
            | Duration::R109ThatCombat(_)
            | Duration::DuringItsControllersNextTurn
            | Duration::UntilItsControllersNextCombatEnds
            | Duration::WhileYouControlSource
            | Duration::WhileSourceTapped
            | Duration::WhileSourceAttached
            | Duration::WhileYouControlSourceTapped
            | Duration::WhileHasCounter(_)
            | Duration::WhileEnchanted
            | Duration::WhileTapped
            | Duration::WhileMonarch(_)
            | Duration::R113While(_)
            | Duration::UntilItsControllersNextUntapStep
            | Duration::NextUntapStep
            | Duration::UntilPaid(_)
            | Duration::UntilTurnSkipped
            | Duration::UntilSkipped
            | Duration::UntilSkippedThisTurn
            | Duration::TheirNextTurn
            | Duration::UntilTheirNextTurn
            | Duration::R113UntilTheirNextEndStep
            | Duration::UntilCastFromExile
            | Duration::UntilCreatureSpellCast
            | Duration::UntilEndOfTurnOrRolled(_) => true,
        }
    }
}

/// Sets the delayed trigger up.
pub(crate) fn fold_created(
    state: &mut GameState,
    created: &DelayedCreated,
) -> Result<(), EngineError> {
    if state.delayed.iter().any(|d| d.id == created.id) {
        return Err(invalid(format!("{:?} is set up twice", created.id)));
    }
    let mut bindings = Bindings::new();
    for (name, targets) in &created.bindings {
        bindings.insert(name.clone(), targets.clone());
    }
    state.next_delayed = state.next_delayed.max(created.id.0 + 1);
    state.delayed.push(Delayed {
        id: created.id,
        source: created.source,
        snapshot: created.snapshot.clone(),
        controller: created.controller,
        ability: created.ability,
        rules: Arc::new(Ability::Triggered {
            when: created.when.clone(),
            intervening_if: created.intervening_if.clone(),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: created.targets.clone(),
            effects: created.effects.clone(),
        }),
        bindings,
        once: created.once,
        duration: if let Some(until) = &created.until {
            until.clone()
        } else if created.once && !created.this_turn {
            Duration::Forever
        } else {
            Duration::UntilEndOfTurn
        },
        turn: state.turn.number,
        kept: created.kept.as_deref().cloned().map(Arc::new),
        origin: created.origin,
    });
    Ok(())
}

/// Gives each delayed trigger a batch sets up an id no other has, as the batch is folded: the effects that set them up
/// read the state before the batch (replacements of several events of one batch, CR 614.1a: a shield exiling each
/// creature a wipe destroys, copies entering together), so two of them can name the same next id. The events of the
/// batch after one renamed that name its old id (its trigger, its removal) name the new one.
#[derive(Debug, Default)]
pub(crate) struct FreshDelayedIds(std::collections::BTreeMap<DelayedId, DelayedId>);

impl FreshDelayedIds {
    /// The event with its delayed trigger renamed, when it is one to rename; `None` when it stays as it is.
    pub(crate) fn renamed(&mut self, state: &GameState, event: &Event) -> Option<Event> {
        match event {
            Event::DelayedTriggerCreated(created) if created.id.0 < state.next_delayed => {
                let fresh = DelayedId(state.next_delayed);
                self.0.insert(created.id, fresh);
                let mut created = created.clone();
                created.id = fresh;
                Some(Event::DelayedTriggerCreated(created))
            }
            Event::AbilityTriggered(triggered) => {
                let fresh = self.fresh(triggered.delayed)?;
                let mut triggered = triggered.clone();
                triggered.delayed = Some(fresh);
                Some(Event::AbilityTriggered(triggered))
            }
            Event::TriggerRemoved { delayed, .. } => {
                let fresh = self.fresh(*delayed)?;
                let mut event = event.clone();
                if let Event::TriggerRemoved { delayed, .. } = &mut event {
                    *delayed = Some(fresh);
                }
                Some(event)
            }
            Event::DelayedTriggerExpired { id } => Some(Event::DelayedTriggerExpired {
                id: self.fresh(Some(*id))?,
            }),
            _ => None,
        }
    }

    /// The new id of a renamed delayed trigger.
    fn fresh(&self, id: Option<DelayedId>) -> Option<DelayedId> {
        if self.0.is_empty() {
            return None;
        }
        self.0.get(&id?).copied()
    }
}

/// Drops a delayed trigger whose duration has run out.
pub(crate) fn fold_expired(state: &mut GameState, id: DelayedId) -> Result<(), EngineError> {
    let before = state.delayed.len();
    state.delayed.retain(|d| d.id != id);
    if state.delayed.len() == before {
        return Err(invalid(format!("{id:?} is not waiting")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Goal: a delayed trigger runs out exactly when its duration says (the turn it was set up in, the end of combat, the controller's next turn, the source leaving, or its controller leaving the game), is housekept away by an event of its own, carries the names it took into the resolution of the ability it becomes, and cannot be set up or dropped twice.
    use mtg_cards::refs::{Ref, ZoneKind};
    use mtg_cards::trigger::TriggerPattern;
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::abilities::{Cause, PutOnStack};
    use crate::choose::{Choices, Target};
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;
    use crate::triggers::Triggered;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn created(
        state: &GameState,
        source: ObjectId,
        bindings: Vec<(String, Vec<Target>)>,
    ) -> DelayedCreated {
        DelayedCreated {
            id: DelayedId(0),
            source: state.object(source).unwrap().reference(),
            snapshot: snapshot(state, source).unwrap(),
            controller: ME,
            ability: 0,
            when: TriggerPattern::StepBegins {
                step: mtg_cards::trigger::Step::End,
                whose: Ref::EachPlayer,
            },
            once: false,
            this_turn: false,
            until: None,
            effects: vec![mtg_cards::effect::Effect::GainLife {
                who: Ref::You,
                amount: mtg_cards::value::Amount::Fixed(1),
            }],
            targets: Vec::new(),
            bindings,
            intervening_if: None,
            kept: None,
            origin: crate::abilities::AbilitySource::default(),
        }
    }

    fn table() -> (GameState, ObjectId) {
        let state = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let source = state.zone(Zone::Battlefield)[0];
        (state, source)
    }

    fn set_up(state: &mut GameState, source: ObjectId, duration: Duration) {
        let mut event = created(state, source, Vec::new());
        // `once` decides the duration of a delayed trigger from the card data; the others are for the engine's own use.
        event.once = duration == Duration::Forever;
        apply_event(state, &Event::DelayedTriggerCreated(Box::new(event))).unwrap();
        state.delayed[0].duration = duration;
    }

    fn next_turn(state: &mut GameState, active: PlayerId) {
        let number = state.turn().number() + 1;
        apply_event(
            state,
            &Event::TurnBegan {
                number,
                active,
                extra: false,
            },
        )
        .unwrap();
    }

    #[rstest]
    #[case::forever(Duration::Forever, false)]
    #[case::this_turn(Duration::UntilEndOfTurn, true)]
    #[case::my_next_turn(Duration::UntilYourNextTurn, false)]
    fn after_the_other_players_turn_begins_each_duration_says_whether_it_ran_out(
        #[case] duration: Duration,
        #[case] lapsed: bool,
    ) {
        let (mut state, source) = table();
        set_up(&mut state, source, duration);
        assert!(!state.delayed()[0].lapsed(&state), "not at once");
        next_turn(&mut state, FOE);
        assert_eq!(state.delayed()[0].lapsed(&state), lapsed);
    }

    /// A delayed trigger "until the end of defending player's next turn" set up in `ME`'s turn, the name bound to `FOE`.
    fn r6700_set_up_for_foe(state: &mut GameState, source: ObjectId) {
        let mut event = created(
            state,
            source,
            vec![("defender".to_owned(), vec![Target::Player(FOE)])],
        );
        event.until = Some(Duration::R6700UntilEndOfTheirNextTurn(Ref::Bound(
            "defender".to_owned(),
        )));
        apply_event(state, &Event::DelayedTriggerCreated(Box::new(event))).unwrap();
    }

    #[rstest]
    #[case::through_the_turn_it_began_in(&[], false)]
    #[case::through_that_players_next_turn(&[FOE], false)]
    #[case::not_past_that_players_next_turn(&[FOE, ME], true)]
    #[case::not_a_turn_of_another_player(&[ME], false)]
    fn r6700_it_waits_until_the_end_of_that_players_next_turn(
        #[case] turns: &[PlayerId],
        #[case] lapsed: bool,
    ) {
        let (mut state, source) = table();
        next_turn(&mut state, ME);
        r6700_set_up_for_foe(&mut state, source);
        for active in turns {
            next_turn(&mut state, *active);
        }
        assert_eq!(state.delayed()[0].lapsed(&state), lapsed);
    }

    #[rstest]
    #[case::once_for_ever(true, false, false)]
    #[case::once_this_turn(true, true, true)]
    #[case::each_time_this_turn(false, false, true)]
    #[case::each_time_said_this_turn(false, true, true)]
    fn the_card_data_decide_whether_it_waits_past_the_turn(
        #[case] once: bool,
        #[case] this_turn: bool,
        #[case] lapsed: bool,
    ) {
        let (mut state, source) = table();
        let mut event = created(&state, source, Vec::new());
        (event.once, event.this_turn) = (once, this_turn);
        apply_event(&mut state, &Event::DelayedTriggerCreated(Box::new(event))).unwrap();
        assert!(!state.delayed()[0].lapsed(&state), "not at once");
        next_turn(&mut state, FOE);
        assert_eq!(state.delayed()[0].lapsed(&state), lapsed);
    }

    #[test]
    fn until_your_next_turn_runs_out_as_that_turn_begins_not_before() {
        let (mut state, source) = table();
        set_up(&mut state, source, Duration::UntilYourNextTurn);
        next_turn(&mut state, FOE);
        assert!(!state.delayed()[0].lapsed(&state));
        next_turn(&mut state, ME);
        assert!(state.delayed()[0].lapsed(&state));
    }

    #[test]
    fn until_end_of_combat_runs_out_after_the_end_of_combat_step_or_with_the_turn() {
        let (mut state, source) = table();
        set_up(&mut state, source, Duration::UntilEndOfCombat);
        apply_event(
            &mut state,
            &Event::StepBegan {
                step: crate::turn::Step::EndCombat,
            },
        )
        .unwrap();
        assert!(!state.delayed()[0].lapsed(&state), "still in the combat");
        apply_event(
            &mut state,
            &Event::StepBegan {
                step: crate::turn::Step::Main2,
            },
        )
        .unwrap();
        assert!(state.delayed()[0].lapsed(&state));
    }

    #[test]
    fn while_the_source_stays_runs_out_when_it_leaves_the_battlefield_and_with_a_player_who_leaves()
    {
        let (mut state, source) = table();
        set_up(&mut state, source, Duration::WhileSourceOnBattlefield);
        assert!(!state.delayed()[0].lapsed(&state));
        let mut gone = state.clone();
        apply_event(
            &mut gone,
            &Event::ZoneChanged {
                object: source,
                from: Zone::Battlefield,
                to: Zone::Graveyard(ME),
                cause: crate::events::ZoneCause::Moved,
            },
        )
        .unwrap();
        assert!(gone.delayed()[0].lapsed(&gone));
        // Whatever its duration, a player who leaves the game takes their delayed triggers with them.
        let (mut state, source) = table();
        set_up(&mut state, source, Duration::Forever);
        apply_event(
            &mut state,
            &Event::PlayerLost {
                player: ME,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        assert!(state.delayed()[0].lapsed(&state));
    }

    #[test]
    fn housekeeping_drops_what_ran_out_with_an_event_and_the_event_cannot_be_repeated() {
        let (mut state, source) = table();
        set_up(&mut state, source, Duration::UntilEndOfTurn);
        next_turn(&mut state, FOE);
        let events = crate::sba::housekeeping(&state);
        assert_eq!(events, [Event::DelayedTriggerExpired { id: DelayedId(0) }]);
        apply_event(&mut state, &events[0]).unwrap();
        assert_eq!(state.delayed(), []);
        assert!(apply_event(&mut state, &events[0]).is_err());
        assert_eq!(crate::sba::housekeeping(&state), []);
    }

    #[test]
    fn a_delayed_trigger_is_not_set_up_twice() {
        let (mut state, source) = table();
        set_up(&mut state, source, Duration::Forever);
        let again = created(&state, source, Vec::new());
        assert!(apply_event(&mut state, &Event::DelayedTriggerCreated(Box::new(again))).is_err());
    }

    #[test]
    fn the_ability_it_becomes_starts_its_resolution_with_the_names_the_setup_had_bound() {
        let (mut state, source) = table();
        let kept = vec![("kept".to_owned(), vec![Target::Player(FOE)])];
        let event = created(&state, source, kept.clone());
        apply_event(&mut state, &Event::DelayedTriggerCreated(Box::new(event))).unwrap();
        let object = state.object(source).unwrap();
        let triggered = Triggered {
            origin: crate::abilities::AbilitySource::default(),
            source: object.reference(),
            ability: 0,
            controller: ME,
            cause: Cause::default(),
            snapshot: snapshot(&state, source).unwrap(),
            delayed: Some(DelayedId(0)),
        };
        apply_event(
            &mut state,
            &Event::AbilityTriggered(Box::new(triggered.clone())),
        )
        .unwrap();
        let put = PutOnStack {
            origin: crate::abilities::AbilitySource::default(),
            object: state.next_object(),
            source: triggered.source,
            snapshot: triggered.snapshot.clone(),
            ability: 0,
            controller: ME,
            choices: Choices::default(),
            cause: Some(Cause::default()),
            delayed: Some(DelayedId(0)),
        };
        let ability = put.object;
        apply_event(&mut state, &Event::AbilityPutOnStack(Box::new(put))).unwrap();
        apply_event(&mut state, &Event::ResolutionBegan { object: ability }).unwrap();
        let resolving = state.resolving().unwrap();
        assert_eq!(resolving.bindings().get("kept"), Some(&kept[0].1));
        let rules = state.object(ability).unwrap().ability().unwrap().rules();
        assert!(matches!(
            rules,
            Ability::Triggered {
                from: ZoneKind::Battlefield,
                ..
            }
        ));
    }

    /// Folds `events` as the engine commits a batch, renaming the delayed triggers that need a fresh id.
    fn fold_batch(state: &mut GameState, events: Vec<Event>) -> Vec<Event> {
        let mut fresh = FreshDelayedIds::default();
        events
            .into_iter()
            .map(|event| {
                let event = fresh.renamed(state, &event).unwrap_or(event);
                apply_event(state, &event).unwrap();
                event
            })
            .collect()
    }

    #[rstest]
    fn each_delayed_trigger_of_a_batch_that_names_one_id_gets_its_own_and_its_trigger_follows(
        #[values(0, 1)] live: usize,
        #[values(2, 3, 5)] copies: usize,
    ) {
        let (mut state, source) = table();
        for _ in 0..live {
            set_up(&mut state, source, Duration::Forever);
        }
        let object = state.object(source).unwrap().reference();
        let triggered = Triggered {
            origin: crate::abilities::AbilitySource::default(),
            source: object,
            ability: 0,
            controller: ME,
            cause: Cause::default(),
            snapshot: snapshot(&state, source).unwrap(),
            delayed: Some(DelayedId(0)),
        };
        // Each copy of the batch was worked out from the state before it: every one names id 0.
        let batch: Vec<Event> = (0..copies)
            .flat_map(|_| {
                [
                    Event::DelayedTriggerCreated(Box::new(created(&state, source, Vec::new()))),
                    Event::AbilityTriggered(Box::new(triggered.clone())),
                ]
            })
            .collect();
        let folded = fold_batch(&mut state, batch);
        let first = u64::try_from(live).unwrap();
        let want: Vec<u64> = (first..first + u64::try_from(copies).unwrap()).collect();
        let made: Vec<u64> = folded
            .iter()
            .filter_map(|event| match event {
                Event::DelayedTriggerCreated(created) => Some(created.id.0),
                _ => None,
            })
            .collect();
        let fired: Vec<u64> = folded
            .iter()
            .filter_map(|event| match event {
                Event::AbilityTriggered(triggered) => triggered.delayed.map(|id| id.0),
                _ => None,
            })
            .collect();
        assert_eq!(made, want);
        assert_eq!(
            fired, want,
            "each trigger names the delayed trigger set up before it"
        );
        assert_eq!(state.delayed.len(), live + copies);
    }
}
