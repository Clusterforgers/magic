//! `ChooseTargets`: who and what a target specification may name, the decision that picks them, and the
//! recheck on resolution that uses the same rule (CR 115, 608.2b).

use mtg_cards::refs::{Pred, Selector, TargetSpec, Targetable};
use mtg_cards::value::Amount;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::{Choice, Choices, Target};
use crate::abilities::Cause;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::keywords;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{ResolveError, Scope};
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::wording;
use crate::zones::Zone;

/// [`options_for`] before X is chosen: a comparison with X is left out of the specification, since some X may allow
/// what the rest of it allows. The announcement asks the real question once X is known.
pub(crate) fn options_before_x(
    state: &GameState,
    source: ObjectId,
    controller: PlayerId,
    index: usize,
    spec: &TargetSpec,
    cause: Option<&Cause>,
) -> Option<TargetOptions> {
    let relaxed = TargetSpec {
        what: without_x(&spec.what),
        ..spec.clone()
    };
    options_for(
        state, source, controller, index, &relaxed, true, cause, None,
    )
}

/// Whether what `spec` may target depends on X: a comparison against X among its predicates.
pub(crate) fn reads_x(spec: &TargetSpec) -> bool {
    without_x(&spec.what) != spec.what
}

/// Whether every one of `specs` can be met when the X announced is `x` (CR 601.2c): each has enough legal targets.
pub(crate) fn targets_met(
    state: &GameState,
    source: ObjectId,
    controller: PlayerId,
    specs: &[TargetSpec],
    cause: Option<&Cause>,
    x: Option<u32>,
) -> bool {
    specs.iter().enumerate().all(|(index, spec)| {
        options_for(state, source, controller, index, spec, false, cause, x).is_some()
    })
}

/// `what` without the comparisons against X among its predicates.
fn without_x(what: &Targetable) -> Targetable {
    match what {
        Targetable::Object(selector) => Targetable::Object(Selector(
            selector
                .0
                .iter()
                .filter(|pred| !matches!(pred, Pred::Compare { to: Amount::X, .. }))
                .cloned()
                .collect(),
        )),
        Targetable::Either(kinds) => Targetable::Either(kinds.iter().map(without_x).collect()),
        other => other.clone(),
    }
}

/// The targets a player may choose for one target specification of a spell or ability being put on the stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TargetOptions {
    /// The spell or ability.
    pub source: ObjectId,
    /// Which of its target specifications this is, from 0.
    pub spec: usize,
    /// Everything that may be chosen.
    pub legal: Vec<Target>,
    /// The fewest targets to choose.
    pub min: u32,
    /// The most targets to choose, never more than there are to choose from.
    pub max: u32,
    /// Whether the cast or activation may still be backed out of: nothing is logged until it is paid for.
    pub can_cancel: bool,
    /// What the specification asks for in the words of a card: "target creature you control".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub wording: String,
    /// Whether the targets are for a triggered ability that goes on the stack, which cannot be backed out of: a player
    /// who chooses none removes it (CR 603.3d).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub triggered: bool,
}

/// The answer to a target decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum TargetChoice {
    /// These targets, each once, between `min` and `max` of them, all among the legal ones.
    Targets(Vec<Target>),
    /// Back out of the cast or activation (when `can_cancel`).
    Cancel,
    /// Choose no target for a triggered ability, which is then removed instead of going on the stack (when `triggered`).
    Remove,
}

fn candidates(scope: &Scope<'_>, what: &Targetable) -> Result<Vec<Target>, ResolveError> {
    let state = scope.state;
    let players = || state.alive_players().map(Target::Player);
    Ok(match what {
        Targetable::AnyTarget => players()
            .chain(
                state
                    .zone(Zone::Battlefield)
                    .iter()
                    .filter_map(|id| state.object(*id))
                    .filter(|o| scope.can_be_dealt_damage(o.reference()))
                    .map(|o| Target::Object(o.reference())),
            )
            .collect(),
        Targetable::Player => players().collect(),
        Targetable::Opponent => players()
            .filter(|t| t.player() != Some(scope.controller))
            .collect(),
        Targetable::Object(selector) => scope
            .matching(selector)?
            .into_iter()
            .map(Target::Object)
            .collect(),
        Targetable::Either(kinds) => {
            let mut all = Vec::new();
            for kind in kinds {
                all.extend(candidates(scope, kind)?);
            }
            all
        }
    })
}

/// Everything `spec` lets a spell or ability of `source`, controlled by `controller`, target right now.
///
/// One rule serves both choosing and the recheck on resolution (CR 608.2b): a target is still legal when it
/// is still in this list. An object that changed zones is a new object, so it is not. A selector the engine
/// cannot evaluate matches nothing, so the spell is not offered. A target its keywords protect from `controller`
/// (hexproof) is left out, so a recheck on resolution drops it too.
pub fn legal_targets(
    state: &GameState,
    spec: &TargetSpec,
    source: ObjectId,
    controller: PlayerId,
) -> Vec<Target> {
    legal_for(state, spec, source, controller, None, None)
}

/// [`legal_targets`] for an ability that triggered: what it triggered on is what "that player" in the specification
/// names. `x` is the X announced, which a filter may read.
pub(crate) fn legal_for(
    state: &GameState,
    spec: &TargetSpec,
    source: ObjectId,
    controller: PlayerId,
    cause: Option<&Cause>,
    x: Option<u32>,
) -> Vec<Target> {
    let source = state.object(source).map_or(
        ObjectRef {
            id: source,
            counter: 0,
        },
        GameObject::reference,
    );
    legal_from(state, spec, source, &[], controller, cause, x)
}

/// [`legal_for`] for a source named by its reference, with the snapshots that say how objects were when they have gone:
/// a resolving ability reads the source it came from as it was (CR 608.2h), which protection from a colour needs.
pub(crate) fn legal_from(
    state: &GameState,
    spec: &TargetSpec,
    source: ObjectRef,
    known: &[&Snapshot],
    controller: PlayerId,
    cause: Option<&Cause>,
    x: Option<u32>,
) -> Vec<Target> {
    let announced = Choices {
        x,
        ..Choices::default()
    };
    let mut scope = Scope::announcing(state, controller, source);
    scope.choices = &announced;
    scope.cause = cause;
    scope.snapshots = cause
        .iter()
        .flat_map(|c| c.snapshots.iter())
        .chain(known.iter().copied())
        .collect();
    let mut legal = candidates(&scope, &spec.what).unwrap_or_default();
    let origin = keywords::Source {
        object: source,
        snapshots: &scope.snapshots,
    };
    legal.retain(|target| match target {
        Target::Object(object) => {
            !keywords::protects(state, object.id, source.id, controller)
                && !keywords::protected_from(state, keywords::Holder::Object(object.id), origin)
        }
        Target::Player(player) => {
            !keywords::protects_player(state, *player, controller)
                && !keywords::protected_from(state, keywords::Holder::Player(*player), origin)
        }
    });
    legal.sort_unstable();
    legal.dedup();
    legal
}

/// Whether `target` is among the legal targets of `spec`.
pub fn is_legal(
    state: &GameState,
    spec: &TargetSpec,
    target: Target,
    source: ObjectId,
    controller: PlayerId,
) -> bool {
    legal_targets(state, spec, source, controller).contains(&target)
}

/// The decision for the specification `index`, or `None` when it cannot be met: fewer legal targets than its minimum.
// Every argument is a different thing the legal targets depend on; the callers each have them all.
#[allow(clippy::too_many_arguments)]
pub(crate) fn options_for(
    state: &GameState,
    source: ObjectId,
    controller: PlayerId,
    index: usize,
    spec: &TargetSpec,
    can_cancel: bool,
    cause: Option<&Cause>,
    x: Option<u32>,
) -> Option<TargetOptions> {
    let legal = legal_for(state, spec, source, controller, cause, x);
    let available = u32::try_from(legal.len()).unwrap_or(u32::MAX);
    if available < spec.bounds.min {
        return None;
    }
    let max = match &spec.bounds.max {
        None => available,
        Some(Amount::Fixed(n)) => u32::try_from(*n).unwrap_or(0),
        Some(other) => {
            let origin = state.object(source).map_or(
                ObjectRef {
                    id: source,
                    counter: 0,
                },
                GameObject::reference,
            );
            let announced = Choices {
                x,
                ..Choices::default()
            };
            let mut scope = Scope::announcing(state, controller, origin);
            scope.choices = &announced;
            let amount = scope.amount(other).ok()?;
            u32::try_from(amount).unwrap_or(0)
        }
    };
    // Every target of a divided amount gets at least one (CR 601.2d), so there are never more targets than the amount.
    let max = match &spec.divide {
        Some(Amount::Fixed(n)) => max.min(u32::try_from(*n).unwrap_or(0)),
        _ => max,
    };
    Some(TargetOptions {
        source,
        spec: index,
        legal,
        min: spec.bounds.min,
        max: max.min(available),
        can_cancel,
        wording: wording::spec(spec),
        triggered: false,
    })
}

/// Whether the specification asks for any choice at all: a specification that allows no targets, or has none
/// to offer and needs none, is passed over.
pub(crate) fn asks(options: &TargetOptions) -> bool {
    options.max > 0 && !options.legal.is_empty()
}

/// Checks the answer against the options it answers. While a cast or activation is being announced the targets are
/// kept for it; otherwise they are the new targets of a copy and are logged as the chooser's answer.
pub(crate) fn answer(
    player: PlayerId,
    options: &TargetOptions,
    choice: TargetChoice,
    announcing: bool,
) -> Result<Outcome, EngineError> {
    let targets = match choice {
        TargetChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        TargetChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        TargetChoice::Remove if options.triggered => return Ok(Outcome::DropTrigger),
        TargetChoice::Remove => return Err(IllegalAnswer::NotOffered("remove").into()),
        TargetChoice::Targets(targets) => targets,
    };
    for (at, target) in targets.iter().enumerate() {
        if targets[..at].contains(target) {
            return Err(IllegalAnswer::Duplicate("target").into());
        }
        if !options.legal.contains(target) {
            return Err(IllegalAnswer::NotOffered("target").into());
        }
    }
    let count = u32::try_from(targets.len()).unwrap_or(u32::MAX);
    let wanted = if count < options.min {
        Some(options.min)
    } else if count > options.max {
        Some(options.max)
    } else {
        None
    };
    if let Some(expected) = wanted {
        return Err(IllegalAnswer::WrongCount {
            expected: usize::try_from(expected).unwrap_or(usize::MAX),
            got: targets.len(),
        }
        .into());
    }
    if announcing {
        return Ok(Outcome::Targets(targets));
    }
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Targets(targets),
    }]))
}

#[cfg(test)]
mod tests {
    //! Goal: what a specification lets a spell target is the same list for choosing and for the recheck: players and creatures, opponents, selectors, unions, never what has left the game or changed zones, and the answer to a target decision is accepted exactly when it is built from that list within the bounds.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::player::LossReason;

    fn relic() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact))
    }

    /// Seat 0: a Bear, a Relic and a spell in hand; seat 1: an Ogre.
    fn game() -> GameState {
        state(
            vec![vec![relic()], vec![]],
            vec![
                vec![creature("Bear", 2, 2), relic()],
                vec![creature("Ogre", 3, 4)],
            ],
        )
    }

    fn spec(json: &str) -> TargetSpec {
        serde_json::from_str(json).unwrap()
    }

    fn describe(game: &GameState, targets: &[Target]) -> Vec<String> {
        targets
            .iter()
            .map(|t| match t {
                Target::Player(p) => format!("player {}", p.get()),
                Target::Object(o) => game.object(o.id).unwrap().card().name.clone(),
            })
            .collect()
    }

    /// The source is the Relic in seat 0's hand, standing for a spell.
    fn legal(game: &GameState, json: &str) -> Vec<String> {
        let spell = game.zone(Zone::Hand(PlayerId(0)))[0];
        describe(game, &legal_targets(game, &spec(json), spell, PlayerId(0)))
    }

    #[rstest]
    #[case::any_target(r#"{"what":"AnyTarget"}"#, &["player 0", "player 1", "Bear", "Ogre"])]
    #[case::a_player(r#"{"what":"Player"}"#, &["player 0", "player 1"])]
    #[case::an_opponent(r#"{"what":"Opponent"}"#, &["player 1"])]
    #[case::creatures(r#"{"what":{"Object":[{"Type":"Creature"}]}}"#, &["Bear", "Ogre"])]
    #[case::your_creatures(r#"{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#, &["Bear"])]
    #[case::artifacts(r#"{"what":{"Object":[{"Type":"Artifact"}]}}"#, &["Relic"])]
    #[case::a_player_or_an_artifact(r#"{"what":{"Either":["Player",{"Object":[{"Type":"Artifact"}]}]}}"#, &["player 0", "player 1", "Relic"])]
    #[case::a_kind_listed_twice_is_listed_once(r#"{"what":{"Either":["Player","AnyTarget"]}}"#, &["player 0", "player 1", "Bear", "Ogre"])]
    #[case::cards_in_a_hand(r#"{"what":{"Object":[{"In":"Hand"}]}}"#, &["Relic"])]
    #[case::a_selector_the_engine_cannot_read_matches_nothing(r#"{"what":{"Object":[{"Compare":{"stat":"Loyalty","op":"Ge","to":1}}]}}"#, &[])]
    fn a_specification_lists_what_it_lets_a_spell_target(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(legal(&game(), json), expected);
    }

    #[test]
    fn a_player_who_lost_is_not_a_target_and_neither_is_an_object_that_changed_zones() {
        let mut game = game();
        let ogre = game.zone(Zone::Battlefield)[2];
        let any = spec(r#"{"what":"AnyTarget"}"#);
        let spell = game.zone(Zone::Hand(PlayerId(0)))[0];
        let before = legal_targets(&game, &any, spell, PlayerId(0));
        assert!(before.contains(&Target::Object(game.object(ogre).unwrap().reference())));
        let old = game.object(ogre).unwrap().reference();
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: ogre,
                from: Zone::Battlefield,
                to: Zone::Graveyard(PlayerId(1)),
                cause: ZoneCause::Destroyed,
            },
        )
        .unwrap();
        apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: LossReason::Conceded,
            },
        )
        .unwrap();
        let after = legal_targets(&game, &any, spell, PlayerId(0));
        assert_eq!(describe(&game, &after), ["player 0", "Bear"]);
        assert!(!is_legal(
            &game,
            &any,
            Target::Object(old),
            spell,
            PlayerId(0)
        ));
        assert!(!is_legal(
            &game,
            &any,
            Target::Player(PlayerId(1)),
            spell,
            PlayerId(0)
        ));
        assert!(is_legal(
            &game,
            &any,
            Target::Player(PlayerId(0)),
            spell,
            PlayerId(0)
        ));
    }

    #[test]
    fn a_stale_reference_is_not_the_object_it_names() {
        let game = game();
        let spell = game.zone(Zone::Hand(PlayerId(0)))[0];
        let any = spec(r#"{"what":"AnyTarget"}"#);
        let bear = game
            .object(game.zone(Zone::Battlefield)[0])
            .unwrap()
            .reference();
        assert!(is_legal(
            &game,
            &any,
            Target::Object(bear),
            spell,
            PlayerId(0)
        ));
        let stale = ObjectRef {
            counter: bear.counter + 1,
            ..bear
        };
        assert!(!is_legal(
            &game,
            &any,
            Target::Object(stale),
            spell,
            PlayerId(0)
        ));
    }

    #[rstest]
    #[case::exactly_one(r#"{"what":"AnyTarget"}"#, Some((1, 1)))]
    #[case::up_to_two(r#"{"what":"AnyTarget","bounds":{"min":0,"max":2}}"#, Some((0, 2)))]
    #[case::any_number(r#"{"what":"AnyTarget","bounds":{"min":0,"max":null}}"#, Some((0, 4)))]
    #[case::more_than_there_are(r#"{"what":"AnyTarget","bounds":{"min":1,"max":9}}"#, Some((1, 4)))]
    #[case::too_few_to_meet_the_minimum(r#"{"what":"Opponent","bounds":{"min":2,"max":2}}"#, None)]
    #[case::none_to_choose_from_but_none_needed(r#"{"what":{"Object":[{"Type":"Land"}]},"bounds":{"min":0,"max":1}}"#, Some((0, 0)))]
    #[case::none_to_choose_from_and_one_needed(r#"{"what":{"Object":[{"Type":"Land"}]}}"#, None)]
    fn the_decision_offers_the_bounds_within_what_is_legal(
        #[case] json: &str,
        #[case] expected: Option<(u32, u32)>,
    ) {
        let game = game();
        let spell = game.zone(Zone::Hand(PlayerId(0)))[0];
        let options = options_for(&game, spell, PlayerId(0), 0, &spec(json), true, None, None);
        assert_eq!(options.map(|o| (o.min, o.max)), expected);
    }

    fn options(min: u32, max: u32, can_cancel: bool) -> TargetOptions {
        TargetOptions {
            source: ObjectId(0),
            spec: 0,
            legal: vec![
                Target::Player(PlayerId(0)),
                Target::Player(PlayerId(1)),
                Target::Object(ObjectRef {
                    id: ObjectId(5),
                    counter: 0,
                }),
            ],
            min,
            max,
            can_cancel,
            wording: String::new(),
            triggered: false,
        }
    }

    fn answered(options: &TargetOptions, choice: TargetChoice) -> Result<Outcome, IllegalAnswer> {
        match answer(PlayerId(0), options, choice, true) {
            Err(EngineError::Illegal(illegal)) => Err(illegal),
            Err(other) => panic!("{other}"),
            Ok(outcome) => Ok(outcome),
        }
    }

    #[rstest]
    #[case::one_target(1, 1, vec![0], true)]
    #[case::the_last_one(1, 1, vec![2], true)]
    #[case::two_when_two_are_allowed(1, 2, vec![0, 1], true)]
    #[case::none_when_none_are_needed(0, 1, vec![], true)]
    #[case::none_when_one_is_needed(1, 1, vec![], false)]
    #[case::too_many(1, 1, vec![0, 1], false)]
    #[case::the_same_twice(1, 2, vec![1, 1], false)]
    fn targets_are_accepted_when_legal_distinct_and_within_the_bounds(
        #[case] min: u32,
        #[case] max: u32,
        #[case] picks: Vec<usize>,
        #[case] accepted: bool,
    ) {
        let options = options(min, max, true);
        let chosen: Vec<Target> = picks.iter().map(|at| options.legal[*at]).collect();
        let result = answered(&options, TargetChoice::Targets(chosen.clone()));
        match (accepted, result) {
            (true, Ok(Outcome::Targets(taken))) => assert_eq!(taken, chosen),
            (false, Err(_)) => {}
            (_, other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_target_that_was_not_offered_is_refused() {
        let options = options(1, 1, true);
        let stranger = Target::Object(ObjectRef {
            id: ObjectId(9),
            counter: 0,
        });
        assert_eq!(
            answered(&options, TargetChoice::Targets(vec![stranger])).unwrap_err(),
            IllegalAnswer::NotOffered("target")
        );
    }

    #[test]
    fn backing_out_is_free_while_the_cast_can_be_cancelled_and_refused_after() {
        assert!(matches!(
            answered(&options(1, 1, true), TargetChoice::Cancel),
            Ok(Outcome::CancelCast)
        ));
        assert_eq!(
            answered(&options(1, 1, false), TargetChoice::Cancel).unwrap_err(),
            IllegalAnswer::NotOffered("cancel")
        );
    }

    #[rstest]
    #[case::something_to_choose(1, 1, 3, true)]
    #[case::nothing_to_choose(0, 0, 3, false)]
    #[case::nothing_to_choose_from(0, 1, 0, false)]
    fn a_specification_with_nothing_to_choose_is_passed_over(
        #[case] min: u32,
        #[case] max: u32,
        #[case] legal: usize,
        #[case] asked: bool,
    ) {
        let mut options = options(min, max, true);
        options.legal.truncate(legal);
        assert_eq!(asks(&options), asked);
    }
}
