//! Prohibitions (`GameRule::Cant`): what players or objects can't do, read in one place (`forbidden`).
//!
//! The places that offer an action (activation, land play, the candidates of a cost, the legal targets) ask `forbidden`
//! with the action they are about to offer, so a forbidden action is not offered and a stale answer is refused. The
//! replacement step drops the events an effect proposes that the rules forbid (`filter`): an effect that cannot happen
//! does not happen, and a game that needs it is never stuck. Turn flow asks for skipped steps and turns. A new
//! prohibition is one `Prohibited` entry, one `Doing` entry and one arm of `applies`.

use crate::triggers::step_matches;
use crate::turn::Step;
use mtg_cards::modify::Prohibited;
use mtg_cards::refs::Selector;
use mtg_cards::value::{Condition, TallyEvent};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::object::{GameObject, ObjectRef};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// A prohibition in force: whose it is, whom it is about and what it forbids.
#[derive(Debug, Clone)]
pub(crate) struct Banned {
    /// The controller of the effect or permanent that forbids, for the selectors' "you".
    pub(crate) controller: PlayerId,
    /// The object that forbids, for the selectors' "this"; none for a rule that no object began.
    pub(crate) source: Option<ObjectRef>,
    /// The players and objects it is about.
    pub(crate) who: Vec<Target>,
    /// What they can't do.
    pub(crate) action: Prohibited,
}

impl Banned {
    /// The prohibition of `action` for `who`, as the rule of `controller` and `source` puts it in force.
    pub(crate) fn new(
        who: &[Target],
        action: &Prohibited,
        controller: PlayerId,
        source: Option<ObjectRef>,
    ) -> Banned {
        Banned {
            controller,
            source,
            who: who.to_vec(),
            action: action.clone(),
        }
    }
}

/// Something a player or an object is about to do or have done.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Doing {
    /// `player` activates an ability of `source`.
    Activate { player: PlayerId, source: ObjectId },
    /// `target` becomes the target of a spell or ability of `by`, controlled by `controller`.
    BeTargeted { target: Target, by: ObjectRef },
    /// `player` plays a land.
    PlayLand { player: PlayerId },
    /// `player` sacrifices `object`.
    Sacrifice { player: PlayerId, object: ObjectId },
    /// Counters are put on `on`.
    PutCounter { on: Target },
    /// `object` becomes attached to `to`.
    Attach { object: ObjectId, to: Target },
    /// `player` discards `object`.
    Discard { player: PlayerId, object: ObjectId },
    /// `object` is exiled.
    Exile { object: ObjectId },
    /// `object` transforms.
    Transform { object: ObjectId },
    /// The turn of `player` skips `step`.
    SkipStep { player: PlayerId, step: Step },
    /// The turn of `player` is skipped.
    SkipTurn { player: PlayerId },
    /// `player` casts the card `card`.
    Cast { player: PlayerId, card: ObjectId },
}

/// Whether a prohibition's subjects cover the player or the object an action is about.
pub(crate) fn covers(who: &[Target], player: Option<PlayerId>, object: Option<ObjectId>) -> bool {
    who.iter().any(|subject| match subject {
        Target::Player(p) => Some(*p) == player,
        Target::Object(o) => Some(o.id) == object,
    })
}

/// Whether `selector` picks `source`, judged for the rule: an empty selector picks everything.
fn picks(
    state: &GameState,
    ban: &Banned,
    selector: &mtg_cards::refs::Selector,
    source: ObjectRef,
) -> bool {
    selector.is_empty()
        || Scope::announcing(state, ban.controller, ban.source.unwrap_or(source))
            .selects(selector, source)
            .unwrap_or(false)
}

/// The player an object's action is charged to: its controller on the battlefield, its owner elsewhere.
fn holder(state: &GameState, object: ObjectId) -> Option<PlayerId> {
    controller_of(state, object).or_else(|| state.object(object).map(GameObject::owner))
}

/// Whether one prohibition forbids the action: the one check of each entry of `Prohibited`.
fn applies(state: &GameState, ban: &Banned, doing: Doing) -> bool {
    let who = &ban.who;
    match (&ban.action, doing) {
        (Prohibited::Activate(sources), Doing::Activate { player, source }) => {
            covers(who, Some(player), Some(source))
                && state
                    .object(source)
                    .is_some_and(|o| picks(state, ban, sources, o.reference()))
        }
        (Prohibited::BeTargeted(by), Doing::BeTargeted { target, by: source }) => {
            let (player, object) = match target {
                Target::Player(p) => (Some(p), None),
                Target::Object(o) => (None, Some(o.id)),
            };
            covers(who, player, object) && picks(state, ban, by, source)
        }
        (Prohibited::PlayLand, Doing::PlayLand { player })
        | (Prohibited::SkipTurn, Doing::SkipTurn { player }) => covers(who, Some(player), None),
        (Prohibited::Sacrifice, Doing::Sacrifice { player, object })
        | (Prohibited::Discard, Doing::Discard { player, object }) => {
            covers(who, Some(player), Some(object))
        }
        (Prohibited::PutCounter, Doing::PutCounter { on }) => match on {
            Target::Player(p) => covers(who, Some(p), None),
            Target::Object(o) => covers(who, holder(state, o.id), Some(o.id)),
        },
        (Prohibited::Attach, Doing::Attach { object, to }) => {
            let hosted = match to {
                Target::Player(p) => covers(who, Some(p), None),
                Target::Object(o) => covers(who, holder(state, o.id), Some(o.id)),
            };
            hosted || covers(who, holder(state, object), Some(object))
        }
        (Prohibited::Exile, Doing::Exile { object }) => covers(
            who,
            state.object(object).map(GameObject::owner),
            Some(object),
        ),
        (Prohibited::Transform, Doing::Transform { object }) => {
            covers(who, holder(state, object), Some(object))
        }
        (Prohibited::SkipStep(skipped), Doing::SkipStep { player, step }) => {
            step_matches(*skipped, step) && covers(who, Some(player), None)
        }
        (
            Prohibited::Cast {
                filter,
                more_than,
                unless,
            },
            Doing::Cast { player, card },
        ) => {
            covers(who, Some(player), None)
                && casting_forbidden(
                    state,
                    ban,
                    (filter, *more_than, unless.as_ref()),
                    player,
                    card,
                )
        }
        _ => false,
    }
}

/// The parts of a cast prohibition: the filter, the spells a player may cast each turn and the exception.
type CastBan<'a> = (&'a Selector, Option<u32>, Option<&'a Condition>);

/// Whether a cast prohibition forbids `player` to cast `card`: the card matches the filter, the caster has cast as many
/// such spells this turn as the limit allows (any number when there is none) and the exception does not hold for them.
fn casting_forbidden(
    state: &GameState,
    ban: &Banned,
    (filter, limit, unless): CastBan<'_>,
    player: PlayerId,
    card: ObjectId,
) -> bool {
    let Some(object) = state.object(card) else {
        return false;
    };
    if !picks(state, ban, filter, object.reference()) {
        return false;
    }
    if let Some(limit) = limit {
        let scope = Scope::announcing(
            state,
            ban.controller,
            ban.source.unwrap_or(object.reference()),
        );
        let cast = scope
            .logged_for(TallyEvent::SpellCast, &[player], filter)
            .unwrap_or(0);
        if cast < i64::from(limit) {
            return false;
        }
    }
    !unless.is_some_and(|condition| {
        Scope::announcing(state, player, object.reference())
            .holds(condition)
            .unwrap_or(false)
    })
}

/// Whether a rule in force forbids `doing`: the one place every prohibition is read.
pub(crate) fn forbidden(state: &GameState, doing: Doing) -> bool {
    state
        .rules()
        .cants
        .iter()
        .any(|ban| applies(state, ban, doing))
}

/// The prohibitions of triggers in force: the one place trigger detection reads them.
pub(crate) fn trigger_bans(
    state: &GameState,
) -> impl Iterator<Item = (&Banned, &mtg_cards::trigger::TriggerPattern)> {
    state
        .rules()
        .cants
        .iter()
        .filter_map(|ban| match &ban.action {
            Prohibited::Trigger(cause) => Some((ban, cause)),
            _ => None,
        })
}

/// Whether the turn of `player` skips `step`: the cleanup step never does (CR 514).
pub(crate) fn skips_step(state: &GameState, player: PlayerId, step: Step) -> bool {
    step != Step::Cleanup && forbidden(state, Doing::SkipStep { player, step })
}

/// Whether the turn `player` would take is skipped.
pub fn skips_turn(state: &GameState, player: PlayerId) -> bool {
    forbidden(state, Doing::SkipTurn { player })
}

/// Whether the object `object` can be sacrificed by `player`, as the candidates of a cost and the choices of an effect
/// ask.
pub(crate) fn may_sacrifice(state: &GameState, player: PlayerId, object: ObjectId) -> bool {
    !forbidden(state, Doing::Sacrifice { player, object })
}

/// Whether `player` can discard `object`.
pub(crate) fn may_discard(state: &GameState, player: PlayerId, object: ObjectId) -> bool {
    !forbidden(state, Doing::Discard { player, object })
}

/// Whether `object` can be exiled.
pub(crate) fn may_exile(state: &GameState, object: ObjectId) -> bool {
    !forbidden(state, Doing::Exile { object })
}

/// What an event does that a prohibition may forbid: the actions it carries out.
fn doings(state: &GameState, event: &Event) -> Vec<Doing> {
    let mut found = Vec::new();
    match event {
        Event::ZoneChanged {
            object, to, cause, ..
        } => {
            let player = holder(state, *object);
            match (cause, player) {
                (ZoneCause::Sacrificed, Some(player)) => {
                    found.push(Doing::Sacrifice {
                        player,
                        object: *object,
                    });
                }
                (ZoneCause::Discarded, Some(player)) => {
                    found.push(Doing::Discard {
                        player,
                        object: *object,
                    });
                }
                _ => {}
            }
            if *to == Zone::Exile {
                found.push(Doing::Exile { object: *object });
            }
        }
        Event::CardDiscarded { player, object } => {
            found.push(Doing::Discard {
                player: *player,
                object: *object,
            });
        }
        Event::CountersChanged(changed) if changed.delta > 0 => {
            found.push(Doing::PutCounter { on: changed.on });
        }
        Event::Attached { object, to } => {
            if let Some(host) = state.object(*to) {
                found.push(Doing::Attach {
                    object: *object,
                    to: Target::Object(host.reference()),
                });
            }
        }
        Event::AttachedToPlayer { object, player } => {
            found.push(Doing::Attach {
                object: *object,
                to: Target::Player(*player),
            });
        }
        Event::Transformed { object, .. } => found.push(Doing::Transform { object: *object }),
        _ => {}
    }
    found
}

/// The events a batch keeps once the prohibitions in force take out those they forbid.
pub(crate) fn filter(state: &GameState, events: Vec<Event>) -> Vec<Event> {
    if state.rules().cants.is_empty() {
        return events;
    }
    events
        .into_iter()
        .filter(|event| {
            !doings(state, event)
                .into_iter()
                .any(|doing| forbidden(state, doing))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: each prohibition drops exactly the events of its action whose subject it names, for a player, for an object and for a group, and keeps every other event of the batch in its order.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::value::CounterKind;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::counters::CountersChanged;
    use crate::fixtures::{creature, state};

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 controls a Holder with the rule and a Wolf, seat 1 a Bear.
    fn game(rule: &str) -> GameState {
        let ability: Ability =
            serde_json::from_str(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#)).unwrap();
        let holder = Arc::new(
            CardDefinition::new("Holder")
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(ability),
        );
        state(
            vec![vec![], vec![]],
            vec![
                vec![holder, creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2)],
            ],
        )
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(GameObject::id)
            .unwrap()
    }

    /// The event of the named action on the Bear (or between the Wolf and the Bear).
    fn event(game: &GameState, action: &str) -> Event {
        let bear = id(game, "Bear");
        let bear_ref = game.object(bear).unwrap().reference();
        let counters = |delta| {
            Event::CountersChanged(CountersChanged {
                on: Target::Object(bear_ref),
                counter: CounterKind::PlusOnePlusOne,
                delta,
            })
        };
        match action {
            "sacrifice" => Event::ZoneChanged {
                object: bear,
                from: Zone::Battlefield,
                to: Zone::Graveyard(FOE),
                cause: ZoneCause::Sacrificed,
            },
            "exile" => Event::ZoneChanged {
                object: bear,
                from: Zone::Battlefield,
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            },
            "discard" => Event::CardDiscarded {
                player: FOE,
                object: bear,
            },
            "counter" => counters(1),
            "counter_removed" => counters(-1),
            "attach" => Event::Attached {
                object: id(game, "Wolf"),
                to: bear,
            },
            "transform" => Event::Transformed {
                object: bear,
                face: None,
            },
            other => panic!("unknown action {other}"),
        }
    }

    #[rstest]
    #[case::their_sacrifice(
        r#"{"Cant":{"who":"Opponents","action":"Sacrifice"}}"#,
        "sacrifice",
        true
    )]
    #[case::my_sacrifice_is_not_theirs(
        r#"{"Cant":{"who":"You","action":"Sacrifice"}}"#,
        "sacrifice",
        false
    )]
    #[case::that_permanent(r#"{"Cant":{"who":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"action":"Sacrifice"}}"#, "sacrifice", true)]
    #[case::exile(
        r#"{"Cant":{"who":{"Each":[{"Type":"Creature"}]},"action":"Exile"}}"#,
        "exile",
        true
    )]
    #[case::exile_of_mine(r#"{"Cant":{"who":"You","action":"Exile"}}"#, "exile", false)]
    #[case::exile_of_theirs(r#"{"Cant":{"who":"Opponents","action":"Exile"}}"#, "exile", true)]
    #[case::discard(r#"{"Cant":{"who":"Opponents","action":"Discard"}}"#, "discard", true)]
    #[case::discard_of_mine(r#"{"Cant":{"who":"You","action":"Discard"}}"#, "discard", false)]
    #[case::counters(
        r#"{"Cant":{"who":{"Each":[{"Type":"Creature"}]},"action":"PutCounter"}}"#,
        "counter",
        true
    )]
    #[case::counters_on_a_player(
        r#"{"Cant":{"who":"Opponents","action":"PutCounter"}}"#,
        "counter",
        true
    )]
    #[case::removing_counters_is_not_putting(
        r#"{"Cant":{"who":{"Each":[{"Type":"Creature"}]},"action":"PutCounter"}}"#,
        "counter_removed",
        false
    )]
    #[case::attaching_to_it(r#"{"Cant":{"who":"Opponents","action":"Attach"}}"#, "attach", true)]
    #[case::attaching_it(r#"{"Cant":{"who":"You","action":"Attach"}}"#, "attach", true)]
    #[case::attaching_for_nobody_here(
        r#"{"Cant":{"who":{"Each":[{"Type":"Land"}]},"action":"Attach"}}"#,
        "attach",
        false
    )]
    #[case::transforming(
        r#"{"Cant":{"who":{"Each":[{"Type":"Creature"}]},"action":"Transform"}}"#,
        "transform",
        true
    )]
    #[case::another_action(
        r#"{"Cant":{"who":{"Each":[{"Type":"Creature"}]},"action":"Transform"}}"#,
        "exile",
        false
    )]
    fn a_prohibition_drops_the_events_of_its_action_it_covers(
        #[case] rule: &str,
        #[case] action: &str,
        #[case] dropped: bool,
    ) {
        let game = game(rule);
        let wanted = event(&game, action);
        let other = Event::LifeGained {
            player: ME,
            amount: 1,
        };
        let kept = filter(&game, vec![other.clone(), wanted.clone(), other.clone()]);
        let expected = if dropped {
            vec![other.clone(), other]
        } else {
            vec![other.clone(), wanted, other]
        };
        assert_eq!(kept, expected);
    }

    #[rstest]
    #[case::the_cleanup_step_is_never_skipped(
        r#"{"Cant":{"who":"You","action":{"SkipStep":"Cleanup"}}}"#,
        Step::Cleanup,
        false
    )]
    #[case::my_draw(
        r#"{"Cant":{"who":"You","action":{"SkipStep":"Draw"}}}"#,
        Step::Draw,
        true
    )]
    #[case::another_step(
        r#"{"Cant":{"who":"You","action":{"SkipStep":"Draw"}}}"#,
        Step::Upkeep,
        false
    )]
    fn a_step_is_skipped_only_when_named_and_never_cleanup(
        #[case] rule: &str,
        #[case] step: Step,
        #[case] skipped: bool,
    ) {
        let game = game(rule);
        assert_eq!(skips_step(&game, ME, step), skipped);
        assert!(!skips_step(&game, FOE, step));
    }

    #[rstest]
    #[case::mine(r#"{"Cant":{"who":"You","action":"SkipTurn"}}"#, true, false)]
    #[case::theirs(r#"{"Cant":{"who":"Opponents","action":"SkipTurn"}}"#, false, true)]
    #[case::everyones(r#"{"Cant":{"who":"EachPlayer","action":"SkipTurn"}}"#, true, true)]
    fn a_turn_is_skipped_for_the_players_named(
        #[case] rule: &str,
        #[case] me: bool,
        #[case] foe: bool,
    ) {
        let game = game(rule);
        assert_eq!((skips_turn(&game, ME), skips_turn(&game, FOE)), (me, foe));
    }
}
