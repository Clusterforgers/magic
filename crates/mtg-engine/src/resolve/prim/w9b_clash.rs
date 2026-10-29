//! `Effect::Clash`: a player clashes with an opponent (CR 701.23) and the resolution goes on with the list for the result.

use mtg_cards::effect::{Effect, W9B_CLASHED};
use mtg_cards::refs::Ref;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{ChoiceOption, Questions, Stop, Target};
use crate::events::Event;
use crate::library::{Revealed, top_cards};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The top card of `player`'s library, if any.
fn w9b_top(cx: &Cx<'_>, player: PlayerId) -> Option<ObjectId> {
    top_cards(cx.state(), player, 1).first().copied()
}

/// The mana value of a revealed card.
fn w9b_value(cx: &Cx<'_>, card: ObjectId) -> u32 {
    cx.state()
        .object(card)
        .map_or(0, crate::object::GameObject::mana_value)
}

/// The clash after the opponent is known: both reveal, each chooses top or bottom (the clashing player first), and the
/// player wins when they revealed a card of greater mana value than the opponent's, or one against none (CR 701.23c).
fn w9b_reveal(
    cx: &Cx<'_>,
    questions: &mut Questions<'_>,
    pair: [PlayerId; 2],
) -> Result<(Vec<Event>, bool), Stop> {
    let tops = pair.map(|player| w9b_top(cx, player));
    let mut events = Vec::new();
    for (player, top) in pair.iter().zip(tops) {
        if let Some(card) = top {
            events.push(Event::Revealed(Revealed {
                player: *player,
                cards: vec![card],
            }));
        }
    }
    for (player, top) in pair.iter().zip(tops) {
        let Some(card) = top else { continue };
        let name = cx
            .state()
            .object(card)
            .map_or_else(String::new, |object| object.card().name.clone());
        let text = format!("Clash: put {name} on the bottom of your library?");
        if questions.yes_no(*player, text)? {
            events.push(Event::LibraryBottomed {
                player: *player,
                cards: vec![card],
                random: false,
            });
        }
    }
    let won = match tops {
        [Some(mine), Some(theirs)] => w9b_value(cx, mine) > w9b_value(cx, theirs),
        [Some(_), None] => true,
        _ => false,
    };
    events.push(Event::Clashed {
        player: pair[0],
        opponent: pair[1],
        won,
    });
    Ok((events, won))
}

/// The opponent chosen and the clash: its events, the opponent, and whether the clash was won.
fn w9b_walk(
    cx: &Cx<'_>,
    player: PlayerId,
    with: Option<&Ref>,
) -> Result<Option<(Vec<Event>, PlayerId, bool)>, Stop> {
    let state = cx.state();
    let opponents: Vec<PlayerId> = cx
        .scope
        .players(with.unwrap_or(&Ref::Opponents))?
        .into_iter()
        .filter(|other| *other != player && state.is_alive(*other))
        .collect();
    if opponents.is_empty() {
        return Ok(None);
    }
    let mut questions = Questions::new(cx);
    let options = opponents
        .iter()
        .copied()
        .map(ChoiceOption::Player)
        .collect();
    let index = questions.w9b_option(player, options, "Clash with an opponent.".to_owned())?;
    let opponent = opponents.get(index).copied().unwrap_or(opponents[0]);
    let (events, won) = w9b_reveal(cx, &mut questions, [player, opponent])?;
    questions.end()?;
    Ok(Some((events, opponent, won)))
}

/// The first player the effect names who is still in the game clashes with an opponent they choose; the step goes on
/// into `win` or `lose` with the opponent bound, and a clash until a loss clashes again after a win. Nobody to clash
/// is no clash.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Clash {
        who,
        win,
        until,
        with,
        ..
    } = effect
    else {
        return Err(wrong_effect("Clash"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let walked = match w9b_walk(cx, player, with.as_ref()) {
        Ok(walked) => walked,
        Err(stop) => return Stop::settle(Err(stop)),
    };
    let Some((events, opponent, won)) = walked else {
        return Ok(Run::events(Vec::new()));
    };
    Ok(Run::Done {
        events,
        // A repeated clash keeps the opponent of the first, which "that player" names.
        bound: if cx.resolving.bindings.contains_key(W9B_CLASHED) {
            Vec::new()
        } else {
            vec![(W9B_CLASHED.to_owned(), vec![Target::Player(opponent)])]
        },
        snapshots: Vec::new(),
        next: if won && *until && win.is_empty() {
            Next::Again
        } else {
            Next::Enter(usize::from(!won))
        },
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a clash reveals both top cards, asks each player top or bottom, is won only by a greater mana value (or a
    //! card against none), binds the opponent, asks the clashing player which opponent at three or four seats, clashes
    //! with nobody when no opponent is left, and refuses another effect by name.
    use std::sync::Arc;

    use mtg_cards::definition::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const CLASH: &str = r#"{"Clash":{"win":[{"Draw":{}}]}}"#;

    fn costed(cost: &str) -> Arc<CardDefinition> {
        let card = CardDefinition::new("Top").with_type(CardType::Creature);
        Arc::new(match cost {
            text if !text.is_empty() => card.with_mana_cost(ManaCost::parse(text).unwrap()),
            _ => card,
        })
    }

    /// A game of `seats` players whose top cards cost what `tops` says (`None` in a slot: an empty library).
    fn game(tops: &[Option<&str>]) -> GameState {
        let mut game = setup(
            vec![vec![]; tops.len()],
            (0..tops.len())
                .map(|seat| {
                    if seat == 0 {
                        vec![creature("Source", 1, 1)]
                    } else {
                        vec![]
                    }
                })
                .collect(),
        );
        for (player, top) in game.players.iter_mut().zip(tops) {
            player.library = top.map(costed).into_iter().collect();
        }
        GameState::from_setup(&game).unwrap()
    }

    fn clash(game: &GameState, journal: Vec<(PlayerId, Choice)>, json: &str) -> Run {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let mut resolving = Resolving::begin(source.id());
        resolving.journal = journal;
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap()).unwrap()
    }

    fn keep(player: u64) -> (PlayerId, Choice) {
        (PlayerId(player), Choice::YesNo(false))
    }

    #[rstest]
    #[case::greater(Some("{3}"), Some("{1}{R}"), true)]
    #[case::tie(Some("{2}"), Some("{1}{R}"), false)]
    #[case::smaller(Some(""), Some("{U}"), false)]
    #[case::against_none(Some(""), None, true)]
    #[case::none_against_one(None, Some(""), false)]
    fn the_greater_mana_value_wins(
        #[case] mine: Option<&str>,
        #[case] theirs: Option<&str>,
        #[case] won: bool,
    ) {
        let game = game(&[mine, theirs]);
        let journal: Vec<_> = [mine.map(|_| keep(0)), theirs.map(|_| keep(1))]
            .into_iter()
            .flatten()
            .collect();
        let Run::Done {
            events,
            bound,
            next,
            ..
        } = clash(&game, journal, CLASH)
        else {
            panic!("every question is answered");
        };
        assert_eq!(next, Next::Enter(usize::from(!won)));
        assert_eq!(
            bound,
            vec![(W9B_CLASHED.to_owned(), vec![Target::Player(PlayerId(1))])]
        );
        let revealed = events
            .iter()
            .filter(|e| matches!(e, Event::Revealed(_)))
            .count();
        assert_eq!(
            revealed,
            usize::from(mine.is_some()) + usize::from(theirs.is_some())
        );
    }

    #[test]
    fn each_player_is_asked_top_or_bottom_in_turn() {
        let game = game(&[Some("{3}"), Some("{1}")]);
        let Run::Ask(first) = clash(&game, vec![], CLASH) else {
            panic!("asks")
        };
        assert_eq!(first.player, ME);
        let Run::Ask(second) = clash(&game, vec![keep(0)], CLASH) else {
            panic!("asks")
        };
        assert_eq!(second.player, PlayerId(1));
        let bottom = (PlayerId(1), Choice::YesNo(true));
        let Run::Done { events, .. } = clash(&game, vec![keep(0), bottom], CLASH) else {
            panic!("done")
        };
        assert!(
            events.iter().any(
                |e| matches!(e, Event::LibraryBottomed { player, .. } if *player == PlayerId(1))
            )
        );
    }

    #[rstest]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn the_opponent_is_chosen_among_several(#[case] seats: usize) {
        let tops = vec![Some("{1}"); seats];
        let game = game(&tops);
        let Run::Ask(question) = clash(&game, vec![], CLASH) else {
            panic!("asks")
        };
        assert_eq!(question.player, ME);
        let pick = (ME, Choice::Option(seats - 2));
        let Run::Done { bound, .. } = clash(
            &game,
            vec![pick, keep(0), keep(u64::try_from(seats - 1).unwrap())],
            CLASH,
        ) else {
            panic!("done")
        };
        assert_eq!(
            bound[0].1,
            vec![Target::Player(PlayerId(u64::try_from(seats - 1).unwrap()))]
        );
    }

    #[test]
    fn a_won_clash_until_a_loss_with_no_list_runs_again() {
        let game = game(&[Some("{3}"), Some("")]);
        let Run::Done { next, .. } =
            clash(&game, vec![keep(0), keep(1)], r#"{"Clash":{"until":true}}"#)
        else {
            panic!("done")
        };
        assert_eq!(next, Next::Again);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let game = game(&[Some(""), Some("")]);
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        assert!(run(&cx, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).is_err());
    }
}
