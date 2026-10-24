//! Delayed triggered abilities (CR 603.7): an effect sets one up, and it triggers on a later event or step.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::refs::ZoneKind;
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::choose::Target;
use crate::error::EngineError;
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
    /// What it does.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub effects: Vec<Effect>,
    /// The names the resolution had bound when it set the trigger up, which the delayed ability reads as they were.
    pub bindings: Vec<(String, Vec<Target>)>,
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

    /// The event it waits for.
    pub fn pattern(&self) -> Option<&TriggerPattern> {
        match &*self.rules {
            Ability::Triggered { when, .. } => Some(when),
            _ => None,
        }
    }

    /// How long it lasts: until it triggers (`Forever`, for one that triggers once) or the end of the turn.
    pub const fn duration(&self) -> Duration {
        self.duration
    }

    /// Whether its duration has run out in `state`, so it no longer waits for anything.
    ///
    /// A delayed trigger of a player who has left the game is gone with them.
    pub fn lapsed(&self, state: &GameState) -> bool {
        if !state.is_alive(self.controller) {
            return true;
        }
        let turn = state.turn();
        match self.duration {
            Duration::Forever => false,
            Duration::UntilEndOfTurn => turn.number() != self.turn,
            Duration::UntilEndOfCombat => {
                turn.number() != self.turn || turn.step() > Step::EndCombat
            }
            // No delayed trigger is made for the end of a later turn; it ends when that turn begins.
            Duration::UntilYourNextTurn | Duration::UntilEndOfYourNextTurn => {
                turn.number() != self.turn && turn.active() == self.controller
            }
            Duration::WhileSourceOnBattlefield => !state.object(self.source.id).is_some_and(|o| {
                o.counter() == self.source.counter && o.zone().is_permanent_zone()
            }),
            // Only an exile lasts until an opponent becomes the monarch; a delayed trigger never does.
            Duration::UntilOpponentBecomesMonarch => true,
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
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: created.effects.clone(),
        }),
        bindings,
        once: created.once,
        duration: if created.once {
            Duration::Forever
        } else {
            Duration::UntilEndOfTurn
        },
        turn: state.turn.number,
    });
    Ok(())
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
            effects: vec![mtg_cards::effect::Effect::GainLife {
                who: Ref::You,
                amount: mtg_cards::value::Amount::Fixed(1),
            }],
            bindings,
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
}
