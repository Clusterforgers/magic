//! `Effect::Vote`: each player votes (CR 701.38), the controller first and the others in turn order, and the resolution
//! goes on with the tally bound to names.

use mtg_cards::effect::{Effect, VoteFor};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{
    ChoiceOption, OptionChooser, Optioned, Picked, Selection, Target, pick_options, pick_selections,
};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The ballots of a vote, the controller first, in turn order; a player who has left the game does not vote. A player
/// who gets additional votes casts them right after the first, and one who may vote an additional time (CR 701.38d)
/// casts a last ballot they may leave blank (`true`).
fn voters(cx: &Cx<'_>) -> Vec<(PlayerId, bool)> {
    let state = cx.state();
    state
        .turn_order_from(cx.scope.controller)
        .into_iter()
        .flat_map(|player| {
            let extra = crate::rules::ep33_extra_votes(state, player);
            let again = crate::rules::w17c_votes_again(state, player);
            std::iter::repeat_n((player, false), 1 + extra).chain(again.then_some((player, true)))
        })
        .collect()
}

/// The option of an additional ballot that casts no vote.
const W17C_NO_VOTE: &str = "No additional vote";

/// How a name is shown in the question and in the options: the words of the card.
fn shown(name: &str) -> String {
    name.replace('_', " ")
}

/// One chooser for each ballot of the vote; an additional ballot also offers `blank`, which casts no vote.
fn w17c_choosers(
    cx: &Cx<'_>,
    options: &[ChoiceOption],
    blank: &ChoiceOption,
    text: &str,
) -> Vec<OptionChooser> {
    voters(cx)
        .into_iter()
        .map(|(player, again)| OptionChooser {
            player,
            options: options
                .iter()
                .chain(again.then_some(blank))
                .cloned()
                .collect(),
            text: text.to_owned(),
        })
        .collect()
}

/// The step is done with these names bound and goes on with the next effect.
fn done(bound: Vec<(String, Vec<Target>)>) -> Run {
    Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    }
}

/// Each voter chooses one of the names, one at a time (a secret vote is answered face down); each name is bound to the
/// players who voted for it.
fn named(cx: &Cx<'_>, names: &[String], secret: bool) -> Result<Run, ResolveError> {
    let options: Vec<ChoiceOption> = names
        .iter()
        .map(|name| {
            if secret {
                ChoiceOption::Secret(shown(name))
            } else {
                ChoiceOption::Label(shown(name))
            }
        })
        .collect();
    let text = format!(
        "Vote for {}.",
        names
            .iter()
            .map(|n| shown(n))
            .collect::<Vec<_>>()
            .join(" or ")
    );
    let blank = if secret {
        ChoiceOption::Secret(W17C_NO_VOTE.to_owned())
    } else {
        ChoiceOption::Label(W17C_NO_VOTE.to_owned())
    };
    let choosers = w17c_choosers(cx, &options, &blank, &text);
    let taken = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    let bound = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let for_it = taken
                .iter()
                .filter(|(_, picked)| *picked == index)
                .map(|(player, _)| Target::Player(*player))
                .collect();
            (name.clone(), for_it)
        })
        .collect();
    Ok(done(bound))
}

/// The targets with the most votes, each once, in the order they were first voted for.
fn leaders(votes: &[Target]) -> Vec<Target> {
    let count = |target: &Target| votes.iter().filter(|vote| *vote == target).count();
    let most = votes.iter().map(count).max().unwrap_or(0);
    let mut leading: Vec<Target> = Vec::new();
    for vote in votes {
        if count(vote) == most && !leading.contains(vote) {
            leading.push(*vote);
        }
    }
    leading
}

/// The ballots of a vote on players: each voter chooses one of the players, never themselves when `another`, face down
/// when `secret`; an additional ballot may be left blank. A voter with no player to vote for casts no vote.
fn r376_player_votes(
    cx: &Cx<'_>,
    players: &[PlayerId],
    another: bool,
    secret: bool,
) -> Result<Result<Vec<Target>, Run>, ResolveError> {
    let mut ballots: Vec<Vec<PlayerId>> = Vec::new();
    let mut choosers: Vec<OptionChooser> = Vec::new();
    for (voter, again) in voters(cx) {
        let open: Vec<PlayerId> = players
            .iter()
            .copied()
            .filter(|player| !another || *player != voter)
            .collect();
        if open.is_empty() {
            continue;
        }
        let mut options: Vec<ChoiceOption> = open
            .iter()
            .map(|player| {
                if secret {
                    ChoiceOption::SecretPlayer(*player)
                } else {
                    ChoiceOption::Player(*player)
                }
            })
            .collect();
        if again {
            options.push(if secret {
                ChoiceOption::Secret(W17C_NO_VOTE.to_owned())
            } else {
                ChoiceOption::Label(W17C_NO_VOTE.to_owned())
            });
        }
        ballots.push(open);
        choosers.push(OptionChooser {
            player: voter,
            options,
            text: if another {
                "Vote for another player.".to_owned()
            } else {
                "Vote for a player.".to_owned()
            },
        });
    }
    Ok(match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => Err(Run::Ask(decision)),
        Optioned::Done(taken) => Ok(taken
            .into_iter()
            .zip(&ballots)
            .filter_map(|((_, index), open)| open.get(index).copied().map(Target::Player))
            .collect()),
    })
}

/// The names a vote on players or objects binds: the leaders under `bind`, every vote under `tally`.
fn r376_bound(votes: &[Target], bind: Option<&str>, tally: Option<&str>) -> Vec<(String, Vec<Target>)> {
    bind.map(|name| (name.to_owned(), leaders(votes)))
        .into_iter()
        .chain(tally.map(|name| (name.to_owned(), votes.to_vec())))
        .collect()
}

/// Each voter chooses one of the players or objects the reference names (face down on players when `secret`); the
/// ones with the most votes, all of them when tied, are bound to `bind` and every vote to `tally`.
fn of(
    cx: &Cx<'_>,
    of: &mtg_cards::refs::Ref,
    secret: bool,
    bind: Option<&str>,
    tally: Option<&str>,
) -> Result<Run, ResolveError> {
    let candidates = cx.scope.refs(of)?;
    let players: Vec<PlayerId> = candidates.iter().filter_map(|t| t.player()).collect();
    let objects: Vec<ObjectId> = candidates
        .iter()
        .filter_map(|t| t.object())
        .map(|o| o.id)
        .collect();
    if !players.is_empty() && !objects.is_empty() {
        return Err(ResolveError::Invalid(
            "a vote is on players or on objects, not both".into(),
        ));
    }
    if secret && !objects.is_empty() {
        return Err(ResolveError::Invalid(
            "a vote on objects is never secret".into(),
        ));
    }
    let seats = voters(cx);
    let votes: Vec<Target> = if candidates.is_empty() {
        Vec::new()
    } else if objects.is_empty() {
        match r376_player_votes(cx, &players, false, secret)? {
            Ok(votes) => votes,
            Err(ask) => return Ok(ask),
        }
    } else {
        let selections: Vec<Selection> = seats
            .iter()
            .map(|(player, again)| Selection {
                one_per_name: false,
                player: *player,
                from: objects.clone(),
                min: usize::from(!again),
                max: 1,
                text: "Vote for a permanent or card.".to_owned(),
            })
            .collect();
        match pick_selections(cx, &selections)? {
            Picked::Ask(decision) => return Ok(Run::Ask(decision)),
            Picked::Done(taken) => taken
                .into_iter()
                .flat_map(|(_, cards)| cards)
                .filter_map(|id| {
                    candidates
                        .iter()
                        .find(|t| t.object().is_some_and(|o| o.id == id))
                })
                .copied()
                .collect(),
        }
    };
    Ok(done(r376_bound(&votes, bind, tally)))
}

/// The players vote: on named choices each name is bound to the players who voted for it, once for each of their votes;
/// on players or objects the name of the effect is bound to those with the most votes, all of them when tied.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Vote {
        options,
        secret,
        bind,
        tally,
    } = effect
    else {
        return Err(wrong_effect("Vote"));
    };
    if !matches!(options, VoteFor::Named(_)) && bind.is_none() && tally.is_none() {
        return Err(ResolveError::Invalid(
            "a vote on players or objects names its winners".into(),
        ));
    }
    let (bind, tally) = (bind.as_deref(), tally.as_deref());
    match options {
        VoteFor::Named(names) => named(cx, names, *secret),
        VoteFor::Of(reference) => of(cx, reference, *secret, bind, tally),
        VoteFor::AnotherPlayer => {
            let players = cx.state().turn_order_from(cx.scope.controller);
            match r376_player_votes(cx, &players, true, *secret)? {
                Ok(votes) => Ok(done(r376_bound(&votes, bind, tally))),
                Err(ask) => Ok(ask),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a vote asks the controller first and then the others in turn order whoever is the active player, offers the named choices (face down for a secret vote), offers players face down for a secret vote on them and every other player for a vote for another player at two to four seats, keeps every tied leader and nothing else, binds every vote to the tally, and refuses a secret vote on objects, a vote on players or objects that names no winner nor tally and another effect, by name.
    use rstest::rstest;

    use super::*;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    fn game(seats: usize) -> GameState {
        state(
            vec![vec![]; seats],
            std::iter::once(vec![creature("Source", 1, 1)])
                .chain(std::iter::repeat_n(vec![], seats - 1))
                .collect(),
        )
    }

    fn play(game: &GameState, controller: u64, effect: &str) -> Result<Run, ResolveError> {
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(controller), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(effect).unwrap())
    }

    const NAMED: &str = r#"{"Vote":{"options":{"Named":["time","knowledge"]}}}"#;

    #[rstest]
    #[case::the_active_player_controls_it(0, 0)]
    #[case::another_seat_controls_it(1, 1)]
    #[case::the_last_seat_controls_it(2, 2)]
    fn the_controller_votes_first(#[case] controller: u64, #[case] first: u64) {
        let Run::Ask(decision) = play(&game(3), controller, NAMED).unwrap() else {
            panic!("a vote asks the first voter");
        };
        assert_eq!(decision.player, PlayerId(first));
    }

    #[rstest]
    #[case::open(NAMED, false)]
    #[case::secret(
        r#"{"Vote":{"options":{"Named":["time","knowledge"]},"secret":true}}"#,
        true
    )]
    fn named_choices_are_offered_with_their_words_face_down_when_secret(
        #[case] json: &str,
        #[case] secret: bool,
    ) {
        let Run::Ask(decision) = play(&game(2), 0, json).unwrap() else {
            panic!("a vote asks the first voter");
        };
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("named choices are options");
        };
        let want = |word: &str| {
            if secret {
                ChoiceOption::Secret(word.to_owned())
            } else {
                ChoiceOption::Label(word.to_owned())
            }
        };
        assert_eq!(options.options, [want("time"), want("knowledge")]);
        assert_eq!(options.text, "Vote for time or knowledge.");
    }

    #[rstest]
    #[case::no_votes(&[], &[])]
    #[case::one_vote(&[3], &[3])]
    #[case::a_clear_leader(&[1, 2, 2, 3], &[2])]
    #[case::a_tie_keeps_both_in_the_order_they_were_voted_for(&[2, 1, 1, 2], &[2, 1])]
    #[case::everyone_different(&[1, 2, 3], &[1, 2, 3])]
    fn the_leaders_are_those_with_the_most_votes_each_once(
        #[case] votes: &[u64],
        #[case] leading: &[u64],
    ) {
        let targets: Vec<Target> = votes.iter().map(|n| Target::Player(PlayerId(*n))).collect();
        let want: Vec<Target> = leading
            .iter()
            .map(|n| Target::Player(PlayerId(*n)))
            .collect();
        assert_eq!(leaders(&targets), want);
    }

    #[rstest]
    #[case::secret_on_objects(
        r#"{"Vote":{"options":{"Of":{"Each":[{"Type":"Creature"}]}},"secret":true,"as":"won"}}"#
    )]
    #[case::no_name_for_the_winners(r#"{"Vote":{"options":{"Of":"EachPlayer"}}}"#)]
    #[case::no_name_for_another_player(r#"{"Vote":{"options":"AnotherPlayer","secret":true}}"#)]
    fn a_vote_on_players_that_cannot_be_played_is_refused(#[case] json: &str) {
        assert!(matches!(
            play(&game(2), 0, json),
            Err(ResolveError::Invalid(_))
        ));
    }

    #[rstest]
    #[case::three_seats(3, 0, &[1, 2])]
    #[case::three_seats_from_the_last(3, 2, &[0, 1])]
    #[case::four_seats_from_seat_two(4, 2, &[3, 0, 1])]
    fn a_vote_for_another_player_offers_every_other_player_face_down(
        #[case] seats: usize,
        #[case] controller: u64,
        #[case] offered: &[u64],
    ) {
        let json = r#"{"Vote":{"options":"AnotherPlayer","secret":true,"tally":"votes"}}"#;
        let Run::Ask(decision) = play(&game(seats), controller, json).unwrap() else {
            panic!("a vote asks the first voter");
        };
        assert_eq!(decision.player, PlayerId(controller));
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("players are options");
        };
        let want: Vec<ChoiceOption> = offered
            .iter()
            .map(|seat| ChoiceOption::SecretPlayer(PlayerId(*seat)))
            .collect();
        assert_eq!(options.options, want);
        assert_eq!(options.text, "Vote for another player.");
    }

    #[test]
    fn at_two_seats_each_votes_for_the_other_unasked() {
        let json = r#"{"Vote":{"options":"AnotherPlayer","secret":true,"tally":"votes"}}"#;
        let Run::Done { bound, .. } = play(&game(2), 0, json).unwrap() else {
            panic!("one option each is not asked");
        };
        assert_eq!(
            bound,
            vec![(
                "votes".to_owned(),
                vec![Target::Player(PlayerId(1)), Target::Player(PlayerId(0))]
            )]
        );
    }

    #[rstest]
    #[case::open(false)]
    #[case::secret(true)]
    fn a_vote_on_players_offers_each_player_face_down_when_secret(#[case] secret: bool) {
        let json = format!(
            r#"{{"Vote":{{"options":{{"Of":"EachPlayer"}},"secret":{secret},"as":"won"}}}}"#
        );
        let Run::Ask(decision) = play(&game(3), 0, &json).unwrap() else {
            panic!("a vote asks the first voter");
        };
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("players are options");
        };
        let want: Vec<ChoiceOption> = (0..3)
            .map(|seat| {
                if secret {
                    ChoiceOption::SecretPlayer(PlayerId(seat))
                } else {
                    ChoiceOption::Player(PlayerId(seat))
                }
            })
            .collect();
        assert_eq!(options.options, want);
    }

    #[rstest]
    #[case::leaders_only(Some("won"), None, &[("won", &[2][..])])]
    #[case::tally_only(None, Some("votes"), &[("votes", &[1, 2, 2][..])])]
    #[case::both(Some("won"), Some("votes"), &[("won", &[2][..]), ("votes", &[1, 2, 2][..])])]
    fn the_tally_holds_every_vote_and_the_leaders_each_once(
        #[case] bind: Option<&str>,
        #[case] tally: Option<&str>,
        #[case] want: &[(&str, &[u64])],
    ) {
        let votes: Vec<Target> = [1, 2, 2].iter().map(|n| Target::Player(PlayerId(*n))).collect();
        let want: Vec<(String, Vec<Target>)> = want
            .iter()
            .map(|(name, seats)| {
                (
                    (*name).to_owned(),
                    seats.iter().map(|n| Target::Player(PlayerId(*n))).collect(),
                )
            })
            .collect();
        assert_eq!(r376_bound(&votes, bind, tally), want);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(&game(2), 0, r#"{"Draw":{}}"#),
            Err(ResolveError::Invalid(message)) if message.contains("Vote")
        ));
    }
}
