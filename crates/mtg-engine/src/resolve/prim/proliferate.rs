//! `Effect::Proliferate`: players proliferate (CR 701.34).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::wording;
use crate::zones::Zone;

/// Each player the effect names who is still in the game proliferates, in turn order: they choose any number of the
/// permanents on the battlefield that hold a counter (asked whenever there is one to choose, since choosing none is
/// an answer too), then say yes or no for each player in the game who holds a counter. Everything is decided before
/// any counter is given. One `Proliferated` is logged for each player who proliferates, even when nothing holds a
/// counter or nothing is chosen, followed by a `CountersChanged` of one counter for each kind each chosen permanent
/// and player holds, so a replacement effect sees each as the counters it would put.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Proliferate { who } = effect else {
        return Err(wrong_effect("Proliferate"));
    };
    let state = cx.state();
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    Stop::settle(proliferations(cx, &players).map(Run::events))
}

/// The proliferation of each player, asking in turn.
fn proliferations(cx: &Cx<'_>, players: &[PlayerId]) -> Result<Vec<Event>, Stop> {
    let state = cx.state();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    for player in players {
        let permanents: Vec<ObjectId> = state
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .filter(|id| {
                state
                    .object(*id)
                    .and_then(|object| object.permanent())
                    .is_some_and(|permanent| permanent.counters().next().is_some())
            })
            .collect();
        let mut holders: Vec<Target> = Vec::new();
        if !permanents.is_empty() {
            let chosen = questions.cards(
                *player,
                permanents.clone(),
                (0, permanents.len()),
                wording::PROLIFERATE.to_owned(),
            )?;
            // The answer lists them in the order the player gave; they are given counters in the board's order.
            holders.extend(
                permanents
                    .iter()
                    .filter(|id| chosen.contains(id))
                    .filter_map(|id| state.object(*id))
                    .map(|object| Target::Object(object.reference())),
            );
        }
        for holder in cx.scope.players(&Ref::EachPlayer)? {
            let holds = state.is_alive(holder)
                && state
                    .player(holder)
                    .is_some_and(|p| p.counters().next().is_some());
            if holds && questions.yes_no(*player, wording::proliferate_player(*player, holder))? {
                holders.push(Target::Player(holder));
            }
        }
        events.push(Event::Proliferated { player: *player });
        for holder in holders {
            events.extend(kinds_held(cx, holder).into_iter().map(|counter| {
                Event::CountersChanged(CountersChanged {
                    on: holder,
                    counter,
                    delta: 1,
                })
            }));
        }
    }
    questions.end()?;
    Ok(events)
}

/// The kinds of counter `holder` holds now, in the fixed order of the kinds.
fn kinds_held(cx: &Cx<'_>, holder: Target) -> Vec<mtg_cards::value::CounterKind> {
    let state = cx.state();
    match holder {
        Target::Player(id) => state
            .player(id)
            .map(|p| p.counters().map(|(kind, _)| kind.clone()).collect())
            .unwrap_or_default(),
        Target::Object(named) => state
            .object(named.id)
            .and_then(|object| object.permanent())
            .map(|p| p.counters().map(|(kind, _)| kind.clone()).collect())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a proliferate offers every permanent on the battlefield that holds a counter and asks about every player who holds one (and nothing else), lets the player choose none, some or all, gives one more counter of each kind each chosen permanent or player holds, logs one `Proliferated` for each proliferating player even when nothing was chosen or nothing holds a counter, decides every player's proliferate before any counter moves, skips what has left the battlefield or the game, and refuses an answer it did not ask for.
    use mtg_cards::value::CounterKind;
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// A table of `seats`: seat 0 controls the Source and a Wolf with two +1/+1 and one charge counter, seat 1 a Bear
    /// with one -1/-1 counter and an Ogre without, and seat 1 holds two poison counters.
    fn game(seats: usize, counters: bool) -> GameState {
        let hands = vec![Vec::new(); seats];
        let mut boards = vec![Vec::new(); seats];
        boards[0] = vec![creature("Source", 1, 1), creature("Wolf", 2, 2)];
        boards[1] = vec![creature("Bear", 2, 2), creature("Ogre", 3, 4)];
        let mut game = setup(hands, boards);
        if counters {
            game.players[0].battlefield[1]
                .counters
                .extend([(CounterKind::PlusOnePlusOne, 2), (CounterKind::Charge, 1)]);
            game.players[1].battlefield[0]
                .counters
                .push((CounterKind::MinusOneMinusOne, 1));
            game.players[1].counters.push((CounterKind::Poison, 2));
        }
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
        let source = game.object(named(game, "Source")).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the effect asks");
        };
        decision
    }

    /// The events of a finished run, as short words.
    fn done(game: &GameState, run: Run) -> Vec<String> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the proliferate is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        events
            .into_iter()
            .map(|event| match event {
                Event::Proliferated { player } => format!("proliferated by {}", player.index()),
                Event::CountersChanged(changed) => {
                    let holder = match changed.on {
                        Target::Player(player) => format!("seat {}", player.index()),
                        Target::Object(object) => {
                            game.object(object.id).unwrap().card().name.clone()
                        }
                    };
                    format!("{holder} {:?} {:+}", changed.counter, changed.delta)
                }
                other => panic!("{other:?}"),
            })
            .collect()
    }

    fn cards(game: &GameState, names: &[&str]) -> Choice {
        Choice::Cards(names.iter().map(|n| named(game, n)).collect())
    }

    const JSON: &str = r#"{"Proliferate":{}}"#;

    #[rstest]
    fn the_first_question_offers_the_permanents_that_hold_counters(
        #[values(2, 3, 4)] seats: usize,
    ) {
        let game = game(seats, true);
        let decision = asked(run_with(&game, Vec::new(), JSON).unwrap());
        assert_eq!(
            decision,
            Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source: named(&game, "Source"),
                    from: vec![named(&game, "Wolf"), named(&game, "Bear")],
                    min: 0,
                    max: 2,
                    text: wording::PROLIFERATE.into(),
                    can_cancel: false,
                })
            }
        );
    }

    #[rstest]
    #[case::nothing_chosen(&[], false, &["proliferated by 0"])]
    #[case::the_wolf(&["Wolf"], false, &["proliferated by 0", "Wolf PlusOnePlusOne +1", "Wolf Charge +1"])]
    #[case::the_bear_gets_a_minus_counter(&["Bear"], false, &["proliferated by 0", "Bear MinusOneMinusOne +1"])]
    #[case::only_the_player(&[], true, &["proliferated by 0", "seat 1 Poison +1"])]
    #[case::everything(
        &["Bear", "Wolf"],
        true,
        &[
            "proliferated by 0",
            "Wolf PlusOnePlusOne +1",
            "Wolf Charge +1",
            "Bear MinusOneMinusOne +1",
            "seat 1 Poison +1",
        ]
    )]
    fn each_chosen_permanent_and_player_gets_one_more_counter_of_each_kind_it_holds(
        #[case] chosen: &[&str],
        #[case] poison: bool,
        #[case] expected: &[&str],
        #[values(2, 3, 4)] seats: usize,
    ) {
        let game = game(seats, true);
        let journal = vec![(ME, cards(&game, chosen)), (ME, Choice::YesNo(poison))];
        // The question about the poisoned seat comes after the permanents.
        let second = asked(run_with(&game, journal[..1].to_vec(), JSON).unwrap());
        let DecisionKind::YesNo(options) = second.kind else {
            panic!("the players holding counters are asked");
        };
        assert_eq!(
            options.text,
            "Proliferate seat 2: another counter of each kind they have?"
        );
        let events = done(&game, run_with(&game, journal, JSON).unwrap());
        assert_eq!(events, expected);
    }

    #[rstest]
    fn with_no_counters_anywhere_nothing_is_asked_and_the_proliferate_still_happens(
        #[values(2, 3, 4)] seats: usize,
    ) {
        let game = game(seats, false);
        let run = run_with(&game, Vec::new(), JSON).unwrap();
        assert_eq!(done(&game, run), ["proliferated by 0"]);
    }

    #[test]
    fn only_a_player_holding_counters_is_asked_and_the_question_comes_without_permanents() {
        let mut raw = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        raw.players[0]
            .counters
            .push((CounterKind::Other("energy".into()), 3));
        let game = GameState::from_setup(&raw).unwrap();
        let first = asked(run_with(&game, Vec::new(), JSON).unwrap());
        let DecisionKind::YesNo(options) = first.kind else {
            panic!("no permanent holds a counter: the player is asked first");
        };
        assert_eq!(first.player, ME);
        assert_eq!(
            options.text,
            "Proliferate yourself: another counter of each kind you have?"
        );
        let run = run_with(&game, vec![(ME, Choice::YesNo(true))], JSON).unwrap();
        assert_eq!(
            done(&game, run),
            ["proliferated by 0", "seat 0 Other(\"energy\") +1"]
        );
    }

    #[test]
    fn a_permanent_that_has_left_the_battlefield_is_not_offered_and_a_player_who_left_is_not_asked()
    {
        let mut game = game(2, true);
        let wolf = named(&game, "Wolf");
        crate::fold::apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: wolf,
                from: Zone::Battlefield,
                to: Zone::Graveyard(ME),
                cause: crate::events::ZoneCause::Destroyed,
            },
        )
        .unwrap();
        let decision = asked(run_with(&game, Vec::new(), JSON).unwrap());
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(options.from, vec![named(&game, "Bear")]);
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        // Only the player is skipped here: the question about the poisoned seat is not asked.
        let run = run_with(&game, vec![(ME, cards(&game, &["Bear"]))], JSON).unwrap();
        assert_eq!(
            done(&game, run),
            ["proliferated by 0", "Bear MinusOneMinusOne +1"]
        );
    }

    #[test]
    fn every_player_who_proliferates_decides_before_a_counter_moves() {
        let game = game(2, true);
        let json = r#"{"Proliferate":{"who":"EachPlayer"}}"#;
        let first = asked(run_with(&game, Vec::new(), json).unwrap());
        assert_eq!(first.player, ME);
        let mut journal = vec![(ME, cards(&game, &["Wolf"])), (ME, Choice::YesNo(false))];
        let second = asked(run_with(&game, journal.clone(), json).unwrap());
        assert_eq!(second.player, FOE);
        journal.extend([(FOE, cards(&game, &[])), (FOE, Choice::YesNo(true))]);
        let run = run_with(&game, journal, json).unwrap();
        assert_eq!(
            done(&game, run),
            [
                "proliferated by 0",
                "Wolf PlusOnePlusOne +1",
                "Wolf Charge +1",
                "proliferated by 1",
                "seat 1 Poison +1",
            ]
        );
    }

    #[test]
    fn an_answer_the_step_cannot_use_is_a_bug() {
        let game = game(2, true);
        for journal in [
            vec![(ME, Choice::Cards(vec![named(&game, "Ogre")]))],
            vec![(FOE, cards(&game, &[]))],
            vec![(ME, Choice::YesNo(true))],
            vec![(ME, cards(&game, &[])), (ME, cards(&game, &[]))],
            vec![
                (ME, cards(&game, &[])),
                (ME, Choice::YesNo(true)),
                (ME, Choice::YesNo(true)),
            ],
        ] {
            let refused = run_with(&game, journal.clone(), JSON);
            assert!(
                matches!(refused, Err(ResolveError::Invalid(_))),
                "{journal:?}: {refused:?}"
            );
        }
        let refused = run_with(&game, Vec::new(), r#"{"Draw":{}}"#);
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
