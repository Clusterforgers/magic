//! Goal: each heuristic of the Normal and Hard brains on a set table: attacks (lethal, suicide, evasion), blocks (to
//! survive, good trades, no needless chumps), removal at the biggest threat and never at a teammate, ramp and lands
//! first, mulligans by land count, the least useful card discarded, and answers that ignore the cards a seat cannot see.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, PresetPermanent};
use mtg_testkit::bot::apply_answer;
use mtg_testkit::brain::choices::{keep, least_useful};
use mtg_testkit::brain::{Level, SeatBrain, Seen, brain};
use mtg_testkit::brain_decks::{Duel, duel_setup};
use mtg_testkit::cards::{creature, forest, free_creature, keyworded, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// A body: power, toughness, keywords.
type Stat = (i32, i32, &'static [Keyword]);
const NONE: &[Keyword] = &[];
const FLYING: &[Keyword] = &[Keyword::Flying];
const FIRST: &[Keyword] = &[Keyword::FirstStrike];
const DEATHTOUCH: &[Keyword] = &[Keyword::Deathtouch];

/// A brain of `level` for `seat`, without a clock.
fn bot(level: Level, seat: usize) -> Box<dyn SeatBrain> {
    brain(level, 7, pid(seat), false)
}

/// Creatures by power and toughness, with `keywords`, named for their place.
fn bodies(side: &str, stats: &[Stat]) -> Vec<PresetPermanent> {
    stats
        .iter()
        .enumerate()
        .map(|(at, (power, toughness, keywords))| {
            ready(keyworded(
                &format!("{side} {at}"),
                *power,
                *toughness,
                keywords,
            ))
        })
        .collect()
}

/// The ids of the creatures of `player` on the battlefield, in table order.
fn creatures_of(engine: &Engine, player: usize) -> Vec<ObjectId> {
    let state = engine.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.controller() == pid(player) && o.card().power.is_some())
        })
        .collect()
}

/// Passes priority for whoever holds it until the pending decision is not a priority decision.
fn pass_to_next_question(engine: &mut Engine) -> Decision {
    for _ in 0..50 {
        let decision = engine.decision().cloned().unwrap();
        if !matches!(decision.kind, DecisionKind::Priority(_)) {
            return decision;
        }
        engine
            .apply(Answer::Priority(PriorityAction::Pass))
            .unwrap();
    }
    panic!("no question came");
}

/// Seat 0 about to declare attackers with `mine` against seat 1 at `life` with `theirs`.
fn attack_table(mine: &[Stat], theirs: &[Stat], life: i32) -> Engine {
    Scenario::new(2)
        .at(3, 0, Step::DeclareAttackers)
        .battlefield(0, bodies("Mine", mine))
        .battlefield(1, bodies("Theirs", theirs))
        .life(1, life)
        .build()
        .unwrap()
}

#[rstest]
#[case::lethal_alpha_strike(&[(2, 2, NONE), (2, 2, NONE), (2, 2, NONE)], &[(2, 2, NONE)], 4, 3)]
#[case::no_attack_into_a_bigger_blocker(&[(2, 2, NONE)], &[(4, 4, NONE)], 20, 0)]
#[case::a_free_swing(&[(3, 3, NONE)], &[], 20, 1)]
#[case::a_flyer_over_the_ground(&[(2, 2, FLYING)], &[(4, 4, NONE)], 20, 1)]
#[case::first_strike_wins_the_fight(&[(3, 3, FIRST)], &[(2, 3, NONE)], 20, 1)]
fn attacks_follow_the_simulated_outcome(
    #[case] mine: &[Stat],
    #[case] theirs: &[Stat],
    #[case] life: i32,
    #[case] attackers: usize,
    #[values(Level::Normal, Level::Hard)] level: Level,
) {
    let engine = attack_table(mine, theirs, life);
    let decision = engine.decision().cloned().unwrap();
    assert!(matches!(decision.kind, DecisionKind::DeclareAttackers(_)));
    let Answer::DeclareAttackers(attack) = bot(level, 0).choose(&decision, Some(&engine)) else {
        panic!("not an attack");
    };
    assert_eq!(attack.len(), attackers, "{attack:?}");
    assert!(
        attack
            .iter()
            .all(|(_, target)| *target == AttackTarget::Player(pid(1)))
    );
}

#[rstest]
#[case::chump_to_survive((5, 5, NONE), &[(1, 1, NONE)], 4, true)]
#[case::no_chump_at_high_life((3, 3, NONE), &[(1, 1, NONE)], 20, false)]
#[case::a_good_trade((2, 2, NONE), &[(3, 3, NONE)], 20, true)]
#[case::deathtouch_trades_up((4, 4, NONE), &[(1, 1, DEATHTOUCH)], 20, true)]
#[case::no_block_into_first_strike((2, 2, FIRST), &[(2, 1, NONE)], 20, false)]
fn blocks_prevent_lethal_and_take_good_trades(
    #[case] attacker: Stat,
    #[case] mine: &[Stat],
    #[case] life: i32,
    #[case] blocks: bool,
    #[values(Level::Normal, Level::Hard)] level: Level,
) {
    let mut engine = Scenario::new(2)
        .at(3, 0, Step::DeclareAttackers)
        .battlefield(0, bodies("Theirs", &[attacker]))
        .battlefield(1, bodies("Mine", mine))
        .life(1, life)
        .build()
        .unwrap();
    let raider = creatures_of(&engine, 0)[0];
    engine
        .apply(Answer::DeclareAttackers(vec![(
            raider,
            AttackTarget::Player(pid(1)),
        )]))
        .unwrap();
    let decision = pass_to_next_question(&mut engine);
    assert!(matches!(decision.kind, DecisionKind::DeclareBlockers(_)));
    let Answer::DeclareBlockers(declared) = bot(level, 1).choose(&decision, Some(&engine)) else {
        panic!("not a block");
    };
    assert_eq!(!declared.is_empty(), blocks, "{declared:?}");
}

/// Plays `seat`'s answers with its brain and passes for everyone else until the stack is empty again after the seat
/// acted, or `limit` answers.
fn play_out(engine: &mut Engine, brain: &mut Box<dyn SeatBrain>, seat: usize, limit: usize) {
    let mut acted = false;
    for _ in 0..limit {
        let Some(decision) = engine.decision().cloned() else {
            return;
        };
        let answer = if decision.player == pid(seat) {
            brain.choose(&decision, Some(engine))
        } else {
            Answer::Priority(PriorityAction::Pass)
        };
        if decision.player == pid(seat) && answer != Answer::Priority(PriorityAction::Pass) {
            acted = true;
        }
        apply_answer(engine, &decision, answer).unwrap();
        if acted
            && engine.state().stack().is_empty()
            && let Some(DecisionKind::Priority(_)) = engine.decision().map(|d| &d.kind)
        {
            return;
        }
    }
}

/// Whether the creature named `name` is in a graveyard.
fn died(engine: &Engine, name: &str) -> bool {
    let state = engine.state();
    state.players().iter().any(|player| {
        state
            .zone(Zone::Graveyard(player.id()))
            .iter()
            .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
    })
}

#[rstest]
fn removal_goes_at_the_biggest_threat(#[values(Level::Normal, Level::Hard)] level: Level) {
    let mut engine = Scenario::new(2)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Lightning Bolt").unwrap()])
        .battlefield(0, vec![ready(mountain())])
        .battlefield(
            1,
            vec![
                ready(free_creature("Small", 1, 1)),
                ready(free_creature("Big", 3, 3)),
            ],
        )
        .build()
        .unwrap();
    let mut brain = bot(level, 0);
    play_out(&mut engine, &mut brain, 0, 40);
    assert!(died(&engine, "Big"));
    assert!(!died(&engine, "Small"));
}

#[rstest]
fn removal_spares_teammates(#[values(Level::Normal, Level::Hard)] level: Level) {
    // Seats 0 and 2 play together against seats 1 and 3; the biggest creature on the table is the teammate's.
    let house = HouseRules {
        teams: Some(Teams::even(TeamMode::Teams, 2, 2)),
        ..HouseRules::default()
    };
    let mut engine = Scenario::new(4)
        .house_rules(house)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Lightning Bolt").unwrap()])
        .battlefield(0, vec![ready(mountain())])
        .battlefield(1, vec![ready(free_creature("Foe", 3, 3))])
        .battlefield(
            2,
            vec![
                ready(free_creature("Friend", 3, 3)),
                ready(free_creature("Friend Two", 2, 2)),
            ],
        )
        .build()
        .unwrap();
    let mut brain = bot(level, 0);
    play_out(&mut engine, &mut brain, 0, 60);
    assert!(died(&engine, "Foe"));
    assert!(!died(&engine, "Friend") && !died(&engine, "Friend Two"));
    assert_eq!(engine.state().player(pid(2)).unwrap().life(), 20);
}

#[rstest]
fn ramp_and_the_right_land_come_first(#[values(Level::Normal, Level::Hard)] level: Level) {
    let elves = card("Llanowar Elves").unwrap();
    let engine = Scenario::new(2)
        .at(3, 0, Step::Main1)
        .hand(0, [elves, card("Grizzly Bears").unwrap()])
        .battlefield(0, vec![ready(forest()), ready(forest())])
        .build()
        .unwrap();
    let decision = engine.decision().cloned().unwrap();
    let Answer::Priority(PriorityAction::CastSpell(cast)) =
        bot(level, 0).choose(&decision, Some(&engine))
    else {
        panic!("no spell cast");
    };
    assert_eq!(
        engine.state().object(cast).unwrap().card().name,
        "Llanowar Elves"
    );

    let red: Arc<CardDefinition> = creature("Red One", 0, &[Color::Red], 2, 1);
    let engine = Scenario::new(2)
        .at(3, 0, Step::Main1)
        .hand(0, [forest(), mountain(), red])
        .build()
        .unwrap();
    let decision = engine.decision().cloned().unwrap();
    let Answer::Priority(PriorityAction::PlayLand(land)) =
        bot(level, 0).choose(&decision, Some(&engine))
    else {
        panic!("no land played");
    };
    assert_eq!(engine.state().object(land).unwrap().card().name, "Mountain");
}

#[rstest]
fn a_hand_is_kept_with_two_to_five_lands(#[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64) {
    let engine =
        Engine::new(duel_setup(&[Duel::Gruul, Duel::Orzhov], seed, HouseRules::default()).unwrap())
            .unwrap();
    for decision in engine.decisions() {
        let DecisionKind::Mulligan(options) = &decision.kind else {
            continue;
        };
        let seen = Seen::new(engine.state(), decision.player);
        let lands = options
            .hand
            .iter()
            .filter(|id| {
                engine
                    .state()
                    .object(**id)
                    .unwrap()
                    .card()
                    .types
                    .contains(&mtg_core::card::CardType::Land)
            })
            .count();
        assert_eq!(
            keep(&seen, options),
            (2..=5).contains(&lands),
            "{lands} lands"
        );
    }
}

#[test]
fn the_least_useful_card_is_discarded() {
    let lands: Vec<_> = (0..7).map(|_| ready(forest())).collect();
    let engine = Scenario::new(2)
        .at(3, 0, Step::Main2)
        .hand(
            0,
            [
                forest(),
                creature("Huge", 9, &[Color::Green], 9, 9),
                creature("Bear", 1, &[Color::Green], 2, 2),
            ],
        )
        .battlefield(0, lands)
        .build()
        .unwrap();
    let seen = Seen::new(engine.state(), pid(0));
    let hand = seen.own_hand().to_vec();
    let out = least_useful(&seen, &hand, 1);
    assert_eq!(engine.state().object(out[0]).unwrap().card().name, "Forest");
    // Without lands in play the land is the card kept and the spell out of reach goes.
    let engine = Scenario::new(2)
        .at(3, 0, Step::Main2)
        .hand(
            0,
            [
                forest(),
                creature("Huge", 9, &[Color::Green], 9, 9),
                creature("Bear", 1, &[Color::Green], 2, 2),
            ],
        )
        .build()
        .unwrap();
    let seen = Seen::new(engine.state(), pid(0));
    let hand = seen.own_hand().to_vec();
    let out = least_useful(&seen, &hand, 1);
    assert_eq!(engine.state().object(out[0]).unwrap().card().name, "Huge");
}

/// A table where seat 1's hand holds four of `in_hand` and its library four of `in_library`, the rest fixed.
fn hidden_table(
    in_hand: fn() -> Arc<CardDefinition>,
    in_library: fn() -> Arc<CardDefinition>,
) -> Engine {
    Scenario::new(2)
        .at(3, 0, Step::Main1)
        .hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Grizzly Bears").unwrap(),
            ],
        )
        .battlefield(
            0,
            vec![
                ready(mountain()),
                ready(forest()),
                ready(free_creature("Mine", 2, 2)),
            ],
        )
        .hand(1, (0..4).map(|_| in_hand()))
        .library(1, (0..4).map(|_| in_library()))
        .battlefield(1, vec![ready(free_creature("Theirs", 2, 3))])
        .build()
        .unwrap()
}

#[rstest]
fn answers_ignore_the_cards_a_seat_cannot_see(#[values(Level::Normal, Level::Hard)] level: Level) {
    // Searching worlds redeal seat 1's unseen cards from its hand and library together (its deck as a whole is known),
    // so the two tables hold the same cards and differ only in which of them are in the hand.
    let giant = || free_creature("Hidden Giant", 9, 9);
    let mut tables =
        vec![hidden_table(forest, giant), hidden_table(giant, forest)].into_boxed_slice();
    let mut brains = [bot(level, 0), bot(level, 0)];
    for _ in 0..30 {
        let decisions: Vec<Decision> = tables
            .iter()
            .map(|t| t.decision().cloned().unwrap())
            .collect();
        assert_eq!(decisions[0].player, decisions[1].player);
        let answers: Vec<Answer> = if decisions[0].player == pid(0) {
            let a = brains[0].choose(&decisions[0], Some(&tables[0]));
            let b = brains[1].choose(&decisions[1], Some(&tables[1]));
            vec![a, b]
        } else {
            vec![Answer::Priority(PriorityAction::Pass); 2]
        };
        assert_eq!(answers[0], answers[1]);
        if decisions[0].player != pid(0) && !matches!(decisions[0].kind, DecisionKind::Priority(_))
        {
            break;
        }
        for (table, (decision, answer)) in tables.iter_mut().zip(decisions.iter().zip(answers)) {
            apply_answer(table, decision, answer).unwrap();
        }
        if tables[0].state().turn().number() > 3 {
            break;
        }
    }
    let _: PlayerId = pid(1);
}

#[rstest]
#[case::unblocked(&[(3, 3, NONE), (2, 2, NONE)], &[], 5)]
#[case::one_blocked(&[(3, 3, NONE), (2, 2, NONE)], &[(1, 1, NONE)], 2)]
#[case::flyer_over(&[(3, 3, FLYING)], &[(5, 5, NONE)], 3)]
fn the_threat_of_next_turn_is_what_gets_past_the_blockers(
    #[case] attackers: &[Stat],
    #[case] blockers: &[Stat],
    #[case] through: i32,
) {
    let engine = Scenario::new(2)
        .at(3, 1, Step::Main1)
        .battlefield(0, bodies("Attacker", attackers))
        .battlefield(1, bodies("Blocker", blockers))
        .build()
        .unwrap();
    let seen = Seen::new(engine.state(), pid(1));
    let bodies = seen.bodies();
    let team = |seat: usize| engine.state().team_of(pid(seat));
    assert_eq!(
        mtg_testkit::brain::eval::pressure(&seen, &bodies, team(0), team(1)),
        through
    );
}
