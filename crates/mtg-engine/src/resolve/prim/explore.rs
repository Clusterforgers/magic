//! `Effect::Explore`: creatures explore (CR 701.44).

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::library::{Revealed, top_cards};
use crate::object::ObjectRef;
use crate::resolve::{Cx, ResolveError, Run};
use crate::wording;
use crate::zones::Zone;

/// A creature that explores: its controller, and the permanent when it is still on the battlefield to get the counter.
struct Explorer {
    object: ObjectId,
    player: PlayerId,
    permanent: Option<ObjectRef>,
}

/// Each creature the effect names explores in turn, once each. Its controller (as it was, when the creature has left
/// the battlefield and the ability remembers it) reveals the top card of their library: a land goes to their hand;
/// any other card, or no card because the library is empty, gives the creature a +1/+1 counter if it is still on the
/// battlefield, and the player may put a revealed card into their graveyard (asked with `ChooseCards` over that card,
/// none or one, since seeing it is the point). A card that stays on top is the one the next explore reveals. Every
/// answer is in before anything happens; each explore is an `Explored`, the `Revealed` of its card and the moves and
/// the counter that follow, the counter as a proposed `CountersChanged` that a replacement effect sees.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Explore { what } = effect else {
        return Err(wrong_effect("Explore"));
    };
    let state = cx.state();
    let mut explorers: Vec<Explorer> = Vec::new();
    for named in cx.scope.objects(what)? {
        if explorers.iter().any(|seen| seen.object == named.id) {
            continue;
        }
        let live = state.object(named.id).filter(|object| {
            object.counter() == named.counter && object.zone() == Zone::Battlefield
        });
        let (player, permanent) = match live {
            Some(object) => (object.controller(), Some(object.reference())),
            None => match cx.scope.snapshot_of(named) {
                Some(shot) if shot.zone == Zone::Battlefield => (shot.controller, None),
                _ => continue,
            },
        };
        if state.is_alive(player) {
            explorers.push(Explorer {
                object: named.id,
                player,
                permanent,
            });
        }
    }
    Stop::settle(explorations(cx, &explorers).map(Run::events))
}

/// The events of each exploration, asking in turn.
fn explorations(cx: &Cx<'_>, explorers: &[Explorer]) -> Result<Vec<Event>, Stop> {
    let state = cx.state();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    // The cards earlier explorations took out of a library: the next one reveals the card below them.
    let mut taken: Vec<ObjectId> = Vec::new();
    for explorer in explorers {
        let player = explorer.player;
        events.push(Event::Explored {
            object: explorer.object,
            player,
        });
        let top = top_cards(state, player, taken.len() + 1)
            .into_iter()
            .find(|card| !taken.contains(card));
        let land = top.is_some_and(|card| {
            state
                .object(card)
                .is_some_and(|object| object.base_types().contains(CardType::Land))
        });
        if let Some(card) = top {
            events.push(Event::Revealed(Revealed {
                player,
                cards: vec![card],
            }));
        }
        if land {
            if let Some(card) = top {
                taken.push(card);
                events.push(moved(card, player, Zone::Hand(player)));
            }
            continue;
        }
        if let Some(holder) = explorer.permanent {
            events.push(Event::CountersChanged(CountersChanged {
                on: Target::Object(holder),
                counter: CounterKind::PlusOnePlusOne,
                delta: 1,
            }));
        }
        if let Some(card) = top {
            let chosen =
                questions.cards(player, vec![card], (0, 1), wording::EXPLORE.to_owned())?;
            if !chosen.is_empty() {
                taken.push(card);
                events.push(moved(card, player, Zone::Graveyard(player)));
            }
        }
    }
    questions.end()?;
    Ok(events)
}

/// A card leaves the top of a library for `to`.
fn moved(object: ObjectId, player: PlayerId, to: Zone) -> Event {
    Event::ZoneChanged {
        object,
        from: Zone::Library(player),
        to,
        cause: ZoneCause::Moved,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an explore reveals the top card of the library of the controller of each creature it names, in turn: a land goes to the hand and asks nothing, any other card (or no card at all) gives the creature a +1/+1 counter while it is on the battlefield and a nonland card is offered to the graveyard with a `ChooseCards` over that card alone; a card that stays on top is the one the next explore reveals, a creature the effect cannot find explores not at all, every answer is in before anything happens, and a wrong answer is refused.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Forest").with_type(CardType::Land))
    }

    /// Seat 0 controls the Source and a Wolf (or holds the Source in hand when `on_board` is false) and has `library`
    /// (top first); seat 1 controls a Bear and has a library of one land.
    fn game(library: Vec<Arc<CardDefinition>>, on_board: bool) -> GameState {
        let (hand, board) = if on_board {
            (
                vec![],
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
            )
        } else {
            (vec![creature("Source", 1, 1)], vec![creature("Wolf", 2, 2)])
        };
        let mut game = setup(
            vec![hand, vec![]],
            vec![board, vec![creature("Bear", 2, 2)]],
        );
        game.players[0].library = library;
        game.players[1].library = vec![forest()];
        GameState::from_setup(&game).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        json: &str,
    ) -> Result<Run, ResolveError> {
        let resolving = Resolving {
            journal,
            ..Resolving::begin(named(game, "Source"))
        };
        let source = game.object(named(game, "Source")).unwrap();
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    /// The events of a finished run as short words.
    fn done(game: &GameState, run: Run) -> Vec<String> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the explore is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        let name = |id: ObjectId| game.object(id).unwrap().card().name.clone();
        events
            .into_iter()
            .map(|event| match event {
                Event::Explored { object, player } => {
                    format!("{} explores for seat {}", name(object), player.index())
                }
                Event::Revealed(Revealed { cards, .. }) => format!("reveals {}", name(cards[0])),
                Event::ZoneChanged { object, to, .. } => format!("{} to {to:?}", name(object)),
                Event::CountersChanged(changed) => {
                    let Target::Object(holder) = changed.on else {
                        panic!("a creature gets it");
                    };
                    format!("{} {:+}", name(holder.id), changed.delta)
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    fn asked(run: Run) -> CardsOptions {
        let Run::Ask(Decision {
            player,
            kind: DecisionKind::ChooseCards(options),
        }) = run
        else {
            panic!("the explore asks which card goes to the graveyard");
        };
        assert_eq!(player, ME);
        options
    }

    const SOURCE: &str = r#"{"Explore":{"what":"This"}}"#;
    const BOTH: &str =
        r#"{"Explore":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#;

    #[test]
    fn a_land_goes_to_the_hand_and_asks_nothing() {
        let game = game(vec![forest(), creature("Ogre", 3, 3)], true);
        let events = done(&game, run_with(&game, vec![], SOURCE).unwrap());
        assert_eq!(
            events,
            [
                "Source explores for seat 0",
                "reveals Forest",
                "Forest to Hand(PlayerId(0))"
            ]
        );
    }

    #[rstest]
    #[case::kept_on_top(false, &["Source explores for seat 0", "reveals Ogre", "Source +1"])]
    #[case::put_in_the_graveyard(true, &["Source explores for seat 0", "reveals Ogre", "Source +1", "Ogre to Graveyard(PlayerId(0))"])]
    fn a_card_that_is_not_a_land_gives_a_counter_and_is_offered_to_the_graveyard(
        #[case] graveyard: bool,
        #[case] expected: &[&str],
    ) {
        let game = game(vec![creature("Ogre", 3, 3), forest()], true);
        let options = asked(run_with(&game, vec![], SOURCE).unwrap());
        let ogre = named(&game, "Ogre");
        assert_eq!(
            (options.from.clone(), options.min, options.max),
            (vec![ogre], 0, 1)
        );
        let chosen = if graveyard { vec![ogre] } else { vec![] };
        let events = done(
            &game,
            run_with(&game, vec![(ME, Choice::Cards(chosen))], SOURCE).unwrap(),
        );
        assert_eq!(events, expected);
    }

    #[test]
    fn an_empty_library_still_gives_the_counter_and_asks_nothing() {
        let game = game(vec![], true);
        let events = done(&game, run_with(&game, vec![], SOURCE).unwrap());
        assert_eq!(events, ["Source explores for seat 0", "Source +1"]);
    }

    #[test]
    fn a_creature_that_cannot_be_found_does_not_explore() {
        let game = game(vec![forest()], false);
        let events = done(&game, run_with(&game, vec![], SOURCE).unwrap());
        assert_eq!(events, Vec::<String>::new());
    }

    #[test]
    fn the_card_below_a_card_that_left_is_the_next_one_revealed() {
        // Two explorers: the land goes to the hand, so the second reveals the Ogre.
        let game = game(vec![forest(), creature("Ogre", 3, 3)], true);
        let ogre = named(&game, "Ogre");
        let first = asked(run_with(&game, vec![], BOTH).unwrap());
        assert_eq!(first.from, vec![ogre]);
        let events = done(
            &game,
            run_with(&game, vec![(ME, Choice::Cards(vec![]))], BOTH).unwrap(),
        );
        assert_eq!(
            events,
            [
                "Source explores for seat 0",
                "reveals Forest",
                "Forest to Hand(PlayerId(0))",
                "Wolf explores for seat 0",
                "reveals Ogre",
                "Wolf +1"
            ]
        );
    }

    #[test]
    fn a_card_that_stays_on_top_is_revealed_again_by_the_next_explore() {
        let game = game(vec![creature("Ogre", 3, 3), forest()], true);
        let ogre = named(&game, "Ogre");
        let events = done(
            &game,
            run_with(
                &game,
                vec![(ME, Choice::Cards(vec![])), (ME, Choice::Cards(vec![ogre]))],
                BOTH,
            )
            .unwrap(),
        );
        assert_eq!(
            events,
            [
                "Source explores for seat 0",
                "reveals Ogre",
                "Source +1",
                "Wolf explores for seat 0",
                "reveals Ogre",
                "Wolf +1",
                "Ogre to Graveyard(PlayerId(0))"
            ]
        );
    }

    #[test]
    fn an_answer_that_is_not_the_revealed_card_is_refused() {
        let game = game(vec![creature("Ogre", 3, 3)], true);
        let wolf = named(&game, "Wolf");
        assert!(run_with(&game, vec![(ME, Choice::Cards(vec![wolf]))], SOURCE).is_err());
    }
}
