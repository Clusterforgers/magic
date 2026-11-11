//! `Effect::BidLife`: the controller opens the bidding, then in turn order each other bidder may top the high bid, and
//! the bidding ends when the high bid stands (it comes back around to the high bidder). The answers are read back from
//! the journal: a yes or no to topping, then the new bid.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{Choice, NumberOptions, Target, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::object::Chosen;
use crate::resolve::{CHOSEN_VALUE, Cx, Next, ResolveError, Run};

/// Where the bidding stands: the high bid and its bidder, or the next question.
enum Bidding {
    Ended { high: u32, bidder: PlayerId },
    Ask(Decision),
}

fn number(cx: &Cx<'_>, player: PlayerId, min: u32, text: String) -> Decision {
    Decision {
        player,
        kind: DecisionKind::ChooseNumber(NumberOptions {
            source: cx.scope.source.id,
            min,
            max: None,
            except: Vec::new(),
            text,
            can_cancel: false,
            cost: None,
            per_x: 0,
            secret: false,
        }),
    }
}

/// Replays the journal over the bidders (the controller first, in turn order) and finds where the bidding stands.
fn bidding(cx: &Cx<'_>, bidders: &[PlayerId], start: Option<u32>) -> Result<Bidding, ResolveError> {
    let wrong = |what: &str| ResolveError::Invalid(format!("a bid of life expected {what}"));
    let mut journal = cx.journal().iter();
    let opener = bidders[0];
    let mut high = match start {
        Some(start) => start,
        None => match journal.next() {
            None => {
                let text = "Bid life: choose your opening bid.".to_owned();
                return Ok(Bidding::Ask(number(cx, opener, 0, text)));
            }
            Some((who, Choice::Number(bid))) if *who == opener => *bid,
            Some(_) => return Err(wrong("the opening bid")),
        },
    };
    let mut bidder = 0;
    let mut at = 0;
    loop {
        at = (at + 1) % bidders.len();
        if at == bidder {
            return Ok(Bidding::Ended {
                high,
                bidder: bidders[bidder],
            });
        }
        let player = bidders[at];
        match journal.next() {
            None => {
                let text = format!("Top the high bid of {high} life?");
                return Ok(Bidding::Ask(Decision {
                    player,
                    kind: DecisionKind::YesNo(YesNoOptions::new(cx.scope.source.id, text)),
                }));
            }
            Some((who, Choice::YesNo(false))) if *who == player => {}
            Some((who, Choice::YesNo(true))) if *who == player => match journal.next() {
                None => {
                    let text = format!("Bid more than {high} life.");
                    return Ok(Bidding::Ask(number(
                        cx,
                        player,
                        high.saturating_add(1),
                        text,
                    )));
                }
                Some((who, Choice::Number(bid))) if *who == player && *bid > high => {
                    high = *bid;
                    bidder = at;
                }
                Some(_) => return Err(wrong("a higher bid")),
            },
            Some(_) => return Err(wrong("a yes or no to topping the bid")),
        }
    }
}

/// The bidders bid; when the high bid stands, it and its bidder are bound. Nobody to bid binds nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::BidLife {
        who,
        start,
        bind,
        winner,
    } = effect
    else {
        return Err(wrong_effect("BidLife"));
    };
    let named = cx.scope.players(who)?;
    let bidders: Vec<PlayerId> = cx
        .state()
        .turn_order_from(cx.scope.controller)
        .into_iter()
        .filter(|player| named.contains(player) && cx.state().is_alive(*player))
        .collect();
    let mut bound = Vec::new();
    if !bidders.is_empty() {
        let start = match start {
            Some(start) => Some(u32::try_from(cx.scope.amount(start)?.max(0)).unwrap_or(u32::MAX)),
            None => None,
        };
        let (high, bidder) = match bidding(cx, &bidders, start)? {
            Bidding::Ask(decision) => return Ok(Run::Ask(decision)),
            Bidding::Ended { high, bidder } => (high, bidder),
        };
        if let Ok(json) = serde_json::to_string(&Chosen::Number(high)) {
            bound.push((format!("{CHOSEN_VALUE}{bind}\t{json}"), Vec::new()));
        }
        bound.push((winner.clone(), vec![Target::Player(bidder)]));
    }
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the controller opens (with the bid the card names or any number), each other bidder in turn order is asked to top the high bid and then for more than it, the bidding ends when the high bid comes back around to its bidder, and the high bid and bidder are bound; an answer out of turn or not above the high bid is refused.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ANY: &str = r#"{"BidLife":{"as":"bid","winner":"bidder"}}"#;
    const FROM_ZERO: &str = r#"{"BidLife":{"start":0,"as":"bid","winner":"bidder"}}"#;

    fn game(seats: usize) -> GameState {
        state(
            vec![vec![]; seats],
            std::iter::once(vec![creature("Source", 1, 1)])
                .chain(std::iter::repeat_n(vec![], seats - 1))
                .collect(),
        )
    }

    fn play(
        seats: usize,
        controller: u64,
        json: &str,
        journal: &[(u64, Choice)],
    ) -> Result<Run, ResolveError> {
        let game = game(seats);
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving {
            journal: journal
                .iter()
                .map(|(player, choice)| (PlayerId(*player), choice.clone()))
                .collect(),
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(&game, PlayerId(controller), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    fn asked(run: Run) -> (u64, &'static str, u32) {
        let Run::Ask(decision) = run else {
            panic!("a question was due: {run:?}");
        };
        match decision.kind {
            DecisionKind::YesNo(_) => (decision.player.0, "top", 0),
            DecisionKind::ChooseNumber(options) => (decision.player.0, "bid", options.min),
            other => panic!("{other:?}"),
        }
    }

    use Choice::{Number as N, YesNo as Y};

    #[rstest]
    #[case::the_opening_bid(ANY, 1, &[], (1, "bid", 0))]
    #[case::the_next_seat_may_top(ANY, 1, &[(1, N(3))], (2, "top", 0))]
    #[case::a_top_asks_more(ANY, 1, &[(1, N(3)), (2, Y(true))], (2, "bid", 4))]
    #[case::around_the_table(ANY, 1, &[(1, N(3)), (2, Y(false))], (0, "top", 0))]
    #[case::a_fixed_opening_asks_the_next(FROM_ZERO, 0, &[], (1, "top", 0))]
    #[case::back_to_the_opener_after_a_top(ANY, 0, &[(0, N(1)), (1, Y(true)), (1, N(2)), (2, Y(false))], (0, "top", 0))]
    fn the_bidding_asks_in_turn_order(
        #[case] json: &str,
        #[case] controller: u64,
        #[case] journal: &[(u64, Choice)],
        #[case] next: (u64, &str, u32),
    ) {
        assert_eq!(asked(play(3, controller, json, journal).unwrap()), next);
    }

    #[rstest]
    #[case::nobody_tops(ANY, 3, 0, &[(0, N(5)), (1, Y(false)), (2, Y(false))], 5, 0)]
    #[case::the_last_top_stands(ANY, 3, 0, &[(0, N(1)), (1, Y(true)), (1, N(4)), (2, Y(false)), (0, Y(false))], 4, 1)]
    #[case::a_fixed_opening_stands(FROM_ZERO, 2, 1, &[(0, Y(false))], 0, 1)]
    fn the_high_bid_stands_and_is_bound(
        #[case] json: &str,
        #[case] seats: usize,
        #[case] controller: u64,
        #[case] journal: &[(u64, Choice)],
        #[case] high: u32,
        #[case] bidder: u64,
    ) {
        let Run::Done { bound, .. } = play(seats, controller, json, journal).unwrap() else {
            panic!("the bidding ended");
        };
        let value = format!(
            "{CHOSEN_VALUE}bid\t{}",
            serde_json::to_string(&Chosen::Number(high)).unwrap()
        );
        assert_eq!(
            bound,
            vec![
                (value, Vec::new()),
                ("bidder".to_owned(), vec![Target::Player(PlayerId(bidder))]),
            ]
        );
    }

    #[rstest]
    #[case::not_above_the_high_bid(&[(0, N(3)), (1, Y(true)), (1, N(3))])]
    #[case::out_of_turn(&[(0, N(3)), (2, Y(false))])]
    #[case::another_kind(&[(0, Y(true))])]
    fn a_wrong_answer_is_refused(#[case] journal: &[(u64, Choice)]) {
        assert!(matches!(
            play(3, 0, ANY, journal),
            Err(ResolveError::Invalid(_))
        ));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(2, 0, r#"{"Draw":{}}"#, &[]),
            Err(ResolveError::Invalid(message)) if message.contains("BidLife")
        ));
    }
}
