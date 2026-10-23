//! `Effect::Manifest` and `Effect::Cloak`: cards from the top of a library are put onto the battlefield face down as 2/2
//! creatures under the control of the player who acts (CR 701.40a, 701.58a).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// What the effect says, whichever of the two it is.
struct Fields<'a> {
    cloak: bool,
    who: &'a Ref,
    count: &'a mtg_cards::value::Amount,
    from: &'a Option<Ref>,
    bind: &'a Option<String>,
}

fn fields(effect: &Effect) -> Result<Fields<'_>, ResolveError> {
    match effect {
        Effect::Manifest {
            who,
            count,
            from,
            bind,
        } => Ok(Fields {
            cloak: false,
            who,
            count,
            from,
            bind,
        }),
        Effect::Cloak {
            who,
            count,
            from,
            bind,
        } => Ok(Fields {
            cloak: true,
            who,
            count,
            from,
            bind,
        }),
        _ => Err(wrong_effect("Manifest")),
    }
}

/// Each player the effect names who is in the game takes `count` cards from the top of their own library, or of each
/// library `from` names, the top card first, and puts them onto the battlefield face down under their control: one
/// `ZoneChanged` for each card whose cause says who manifested it, and an `Entered` after it when that player is not the
/// card's owner. A library shorter than the count gives what it has and an empty one nothing. The cards are bound to the
/// name, when the effect has one, as the permanents they become (CR 400.7).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Fields {
        cloak,
        who,
        count,
        from,
        bind,
    } = fields(effect)?;
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let mut events = Vec::new();
    let mut made = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        let libraries: Vec<PlayerId> = match from {
            Some(whose) => cx.scope.players(whose)?,
            None => vec![player],
        };
        for owner in libraries.into_iter().filter(|owner| state.is_alive(*owner)) {
            for id in top_cards(state, owner, wanted) {
                let Some(card) = state.object(id) else {
                    continue;
                };
                events.push(Event::ZoneChanged {
                    object: id,
                    from: Zone::Library(owner),
                    to: Zone::Battlefield,
                    cause: if cloak {
                        ZoneCause::Cloaked { by: player }
                    } else {
                        ZoneCause::Manifested { by: player }
                    },
                });
                if owner != player {
                    events.push(Event::Entered {
                        object: id,
                        tapped: false,
                        controller: player,
                        chosen: None,
                    });
                }
                made.push(Target::Object(ObjectRef {
                    id,
                    counter: card.reference().counter + 1,
                }));
            }
        }
    }
    Ok(Run::Done {
        events,
        bound: bind
            .iter()
            .map(|name| (name.clone(), made.clone()))
            .collect(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: manifesting and cloaking take the top cards of the library the effect names, the top card first, and put each onto the battlefield face down under the control of the player who acts (an `Entered` after the move when that is not the card's owner), take what a short library has and nothing from an empty one, bind the permanents they become, and the fold lays them down as 2/2 creatures only their controller looks at (cloaked ones with ward {2}) whose card is what they were; at two to four seats.
    use mtg_cards::keyword::Keyword;
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::characteristics::{characteristics, controller_of};
    use crate::fixtures::{creature, state};
    use crate::keywords::keywords_of;
    use crate::object::FaceDown;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game(seats: usize) -> GameState {
        let mut batteries = vec![vec![creature("Source", 1, 1)]];
        batteries.resize(seats, vec![]);
        state(vec![vec![]; seats], batteries)
    }

    fn top(game: &GameState, player: PlayerId, n: usize) -> Vec<ObjectId> {
        game.zone(Zone::Library(player))
            .iter()
            .rev()
            .take(n)
            .copied()
            .collect()
    }

    fn run_json(game: &GameState, json: &str) -> (Vec<Event>, Vec<(String, Vec<Target>)>) {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run(&cx, &effect).unwrap()
        else {
            panic!("manifesting asks nothing");
        };
        assert!(snapshots.is_empty() && next == Next::Sibling);
        (events, bound)
    }

    fn arrivals(events: &[Event]) -> Vec<(ObjectId, Zone, ZoneCause)> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::ZoneChanged {
                    object,
                    from,
                    to: Zone::Battlefield,
                    cause,
                } => Some((*object, *from, *cause)),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[case::one_card(r#"{"Manifest":{}}"#, 1)]
    #[case::two_cards(r#"{"Manifest":{"count":2}}"#, 2)]
    #[case::more_than_the_library_holds(r#"{"Manifest":{"count":9}}"#, 5)]
    #[case::a_count_of_nothing(r#"{"Manifest":{"count":0}}"#, 0)]
    #[case::cloaked(r#"{"Cloak":{"count":2}}"#, 2)]
    fn the_top_cards_come_onto_the_battlefield_top_first_under_the_acting_player(
        #[case] json: &str,
        #[case] n: usize,
        #[values(2, 3, 4)] seats: usize,
    ) {
        let game = game(seats);
        let (events, _) = run_json(&game, json);
        let cloak = json.contains("Cloak");
        let made = arrivals(&events);
        assert_eq!(
            made.iter().map(|(id, ..)| *id).collect::<Vec<_>>(),
            top(&game, ME, n)
        );
        for (_, from, cause) in &made {
            assert_eq!(*from, Zone::Library(ME));
            assert_eq!(
                *cause,
                if cloak {
                    ZoneCause::Cloaked { by: ME }
                } else {
                    ZoneCause::Manifested { by: ME }
                }
            );
        }
        assert_eq!(events.len(), n, "the owner needs no Entered");
    }

    #[rstest]
    fn a_library_the_effect_names_is_not_the_acting_players(#[values(2, 3, 4)] seats: usize) {
        let game = game(seats);
        let foe = PlayerId(1);
        let (events, bound) =
            run_json(&game, r#"{"Manifest":{"library":"Opponents","as":"made"}}"#);
        let made = arrivals(&events);
        assert_eq!(made.len(), seats - 1, "one card from each opponent");
        for (id, from, cause) in &made {
            let Zone::Library(owner) = from else {
                panic!("not from a library");
            };
            assert_ne!(*owner, ME);
            assert_eq!(*cause, ZoneCause::Manifested { by: ME });
            assert!(
                events.iter().any(|event| matches!(
                    event,
                    Event::Entered { object, controller, tapped: false, .. }
                        if object == id && *controller == ME
                )),
                "{id:?} enters under the acting player"
            );
        }
        assert!(made.iter().any(|(_, from, _)| *from == Zone::Library(foe)));
        assert_eq!(bound.len(), 1);
        assert_eq!(bound[0].0, "made");
        assert_eq!(bound[0].1.len(), seats - 1);
    }

    #[rstest]
    #[case::manifested(r#"{"Manifest":{"as":"made"}}"#, FaceDown::Manifest, 0)]
    #[case::cloaked(r#"{"Cloak":{"as":"made"}}"#, FaceDown::Cloak, 1)]
    fn the_fold_lays_the_card_down_as_a_2_2_that_only_its_controller_looks_at(
        #[case] json: &str,
        #[case] kind: FaceDown,
        #[case] wards: usize,
        #[values(2, 3, 4)] seats: usize,
    ) {
        let mut game = game(seats);
        let card = top(&game, ME, 1)[0];
        let (events, bound) = run_json(&game, json);
        for event in &events {
            crate::fold::apply_event(&mut game, event).unwrap();
        }
        let object = game.object(card).unwrap();
        assert_eq!(object.zone(), Zone::Battlefield);
        assert_eq!(object.face_down(), Some(kind));
        assert_eq!(controller_of(&game, card), Some(ME));
        let chars = characteristics(&game, card).unwrap();
        assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
        assert_eq!(keywords_of(&game, card).len(), wards);
        assert!(
            keywords_of(&game, card)
                .iter()
                .all(|keyword| matches!(keyword, Keyword::Ward(_)))
        );
        assert_eq!(object.card().name, "");
        assert_ne!(object.whole_card().name, "", "the card is still there");
        for seat in 0..seats {
            let seat = PlayerId::from_index(seat);
            assert_eq!(game.may_look_at(seat, card), seat == ME);
        }
        assert_eq!(
            bound[0].1,
            vec![Target::Object(object.reference())],
            "the name holds the permanent it became"
        );
    }
}
