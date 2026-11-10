//! "If you would flip a coin, instead flip two coins and ignore one" (`GameRule::FlipTwoCoins`, CR 705.3): the flipper
//! calls once, flips two coins for each such rule and keeps one. Only the kept flip is a flip that was won or lost.

use mtg_core::ids::PlayerId;

use super::flip_coin::{CALL, SIDES};
use crate::choose::{Choice, ChoiceOption, CoinSide, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cx, ResolveError, Run};
use crate::rng::GameRng;

/// The option that keeps a won flip.
const KEEP_WON: &str = "Keep a won flip";
/// The option that keeps a lost flip.
const KEEP_LOST: &str = "Keep a lost flip";

/// The call and the side of the coin kept, or the question: the call is asked first; every coin comes from the next
/// substream of the game's random stream (the first as a single flip would come up); when the coins differ the flipper
/// picks a won or a lost one. The answers are read back from the journal in that order.
pub(super) fn ep22_flip(
    cx: &Cx<'_>,
    player: PlayerId,
) -> Result<Result<(CoinSide, CoinSide), Run>, ResolveError> {
    let state = cx.state();
    let mut answers = cx.journal().iter();
    let call = match answers.next() {
        None => {
            let options = SIDES.map(ChoiceOption::Side).to_vec();
            return Ok(Err(Run::Ask(question(
                cx,
                player,
                options,
                CALL.to_owned(),
            ))));
        }
        Some((who, Choice::Option(index))) if *who == player && *index < SIDES.len() => {
            SIDES[*index]
        }
        Some(other) => {
            return Err(ResolveError::Invalid(format!(
                "{other:?} is no answer to the call of a coin"
            )));
        }
    };
    let coins = crate::rules::ep22_coins_per_flip(state, player);
    let mut rng = GameRng::substream(state.seed(), state.rng_index());
    let sides: Vec<CoinSide> = (0..coins)
        .map(|_| SIDES[usize::from(rng.below(2) != 0)])
        .collect();
    let other = if call == CoinSide::Heads {
        CoinSide::Tails
    } else {
        CoinSide::Heads
    };
    let kept = if sides.iter().all(|side| *side == sides[0]) {
        sides[0]
    } else {
        match answers.next() {
            None => {
                let options = vec![
                    ChoiceOption::Label(KEEP_WON.into()),
                    ChoiceOption::Label(KEEP_LOST.into()),
                ];
                let text = format!(
                    "You flipped {coins} coins, won and lost; keep one and ignore the others."
                );
                return Ok(Err(Run::Ask(question(cx, player, options, text))));
            }
            Some((who, Choice::Option(0))) if *who == player => call,
            Some((who, Choice::Option(1))) if *who == player => other,
            Some(other) => {
                return Err(ResolveError::Invalid(format!(
                    "{other:?} is no answer to the choice of a coin to keep"
                )));
            }
        }
    };
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of a coin that was not asked".into(),
        ));
    }
    Ok(Ok((call, kept)))
}

/// One `ChooseOption` of the flipper.
fn question(cx: &Cx<'_>, player: PlayerId, options: Vec<ChoiceOption>, text: String) -> Decision {
    Decision {
        player,
        kind: DecisionKind::ChooseOption(OptionOptions {
            can_cancel: false,
            source: cx.scope.source.id,
            options,
            text,
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each rule that doubles a player's coins doubles them again and another player's rule does not; with two coins
    //! the flipper calls first, keeps a won or a lost coin when the coins differ and is asked nothing more when they agree;
    //! an answer that was not asked or names no option is refused.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use mtg_core::ids::EffectId;
    use rstest::rstest;

    use super::*;
    use crate::effects::Duration;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};
    use crate::rules::{Rule, RuleEffect};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game(seed: u64, doublers: &[PlayerId]) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        game.seed = seed;
        for who in doublers {
            let effect = RuleEffect {
                id: EffectId(game.next_effect),
                rule: Rule::FlipTwoCoins(vec![*who]),
                duration: Duration::Forever,
                source: None,
                controller: *who,
                turn: 1,
            };
            apply_event(
                &mut game,
                &Event::RuleCreated {
                    effect: effect.into(),
                },
            )
            .unwrap();
        }
        game
    }

    type Flipped = Result<Result<(CoinSide, CoinSide), Run>, ResolveError>;

    fn flip(game: &GameState, answers: &[usize]) -> Flipped {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal: answers
                .iter()
                .map(|index| (ME, Choice::Option(*index)))
                .collect(),
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        ep22_flip(&cx, ME)
    }

    #[rstest]
    #[case::no_rule(&[], 1)]
    #[case::one_thumb(&[ME], 2)]
    #[case::two_thumbs(&[ME, ME], 4)]
    #[case::an_opponent_s_thumb(&[FOE], 1)]
    fn each_rule_doubles_the_coins(#[case] doublers: &[PlayerId], #[case] expected: u32) {
        assert_eq!(
            crate::rules::ep22_coins_per_flip(&game(0, doublers), ME),
            expected
        );
    }

    #[test]
    fn the_call_is_asked_first() {
        assert!(matches!(flip(&game(0, &[ME]), &[]), Ok(Err(Run::Ask(_)))));
        assert!(flip(&game(0, &[ME]), &[2]).is_err());
    }

    #[rstest]
    #[case::keep_the_win(0, true)]
    #[case::keep_the_loss(1, false)]
    fn the_flipper_keeps_one_of_two_coins_that_differ(#[case] answer: usize, #[case] won: bool) {
        let (mut asked, mut agreed) = (0, 0);
        for seed in 0..32 {
            let game = game(seed, &[ME]);
            match flip(&game, &[0]).unwrap() {
                Err(Run::Ask(_)) => {
                    asked += 1;
                    let (call, kept) = flip(&game, &[0, answer]).unwrap().unwrap();
                    assert_eq!(call == kept, won);
                    assert!(flip(&game, &[0, 2]).is_err());
                    assert!(flip(&game, &[0, answer, answer]).is_err());
                }
                Ok(_) => {
                    agreed += 1;
                    assert!(flip(&game, &[0, answer]).is_err());
                }
                Err(other) => panic!("only a question or a result: {other:?}"),
            }
        }
        assert!(asked > 0 && agreed > 0, "{asked} asked, {agreed} agreed");
    }
}
