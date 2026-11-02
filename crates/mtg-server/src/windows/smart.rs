//! Smart stops: a step where a permanent of the seat can answer becomes a window for a seat that turned them on.
//!
//! Whether a step is such a window is read from public facts alone ([`can_answer_now`]: the seat's permanents, their
//! abilities' timing, limits, conditions and `{T}`, never a cost), so an opponent who sees the seat paused learns
//! nothing a look at the battlefield and the seat's settings would not tell. Inside the window the seat is asked when
//! it can use such an ability, and passed for after a dwell otherwise, as at a default window: paying can read the hand
//! (a discard), and the dwell keeps that out of the pace. A card in hand alone never opens or fills the window.

use mtg_core::ids::PlayerId;
use mtg_engine::GameState;
use mtg_engine::decision::PriorityOptions;
use mtg_engine::smart_stops::can_answer_now;
use mtg_engine::zones::Zone;

use super::{Verdict, Window};

/// The verdict at `window` once smart stops (`wanted`: turned on and no yield running) have had their say.
///
/// Only a pass at once at a window that is not a default one changes: there a permanent of `seat` that its timing lets
/// answer makes a window, asked when `options` hold an ability of a permanent and dwelt through otherwise.
pub(super) fn widen(
    judged: Verdict,
    window: Window,
    wanted: bool,
    state: &GameState,
    seat: PlayerId,
    options: &PriorityOptions,
) -> Verdict {
    if judged != Verdict::Pass || !wanted || window.is_default() || !can_answer_now(state, seat) {
        return judged;
    }
    let usable = options.abilities.iter().any(|ability| {
        state
            .object(ability.object)
            .is_some_and(|object| object.zone() == Zone::Battlefield)
    });
    if usable { Verdict::Ask } else { Verdict::Dwell }
}

#[cfg(test)]
mod tests {
    //! Goal: with smart stops on, a step that is no default window becomes one exactly where a permanent of the seat has an ability its timing lets it use, read without the hand; the seat is asked when it can pay and dwelt through otherwise, whatever spell it holds; default windows, stops, Full Control and yields decide as before, and with smart stops off nothing changes.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::{Ability, Timing};
    use mtg_cards::cost::{Cost, CostItem};
    use mtg_cards::refs::ZoneKind;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use mtg_engine::Engine;
    use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
    use mtg_engine::turn::Step;
    use mtg_engine::{Answer, decision::Decision};
    use mtg_protocol::{AutoChoices, FullControlMode, YieldUntil};
    use mtg_testkit::cards::{burn, gain_life, instant, mountain};
    use mtg_testkit::{Scenario, ready};
    use rstest::rstest;

    use crate::stops::Stops;
    use crate::windows::{Verdict, verdict};
    use crate::yields::Yields;

    const ME: PlayerId = PlayerId(1);

    use Verdict::{Ask, Dwell, Pass};

    /// The windows of the table, two of them default ones.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Spot {
        OwnUpkeep,
        FoeMain,
        FoeEnd,
        OwnMain,
        FoeSpell,
    }

    /// The spots that are no default window, where smart stops act.
    const QUIET: [Spot; 3] = [Spot::OwnUpkeep, Spot::FoeMain, Spot::FoeEnd];
    /// The default windows, which smart stops leave alone.
    const DEFAULT: [Spot; 2] = [Spot::OwnMain, Spot::FoeSpell];

    /// An artifact with one ability, "`cost`: You gain 1 life.", at `timing`.
    fn relic(cost: Vec<CostItem>, timing: Timing) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Relic")
                .with_type(CardType::Artifact)
                .with_rules(Ability::Activated {
                    cost: Cost(cost),
                    timing,
                    condition: None,
                    limit: None,
                    from: ZoneKind::Battlefield,
                    targets: Vec::new(),
                    effects: vec![gain_life(1)],
                }),
        )
    }

    /// What a seat controls besides a Mountain.
    #[derive(Debug, Clone, Copy)]
    enum Board {
        Nothing,
        Tap,
        TwoMana,
        AtSorcerySpeed,
        Discard,
    }

    fn permanents(board: Board) -> Vec<Arc<CardDefinition>> {
        let mut out = vec![mountain()];
        let mana = |text: &str| CostItem::Mana(text.parse().unwrap());
        out.extend(match board {
            Board::Nothing => None,
            Board::Tap => Some(relic(vec![CostItem::Tap], Timing::Instant)),
            Board::TwoMana => Some(relic(vec![mana("{2}")], Timing::Instant)),
            Board::AtSorcerySpeed => Some(relic(vec![CostItem::Tap], Timing::Sorcery)),
            Board::Discard => Some(relic(
                vec![CostItem::Discard {
                    count: mtg_cards::value::Amount::Fixed(1),
                    filter: None,
                }],
                Timing::Instant,
            )),
        });
        out
    }

    fn pass(engine: &mut Engine) {
        engine
            .apply(Answer::Priority(PriorityAction::Pass))
            .unwrap();
    }

    /// The game at `spot`, at the priority decision of `ME`, who holds `board` and, when `bolt`, a castable instant.
    fn at(spot: Spot, board: Board, bolt: bool) -> Engine {
        let (active, step) = match spot {
            Spot::OwnUpkeep => (1, Step::Upkeep),
            Spot::OwnMain => (1, Step::Main1),
            Spot::FoeMain | Spot::FoeSpell => (0, Step::Main1),
            Spot::FoeEnd => (0, Step::End),
        };
        let hand = if bolt {
            vec![burn("Bolt", 3)]
        } else {
            Vec::new()
        };
        let mut engine = Scenario::new(2)
            .at(2, active, step)
            .hand(1, hand)
            .hand(0, [instant("Foe Zap", 0, &[])])
            .battlefield(1, permanents(board).into_iter().map(ready))
            .build()
            .unwrap();
        match spot {
            Spot::OwnUpkeep | Spot::OwnMain => {}
            Spot::FoeMain | Spot::FoeEnd => pass(&mut engine),
            Spot::FoeSpell => {
                let DecisionKind::Priority(options) = &engine.decision().unwrap().kind else {
                    panic!("priority was expected");
                };
                let zap = options.castable[0];
                engine
                    .apply(Answer::Priority(PriorityAction::CastSpell(zap)))
                    .unwrap();
                while matches!(
                    engine.decision().map(|d| &d.kind),
                    Some(DecisionKind::PayMana(_))
                ) {
                    engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
                }
                pass(&mut engine);
            }
        }
        assert_eq!(engine.decision().unwrap().player, ME, "{spot:?}");
        engine
    }

    fn stops(smart: bool, mode: FullControlMode) -> Stops {
        let mut stops = Stops::new(2);
        stops.set_full_control(ME, mode);
        stops.set_auto_choices(
            ME,
            AutoChoices {
                smart_stops: smart,
                ..AutoChoices::default()
            },
        );
        stops
    }

    fn judged(engine: &Engine, stops: &Stops, yields: &mut Yields) -> Verdict {
        let decision: &Decision = engine.decision().unwrap();
        verdict(decision, stops, yields, engine.state())
    }

    /// Verdicts with smart stops on at the quiet spots, without and with a castable instant in hand: whether the step
    /// is a window never depends on the hand, and a spell in hand never makes the seat asked.
    #[rstest]
    #[case::no_ability(Board::Nothing, [Pass, Pass])]
    #[case::a_free_ability(Board::Tap, [Ask, Ask])]
    #[case::an_ability_it_cannot_pay(Board::TwoMana, [Dwell, Dwell])]
    #[case::an_ability_at_sorcery_speed(Board::AtSorcerySpeed, [Pass, Pass])]
    #[case::a_discard_cost_paid_from_the_hand(Board::Discard, [Dwell, Ask])]
    fn a_quiet_step_is_a_window_where_a_permanent_can_answer_whatever_the_hand(
        #[case] board: Board,
        #[case] expected: [Verdict; 2],
    ) {
        for spot in QUIET {
            for (bolt, expected) in [false, true].into_iter().zip(expected) {
                let engine = at(spot, board, bolt);
                let got = judged(
                    &engine,
                    &stops(true, FullControlMode::Off),
                    &mut Yields::new(2),
                );
                assert_eq!(got, expected, "{spot:?} {board:?} bolt {bolt}");
                let off = judged(
                    &engine,
                    &stops(false, FullControlMode::Off),
                    &mut Yields::new(2),
                );
                assert_eq!(off, Pass, "smart stops off: {spot:?} {board:?} bolt {bolt}");
            }
        }
    }

    /// Default windows, Full Control and a yield give the verdict they give with smart stops off.
    #[rstest]
    fn smart_stops_leave_default_windows_full_control_and_yields_alone(
        #[values(Board::Nothing, Board::Tap, Board::TwoMana, Board::Discard)] board: Board,
        #[values(false, true)] bolt: bool,
    ) {
        for spot in DEFAULT {
            let engine = at(spot, board, bolt);
            let on = judged(
                &engine,
                &stops(true, FullControlMode::Off),
                &mut Yields::new(2),
            );
            let off = judged(
                &engine,
                &stops(false, FullControlMode::Off),
                &mut Yields::new(2),
            );
            assert_eq!(on, off, "{spot:?} {board:?} bolt {bolt}");
        }
        for spot in QUIET {
            let engine = at(spot, board, bolt);
            for mode in [FullControlMode::NextWindow, FullControlMode::Held] {
                let got = judged(&engine, &stops(true, mode), &mut Yields::new(2));
                assert_eq!(got, Ask, "{spot:?} {board:?} {mode:?}");
            }
            let mut yields = Yields::new(2);
            yields.start(ME, YieldUntil::EndOfTurn, engine.state());
            let got = judged(&engine, &stops(true, FullControlMode::Off), &mut yields);
            assert_eq!(got, Pass, "a yield hurries past {spot:?} {board:?}");
        }
    }
}
