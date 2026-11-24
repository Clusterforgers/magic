//! Goal: a bot seat answers a payment that only an announced mana ability can pay (Vexing Puzzlebox's roll, Skirge Familiar's discard) by activating it from the payment, its brain's answer accepted by the table, and so casts the spell priority offered it, at two and four seats and at the Easy and Normal levels.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::Scenario;
use mtg_testkit::brain::{Level, SeatBrain, brain};
use rstest::rstest;

/// The most decisions the table is played for.
const DECISIONS: usize = 60;

#[rstest]
#[case::puzzlebox(&["Grizzly Bears"], &["Vexing Puzzlebox", "Forest"])]
#[case::skirge_familiar(&["Grizzly Bears", "Forest"], &["Skirge Familiar", "Forest"])]
fn a_bot_activates_the_ability_from_the_payment_and_casts(
    #[case] hand: &[&str],
    #[case] battlefield: &[&str],
    #[values(2, 4)] seats: usize,
    #[values(Level::Easy, Level::Normal)] level: Level,
) {
    let mut engine = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield_named(0, battlefield.iter().copied())
        .build()
        .unwrap();
    let me = PlayerId(0);
    let bears = engine.state().zone(Zone::Hand(me))[0];
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..seats)
        .map(|seat| brain(level, 7, PlayerId::from_index(seat), false))
        .collect();
    let mut activated = false;
    let mut cast = false;
    for _ in 0..DECISIONS {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let answer = match &decision.kind {
            // The table offers the Bears though the lands alone cannot pay: the seat casts them.
            DecisionKind::Priority(options)
                if decision.player == me && !cast && options.castable.contains(&bears) =>
            {
                Answer::Priority(PriorityAction::CastSpell(bears))
            }
            _ => brains[decision.player.index()].choose(&decision, Some(&engine)),
        };
        if let (DecisionKind::PayMana(options), Answer::PayMana(PayChoice::Activate(ability))) =
            (&decision.kind, &answer)
        {
            assert!(!options.auto_available);
            assert!(options.announced.contains(ability));
            activated = true;
        }
        let events = match engine.apply(answer.clone()) {
            Ok(events) => events,
            Err(refused) => panic!("{answer:?} refused ({refused}) for {decision:?}"),
        };
        if events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { .. }))
        {
            cast = true;
            break;
        }
        if engine.result().is_some() {
            break;
        }
    }
    assert!(activated, "the bot activated the ability from the payment");
    assert!(cast, "the Bears were cast");
}
