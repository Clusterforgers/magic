//! `Effect::Pay`: a player may pay mana, and what follows depends on whether they do ("unless that player pays {1}").

use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;

use super::wrong_effect;
use crate::cast::payment_decision;
use crate::choose::{Choice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::mana::can_afford;
use crate::price::{Leaves, Price};
use crate::priority::mana_options;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::state::GameState;
use crate::wording;

/// The players the effect names who are in the game are asked in turn order whether they will pay ("Pay {2}?"), until
/// one does; a player who cannot pay, with the floating mana and the permanents they may tap, is not asked and takes
/// no part. A yes leads to the payment decision (`PayMana`, which cannot be backed out of), and the `paid` effects
/// follow once it is made; when nobody pays, the `unpaid` effects follow.
///
/// A step with k questions runs k + 1 times. The journal holds, for each player asked, a no or a yes and then, for a
/// player who paid, a second yes that the payment's answer logs along with its events (`cast::pay_answer`). A
/// player's entries are read before the player is judged again, since paying has spent what they could pay with; the
/// entries of a player who has left the game since are not read, and a player who left before paying has not paid.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Pay { who, cost, .. } = effect else {
        return Err(wrong_effect("Pay"));
    };
    let cost = mana_of(cost)?;
    let state = cx.state();
    let mut journal = cx
        .journal()
        .iter()
        .filter(|entry| state.is_alive(entry.0))
        .peekable();
    let mut paid = false;
    for player in cx.scope.players(who)? {
        let Some((_, asked)) = journal.next_if(|entry| entry.0 == player) else {
            if can_pay(state, player, &cost) {
                return Ok(Run::Ask(question(cx, player, &cost)));
            }
            continue;
        };
        match asked {
            Choice::YesNo(false) => {}
            Choice::YesNo(true) => match journal.next_if(|entry| entry.0 == player) {
                Some((_, Choice::YesNo(true))) => {
                    paid = true;
                    break;
                }
                None => return Ok(Run::Ask(payment(cx, player, cost)?)),
                Some(other) => return Err(unexpected(&other.1, player)),
            },
            other => return Err(unexpected(other, player)),
        }
    }
    if journal.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a question that was not asked".into(),
        ));
    }
    Ok(Run::Done {
        events: Vec::new(),
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Enter(usize::from(!paid)),
    })
}

fn unexpected(answer: &Choice, player: PlayerId) -> ResolveError {
    ResolveError::Invalid(format!(
        "{answer:?} is no answer to the payment put to {player:?}"
    ))
}

/// The mana of a cost that is only mana: the one cost an effect can ask for today.
fn mana_of(cost: &Cost) -> Result<ManaCost, ResolveError> {
    match Price::of(cost) {
        // Losing the source is a cost of an ability; no effect asks a player to pay it yet.
        Ok(Price {
            leaves: Leaves::Sacrificed,
            ..
        }) => Err(ResolveError::Unsupported("Effect.Pay.SacrificeSelf")),
        Ok(Price {
            leaves: Leaves::Exiled,
            ..
        }) => Err(ResolveError::Unsupported("Effect.Pay.ExileSelf")),
        Ok(Price {
            leaves: Leaves::Discarded,
            ..
        }) => Err(ResolveError::Unsupported("Effect.Pay.DiscardSelf")),
        Ok(Price {
            tap: false,
            mana: Some(mana),
            ..
        }) => Ok(mana),
        Ok(_) => Err(ResolveError::Invalid(
            "a payment is mana and nothing else".into(),
        )),
        Err(name) => Err(ResolveError::Unsupported(name)),
    }
}

/// Whether the player is in the game and has the mana for `cost`: floating or from the permanents they may tap.
fn can_pay(state: &GameState, player: PlayerId, cost: &ManaCost) -> bool {
    state.is_alive(player)
        && state.player(player).is_some_and(|seat| {
            can_afford(cost, seat.mana(), &mana_options(state, player), seat.life())
        })
}

/// "Pay {2}?" for the player.
fn question(cx: &Cx<'_>, player: PlayerId, cost: &ManaCost) -> Decision {
    Decision {
        player,
        kind: DecisionKind::YesNo(YesNoOptions {
            source: cx.scope.source.id,
            text: wording::pay(cost),
        }),
    }
}

/// The payment decision of a player who said yes: no way back, since saying no was the way out.
fn payment(cx: &Cx<'_>, player: PlayerId, cost: ManaCost) -> Result<Decision, ResolveError> {
    let state = cx.state();
    payment_decision(
        state,
        player,
        cx.scope.source.id,
        cost,
        mana_options(state, player),
        false,
    )
    .map(|options| Decision {
        player,
        kind: DecisionKind::PayMana(options),
    })
    .ok_or_else(|| ResolveError::Invalid(format!("{player:?} is not in the game")))
}

#[cfg(test)]
mod tests {
    //! Goal: a payment asks the players it names in turn order "Pay {2}?" until one says yes and then for the payment itself (no way back), skips a player who left the game or cannot pay, enters the paid effects once a payment is logged and the unpaid ones when nobody paid, reads a player's answers before judging them again (paying spends what could pay), refuses answers it did not ask for, and refuses by name a cost that is not plain mana.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::color::Color;
    use rstest::rstest;

    use super::*;
    use crate::decision::PayOptions;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::player::LossReason;
    use crate::resolve::{Resolving, Scope};
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(Color::Green)))
    }

    /// Seat 0 controls the source and the first of `lands` Forests; seat `n` the `lands[n]`th.
    fn game(lands: &[usize]) -> GameState {
        let battlefield = |seat: usize, extra: Vec<Arc<CardDefinition>>| {
            let mut cards = vec![forest(); lands[seat]];
            cards.extend(extra);
            cards
        };
        let mut battlefields = vec![battlefield(0, vec![creature("Source", 1, 1)])];
        battlefields.extend((1..lands.len()).map(|seat| battlefield(seat, Vec::new())));
        state(vec![Vec::new(); lands.len()], battlefields)
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn pay(who: &str, cost: &str) -> Effect {
        effect(&format!(
            r#"{{"Pay":{{"who":{who},"cost":{cost},"paid":[{{"Draw":{{}}}}],"unpaid":[{{"Draw":{{}}}}]}}}}"#
        ))
    }

    fn run_with(
        game: &GameState,
        journal: &[(usize, bool)],
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let journal = journal
            .iter()
            .map(|(seat, yes)| (PlayerId::from_index(*seat), Choice::YesNo(*yes)))
            .collect();
        run_journal(game, journal, effect)
    }

    fn run_journal(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| game.object(*id))
            .find(|o| o.card().name == "Source")
            .unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    /// What the effect leaves: who is asked what, or where the resolution goes next.
    #[derive(Debug, PartialEq, Eq)]
    enum Said {
        Asks(usize, String),
        Pays(usize, String, bool),
        Goes(Next),
    }

    fn said(run: Result<Run, ResolveError>) -> Said {
        match run.unwrap() {
            Run::Ask(Decision {
                player,
                kind: DecisionKind::YesNo(options),
            }) => Said::Asks(player.index(), options.text),
            Run::Ask(Decision {
                player,
                kind: DecisionKind::PayMana(options),
            }) => Said::Pays(player.index(), options.cost.to_string(), options.can_cancel),
            Run::Ask(other) => panic!("unexpected question {other:?}"),
            Run::Done {
                events,
                bound,
                snapshots,
                next,
            } => {
                assert!(events.is_empty() && bound.is_empty() && snapshots.is_empty());
                Said::Goes(next)
            }
        }
    }

    const PAID: Said = Said::Goes(Next::Enter(0));
    const UNPAID: Said = Said::Goes(Next::Enter(1));

    fn asks(seat: usize) -> Said {
        Said::Asks(seat, "Pay {2}?".into())
    }

    #[test]
    fn the_player_is_asked_in_the_words_of_the_card_and_then_for_the_payment_without_a_way_back() {
        let game = game(&[2, 0]);
        let pay = pay(r#""You""#, r#""{2}""#);
        assert_eq!(said(run_with(&game, &[], &pay)), asks(0));
        assert_eq!(
            said(run_with(&game, &[(0, true)], &pay)),
            Said::Pays(0, "{2}".into(), false)
        );
    }

    #[test]
    fn the_payment_decision_offers_the_sources_of_the_payer_for_the_source_of_the_effect() {
        let game = game(&[2, 0]);
        let Ok(Run::Ask(Decision {
            kind: DecisionKind::PayMana(options),
            ..
        })) = run_with(&game, &[(0, true)], &pay(r#""You""#, r#""{2}""#))
        else {
            panic!("a payment was expected");
        };
        let PayOptions {
            spell,
            sources,
            auto_available,
            can_cancel,
            ..
        } = options;
        assert_eq!(game.object(spell).unwrap().card().name, "Source");
        assert_eq!(sources.len(), 2);
        assert!(auto_available && !can_cancel);
    }

    #[rstest]
    #[case::no(&[(0, false)], UNPAID)]
    #[case::paid(&[(0, true), (0, true)], PAID)]
    fn the_answers_decide_which_effects_follow(
        #[case] given: &[(usize, bool)],
        #[case] said_: Said,
    ) {
        // The payment spent the lands, so the player could not pay again: the answers are read first.
        let game = game(&[0, 0]);
        assert_eq!(
            said(run_with(&game, given, &pay(r#""You""#, r#""{2}""#))),
            said_
        );
    }

    #[rstest]
    #[case::no_lands(&[0, 0], UNPAID)]
    #[case::one_land_for_two(&[1, 0], UNPAID)]
    #[case::two_lands(&[2, 0], asks(0))]
    fn a_player_who_cannot_pay_is_not_asked(#[case] lands: &[usize], #[case] said_: Said) {
        let game = game(lands);
        assert_eq!(
            said(run_with(&game, &[], &pay(r#""You""#, r#""{2}""#))),
            said_
        );
    }

    #[test]
    fn the_cost_must_be_met_in_colours_and_with_untapped_lands() {
        let wrong_colour = game(&[2, 0]);
        let red = pay(r#""You""#, r#""{R}""#);
        assert_eq!(said(run_with(&wrong_colour, &[], &red)), UNPAID);
        let mut tapped = game(&[2, 0]);
        let lands: Vec<_> = tapped
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .filter(|id| tapped.object(*id).unwrap().card().name == "Forest")
            .collect();
        for land in lands {
            apply_event(&mut tapped, &Event::PermanentTapped { object: land }).unwrap();
        }
        assert_eq!(
            said(run_with(&tapped, &[], &pay(r#""You""#, r#""{1}""#))),
            UNPAID
        );
    }

    #[rstest]
    #[case::the_first_is_asked(&[3, 3, 3, 3], &[], asks(1))]
    #[case::the_first_says_no(&[3, 3, 3, 3], &[(1, false)], asks(2))]
    #[case::two_say_no(&[3, 3, 3, 3], &[(1, false), (2, false)], asks(3))]
    #[case::all_say_no(&[3, 3, 3, 3], &[(1, false), (2, false), (3, false)], UNPAID)]
    #[case::the_second_pays(&[3, 3, 3, 3], &[(1, false), (2, true)], Said::Pays(2, "{2}".into(), false))]
    #[case::the_second_has_paid(&[3, 3, 3, 3], &[(1, false), (2, true), (2, true)], PAID)]
    #[case::the_first_pays_and_nobody_else_is_asked(&[3, 3, 3, 3], &[(1, true), (1, true)], PAID)]
    #[case::the_first_cannot_pay(&[3, 0, 3, 3], &[], asks(2))]
    #[case::the_second_is_asked_after_a_player_who_could_not_pay_was_skipped(&[3, 0, 3, 3], &[(2, false)], asks(3))]
    #[case::none_can_pay(&[3, 0, 1, 1], &[], UNPAID)]
    fn several_players_are_asked_in_turn_order_until_one_pays(
        #[case] lands: &[usize],
        #[case] given: &[(usize, bool)],
        #[case] said_: Said,
    ) {
        let game = game(lands);
        let opponents = pay(r#""Opponents""#, r#""{2}""#);
        assert_eq!(said(run_with(&game, given, &opponents)), said_);
    }

    #[test]
    fn a_player_who_left_the_game_is_not_asked_and_has_not_paid() {
        let mut game = game(&[0, 3, 3]);
        let opponents = pay(r#""Opponents""#, r#""{2}""#);
        assert_eq!(said(run_with(&game, &[], &opponents)), asks(1));
        apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: LossReason::Conceded,
            },
        )
        .unwrap();
        assert_eq!(said(run_with(&game, &[], &opponents)), asks(2));
        // Said yes and left before paying: their answer is not read, the payment is not made and the next player is
        // asked; the answers of the others are read as before.
        assert_eq!(said(run_with(&game, &[(1, true)], &opponents)), asks(2));
        assert_eq!(
            said(run_with(&game, &[(1, true), (2, false)], &opponents)),
            UNPAID
        );
        assert_eq!(
            said(run_with(&game, &[(1, true), (2, true)], &opponents)),
            Said::Pays(2, "{2}".into(), false)
        );
    }

    #[test]
    fn nobody_is_asked_when_nobody_is_named() {
        // A target that was never chosen names nobody: nothing was paid.
        let game = game(&[3, 3]);
        assert_eq!(
            said(run_with(&game, &[], &pay(r#"{"Target":0}"#, r#""{2}""#))),
            UNPAID
        );
    }

    #[rstest]
    #[case::wrong_kind(vec![(ME, Choice::Number(1))])]
    #[case::a_no_with_a_payment(vec![(ME, Choice::YesNo(false)), (ME, Choice::YesNo(true))])]
    #[case::an_answer_after_the_payment(vec![(ME, Choice::YesNo(true)), (ME, Choice::YesNo(true)), (ME, Choice::YesNo(true))])]
    #[case::a_yes_then_another_kind(vec![(ME, Choice::YesNo(true)), (ME, Choice::Option(0))])]
    fn an_answer_to_a_question_that_was_not_asked_is_refused(
        #[case] journal: Vec<(PlayerId, Choice)>,
    ) {
        let game = game(&[2, 2]);
        let error = run_journal(&game, journal, &pay(r#""You""#, r#""{2}""#)).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)), "{error:?}");
    }

    #[test]
    fn an_answer_of_a_player_the_effect_does_not_name_is_refused() {
        // The player named cannot pay and is not asked, so the answer of the other seat answers nothing.
        let game = game(&[0, 2]);
        let error = run_with(&game, &[(1, true)], &pay(r#""You""#, r#""{2}""#)).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)), "{error:?}");
    }

    #[rstest]
    #[case::life(
        r#"[{"Mana":"{1}"},{"PayLife":2}]"#,
        ResolveError::Unsupported("Cost.PayLife")
    )]
    #[case::sacrifice(
        r#"["SacrificeSelf"]"#,
        ResolveError::Unsupported("Effect.Pay.SacrificeSelf")
    )]
    #[case::mana_and_sacrifice(
        r#"[{"Mana":"{2}"},"SacrificeSelf"]"#,
        ResolveError::Unsupported("Effect.Pay.SacrificeSelf")
    )]
    #[case::sacrificing_another(
        r#"[{"Sacrifice":{"filter":[]}}]"#,
        ResolveError::Unsupported("Cost.Sacrifice")
    )]
    #[case::x(r#""{X}""#, ResolveError::Unsupported("Cost.Mana.X"))]
    #[case::tapping(
        r#"[{"Mana":"{1}"},"Tap"]"#,
        ResolveError::Invalid("a payment is mana and nothing else".into())
    )]
    #[case::nothing("[]", ResolveError::Invalid("a payment is mana and nothing else".into()))]
    fn a_cost_that_is_not_plain_mana_is_refused_by_name(
        #[case] cost: &str,
        #[case] expected: ResolveError,
    ) {
        let game = game(&[2, 0]);
        let error = run_with(&game, &[], &pay(r#""You""#, cost)).unwrap_err();
        assert_eq!(error, expected);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let game = game(&[0, 0]);
        let error = run_with(&game, &[], &effect(r#"{"Draw":{}}"#)).unwrap_err();
        assert_eq!(
            error,
            ResolveError::Invalid("the Pay primitive was given another effect".into())
        );
    }
}
