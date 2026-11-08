//! "If you would flip a coin, instead flip two coins and ignore one" (`GameRule::FlipTwoCoins`, CR 705.3): the flipper
//! flips two coins for each such rule and keeps one. Only the kept flip is a flip that was won or lost.

use mtg_core::ids::PlayerId;

use crate::choose::{Choice, ChoiceOption, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cx, ResolveError, Run};
use crate::rng::GameRng;

/// The option that keeps a won flip.
const KEEP_WON: &str = "Keep a won flip";
/// The option that keeps a lost flip.
const KEEP_LOST: &str = "Keep a lost flip";

/// Whether `player`'s flip is won: every coin comes from the next substream of the game's random stream; when the coins
/// differ the flipper picks the one they keep, asked once and read back from the journal.
pub(super) fn ep22_result(
    cx: &Cx<'_>,
    player: PlayerId,
) -> Result<Result<bool, Run>, ResolveError> {
    let state = cx.state();
    let coins = crate::rules::ep22_coins_per_flip(state, player);
    let mut rng = GameRng::substream(state.seed(), state.rng_index());
    let flips: Vec<bool> = (0..coins).map(|_| rng.below(2) == 0).collect();
    let mut answers = cx.journal().iter();
    let mixed = flips.contains(&true) && flips.contains(&false);
    if !mixed {
        if answers.next().is_some() {
            return Err(ResolveError::Invalid(
                "an answer was given to a choice of a coin that was not asked".into(),
            ));
        }
        return Ok(Ok(flips.first().copied().unwrap_or(false)));
    }
    let won = match answers.next() {
        None => return Ok(Err(Run::Ask(question(cx, player, coins)))),
        Some((who, Choice::Option(index))) if *who == player && *index < 2 => *index == 0,
        Some(other) => {
            return Err(ResolveError::Invalid(format!(
                "{other:?} is no answer to the choice of a coin to keep"
            )));
        }
    };
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of a coin that was not asked".into(),
        ));
    }
    Ok(Ok(won))
}

/// "You flipped N coins with both results. Choose the one you keep."
fn question(cx: &Cx<'_>, player: PlayerId, coins: u32) -> Decision {
    Decision {
        player,
        kind: DecisionKind::ChooseOption(OptionOptions {
            can_cancel: false,
            source: cx.scope.source.id,
            options: vec![
                ChoiceOption::Label(KEEP_WON.into()),
                ChoiceOption::Label(KEEP_LOST.into()),
            ],
            text: format!(
                "You flipped {coins} coins, won and lost; keep one and ignore the others."
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: with no rule a flip is one coin and asks nothing; each rule that doubles a player's coins doubles them again,
    //! the flipper keeps a won or a lost flip when the coins differ and nothing is asked when they agree; another player's
    //! rule does not double the coins, and an answer that was not asked or names no option is refused.
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

    fn result(game: &GameState, answers: &[usize]) -> Result<Result<bool, Run>, ResolveError> {
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
        ep22_result(&cx, ME)
    }

    /// The coins `player` flips in `game`.
    fn coins(game: &GameState, player: PlayerId) -> u32 {
        crate::rules::ep22_coins_per_flip(game, player)
    }

    #[rstest]
    #[case::no_rule(&[], 1)]
    #[case::one_thumb(&[ME], 2)]
    #[case::two_thumbs(&[ME, ME], 4)]
    #[case::an_opponent_s_thumb(&[FOE], 1)]
    fn each_rule_doubles_the_coins(#[case] doublers: &[PlayerId], #[case] expected: u32) {
        assert_eq!(coins(&game(0, doublers), ME), expected);
    }

    #[test]
    fn one_coin_asks_nothing() {
        for seed in 0..16 {
            assert!(result(&game(seed, &[]), &[]).unwrap().is_ok());
        }
    }

    #[rstest]
    #[case::keep_the_win(0, true)]
    #[case::keep_the_loss(1, false)]
    fn the_flipper_keeps_one_of_two_coins_that_differ(#[case] answer: usize, #[case] won: bool) {
        let mut asked = 0;
        let mut agreed = 0;
        for seed in 0..32 {
            let game = game(seed, &[ME]);
            match result(&game, &[]).unwrap() {
                Err(Run::Ask(_)) => {
                    asked += 1;
                    assert_eq!(result(&game, &[answer]).unwrap().ok(), Some(won));
                    assert!(result(&game, &[2]).is_err());
                    assert!(result(&game, &[answer, answer]).is_err());
                }
                Ok(_) => {
                    agreed += 1;
                    assert!(result(&game, &[answer]).is_err());
                }
                Err(_) => panic!("only a question or a result"),
            }
        }
        assert!(asked > 0 && agreed > 0, "{asked} asked, {agreed} agreed");
    }
}
