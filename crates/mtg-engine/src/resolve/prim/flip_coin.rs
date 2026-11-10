//! `Effect::FlipCoin`: a player calls heads or tails and flips a coin (CR 705), and the resolution goes on with the list
//! for the result.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::{ChoiceOption, CoinSide, OptionChooser, Optioned, pick_options};
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rng::GameRng;

/// The sides a player calls between, in the order they are offered: an answer that takes the first calls heads.
pub(super) const SIDES: [CoinSide; 2] = [CoinSide::Heads, CoinSide::Tails];

/// The question the call is asked with.
pub(super) const CALL: &str = "Call heads or tails.";

/// The side a flip comes up from the next substream of the game's random stream: heads or tails at even odds.
pub(crate) fn side(seed: u64, index: u64) -> CoinSide {
    SIDES[usize::from(GameRng::substream(seed, index).below(2) != 0)]
}

/// The first player the effect names who is still in the game calls heads or tails (`ChooseOption`) and flips: the
/// side comes from the next substream of the game's random stream, the flip is won when it matches the call, it is
/// logged as `CoinFlipped` and the step goes on into `win` or `lose`. Nobody in the game to flip is no flip.
/// A flip until a loss flips again after a win: after the `win` list (the tree comes back to the flip), or at once.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::FlipCoin {
        who, win, until, ..
    } = effect
    else {
        return Err(wrong_effect("FlipCoin"));
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
    // Krark's Thumb: two coins or more for one flip, one kept (`ep22_two_coins`).
    let (call, side) = if crate::rules::ep22_coins_per_flip(state, player) > 1 {
        match super::ep22_two_coins::ep22_flip(cx, player)? {
            Ok(found) => found,
            Err(asked) => return Ok(asked),
        }
    } else {
        let chooser = OptionChooser {
            player,
            options: SIDES.map(ChoiceOption::Side).to_vec(),
            text: CALL.to_owned(),
        };
        let call = match pick_options(cx, &[chooser])? {
            Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
            Optioned::Done(taken) => taken
                .first()
                .and_then(|(_, index)| SIDES.get(*index).copied())
                .unwrap_or_default(),
        };
        (call, side(state.seed(), state.rng_index()))
    };
    let won = call == side;
    Ok(Run::Done {
        events: vec![Event::CoinFlipped {
            player,
            won,
            call,
            side,
        }],
        bound: Vec::new(),
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
    //! Goal: the flipping player is asked to call heads or tails; the side comes from the game's random stream (the same seed gives the same side, the seeds give both), the flip is won exactly when the call matches it, it is logged with the call and the side for the player who flipped and enters the list for the result, a won flip until a loss flipping again; a player who has left the game flips nothing, and another effect is refused by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn game(seed: u64) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        game.seed = seed;
        game
    }

    /// The first seed whose next flip comes up `want`.
    fn seed_for(want: CoinSide) -> u64 {
        (0..64)
            .find(|seed| {
                let game = game(*seed);
                side(game.seed(), game.rng_index()) == want
            })
            .unwrap()
    }

    fn flip_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        json: &str,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
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

    /// The flip once `call` is called.
    fn called(game: &GameState, json: &str, call: CoinSide) -> (Vec<Event>, Next) {
        let index = SIDES.iter().position(|side| *side == call).unwrap();
        let Run::Done { events, next, .. } =
            flip_with(game, vec![(ME, Choice::Option(index))], json).unwrap()
        else {
            panic!("a called flip asks nothing more");
        };
        (events, next)
    }

    const FLIP: &str = r#"{"FlipCoin":{"win":[{"Draw":{}}],"lose":[{"Shuffle":{}}]}}"#;

    #[test]
    fn the_flipping_player_is_asked_to_call_heads_or_tails() {
        let Run::Ask(decision) = flip_with(&game(1), Vec::new(), FLIP).unwrap() else {
            panic!("the call is asked first");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("the call is a choice of option");
        };
        assert_eq!(
            options.options,
            vec![
                ChoiceOption::Side(CoinSide::Heads),
                ChoiceOption::Side(CoinSide::Tails)
            ]
        );
        assert_eq!(options.text, CALL);
    }

    #[rstest]
    fn a_flip_is_won_exactly_when_the_call_matches_the_side(
        #[values(CoinSide::Heads, CoinSide::Tails)] call: CoinSide,
        #[values(CoinSide::Heads, CoinSide::Tails)] landed: CoinSide,
    ) {
        let (events, next) = called(&game(seed_for(landed)), FLIP, call);
        let won = call == landed;
        assert_eq!(
            events,
            vec![Event::CoinFlipped {
                player: ME,
                won,
                call,
                side: landed,
            }]
        );
        assert_eq!(
            next,
            Next::Enter(usize::from(!won)),
            "the list for the result"
        );
    }

    #[test]
    fn the_same_seed_flips_the_same_side_and_the_seeds_give_both() {
        let sides: Vec<CoinSide> = (0..32)
            .map(|seed| {
                let game = game(seed);
                side(game.seed(), game.rng_index())
            })
            .collect();
        assert_eq!(
            sides,
            (0..32)
                .map(|seed| {
                    let game = game(seed);
                    side(game.seed(), game.rng_index())
                })
                .collect::<Vec<_>>()
        );
        assert!(sides.contains(&CoinSide::Heads) && sides.contains(&CoinSide::Tails));
    }

    #[rstest]
    #[case::a_yes_for_a_call(r#"{"FlipCoin":{"win":[{"Draw":{}}]}}"#)]
    fn an_answer_of_another_kind_is_refused(#[case] json: &str) {
        let error = flip_with(&game(1), vec![(ME, Choice::YesNo(true))], json).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)), "{error}");
    }

    #[rstest]
    #[case::the_player_who_left(r#"{"FlipCoin":{"who":"Opponents","win":[{"Draw":{}}]}}"#)]
    fn a_player_who_has_left_flips_nothing(#[case] json: &str) {
        let mut game = game(1);
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let Run::Done { events, next, .. } = flip_with(&game, Vec::new(), json).unwrap() else {
            panic!("nobody flips, so nobody calls");
        };
        assert_eq!(events.len(), 0);
        assert_eq!(next, Next::Sibling);
    }

    #[rstest]
    #[case::no_list(r#"{"FlipCoin":{"until":true}}"#, true)]
    #[case::a_win_list(r#"{"FlipCoin":{"until":true,"win":[{"Draw":{}}]}}"#, false)]
    fn c8_vote_a_won_flip_until_a_loss_flips_again_or_enters_the_win_list(
        #[case] json: &str,
        #[case] again: bool,
        #[values(CoinSide::Heads, CoinSide::Tails)] call: CoinSide,
    ) {
        for seed in 0..16 {
            let (events, next) = called(&game(seed), json, call);
            let [Event::CoinFlipped { won, .. }] = events.as_slice() else {
                panic!("one flip is logged: {events:?}");
            };
            let expected = match (*won, again) {
                (true, true) => Next::Again,
                (true, false) => Next::Enter(0),
                (false, _) => Next::Enter(1),
            };
            assert_eq!(next, expected, "seed {seed}");
        }
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let error = flip_with(&game(1), Vec::new(), r#"{"Draw":{}}"#).unwrap_err();
        assert!(error.to_string().contains("FlipCoin"), "{error}");
    }
}
