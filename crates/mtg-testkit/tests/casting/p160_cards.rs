//! Goal: the p160 cards play whole, at two to four seats: Rod of Absorption exiles each instant and sorcery as it
//! resolves, its caster keeping priority in that step, and, sacrificed for X, casts the cards it exiled one at a time while their mana values fit X; Baron Helmut
//! Zemo's boast exiles black cards with fifteen or more black mana symbols and casts up to three of their copies; Nicol
//! Bolas, Dragon-God has the loyalty abilities of every other planeswalker, still one activation a turn, and his +1 has
//! each opponent exile a card from their hand or a permanent; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::choose::{CardsChoice, NumberChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, with_counters};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn replays(driver: &Driver) {
    let state = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    })
}

/// Answers every question until the stack is empty and seat 0 has priority again: mana is paid automatically, the
/// first legal targets are taken, `pick` answers what it knows and every yes-or-no is yes.
fn settle(driver: &mut Driver, pick: &dyn Fn(&Decision) -> Option<Answer>) {
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(&decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == pid(0) =>
            {
                return;
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min.max(1) as usize)
                    .collect(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not settle");
}

/// Takes the first card offered by a choice of cards that may take one, and X as `x`.
fn first_card(x: u32) -> impl Fn(&Decision) -> Option<Answer> {
    move |decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(CardsChoice(
            options
                .from
                .iter()
                .copied()
                .take(options.max.min(1).max(options.min) as usize)
                .collect(),
        ))),
        DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(x))),
        _ => None,
    }
}

fn count_in(driver: &Driver, zone: Zone, prefix: &str) -> usize {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name.starts_with(prefix))
        .count()
}

/// A spell of the kind for `{generic}` with "Draw a card."
fn drawing(name: &str, kind: CardType, generic: u32, colored: &[Color]) -> Arc<CardDefinition> {
    let json = r#"{"Spell":{"effects":[{"Draw":{}}]}}"#;
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(generic, colored))
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

#[rstest]
#[case::all_fit(3, 6, true, 3)]
#[case::the_first_two_fit(3, 3, true, 2)]
#[case::only_the_first_fits(3, 1, true, 1)]
#[case::x_zero_casts_none(3, 0, true, 0)]
#[case::the_player_stops(3, 6, false, 0)]
fn rod_of_absorption_casts_its_cards_within_x(
    #[values(2, 3, 4)] seats: usize,
    #[case] spells: u32,
    #[case] x: u32,
    #[case] takes: bool,
    #[case] recast: usize,
) {
    // Spell n costs {n}: an instant, a sorcery, an instant.
    let hand: Vec<_> = (1..=spells)
        .map(|n| {
            let kind = if n % 2 == 0 {
                CardType::Sorcery
            } else {
                CardType::Instant
            };
            drawing(&format!("Spell {n}"), kind, n, &[])
        })
        .collect();
    let mut board = vec![ready(card("Rod of Absorption").unwrap())];
    board.extend((0..12).map(|_| ready(island())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, hand);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    for n in 1..=spells {
        driver.begin_cast(&format!("Spell {n}")).unwrap();
        settle(&mut driver, &first_card(x));
    }
    let exiled = usize::try_from(spells).unwrap();
    assert_eq!(
        count_in(&driver, Zone::Exile, "Spell"),
        exiled,
        "each spell was exiled as it resolved"
    );
    assert_eq!(count_in(&driver, Zone::Graveyard(pid(0)), "Spell"), 0);
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_activation("Rod of Absorption").unwrap();
    if takes {
        settle(&mut driver, &first_card(x));
    } else {
        settle(&mut driver, &|decision| match &decision.kind {
            DecisionKind::ChooseCards(_) => Some(Answer::ChooseCards(CardsChoice(Vec::new()))),
            DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(x))),
            _ => None,
        });
    }
    assert_eq!(
        count_in(&driver, Zone::Graveyard(pid(0)), "Rod"),
        1,
        "the rod was sacrificed"
    );
    // The spells cast again resolve with no rod left to exile them, and each draws a card.
    assert_eq!(count_in(&driver, Zone::Graveyard(pid(0)), "Spell"), recast);
    assert_eq!(count_in(&driver, Zone::Exile, "Spell"), exiled - recast);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + recast);
    replays(&driver);
}

#[rstest]
fn rod_of_absorption_exiles_what_every_player_casts(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("Rod of Absorption").unwrap())])
        .battlefield(1, [ready(island())])
        .hand(1, [drawing("Spell 1", CardType::Sorcery, 1, &[])]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_cast("Spell 1").unwrap();
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && decision.player == pid(1)
        {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert_eq!(count_in(&driver, Zone::Exile, "Spell"), 1);
    assert_eq!(count_in(&driver, Zone::Graveyard(pid(1)), "Spell"), 0);
    replays(&driver);
}

/// Baron Helmut Zemo after attacking, in the second main phase, with `cards` black sorceries of `symbols` black
/// mana symbols each in the graveyard.
fn zemo(seats: usize, cards: usize, symbols: usize) -> Driver {
    let graveyard: Vec<_> = (1..=cards)
        .map(|n| {
            drawing(
                &format!("Dark {n}"),
                CardType::Sorcery,
                0,
                &vec![Color::Black; symbols],
            )
        })
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Baron Helmut Zemo").unwrap())])
        .graveyard(0, graveyard);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Baron Helmut Zemo", 1)]).unwrap();
    driver.pass_until(Step::Main2).unwrap();
    driver
}

#[rstest]
#[case::five_each_three_cards(3, 5, true, 3)]
#[case::four_cards_three_cast(4, 4, true, 3)]
#[case::two_cards_two_cast(2, 8, true, 2)]
#[case::fourteen_symbols(2, 7, false, 0)]
fn zemo_boast_casts_up_to_three_copies(
    #[values(2, 3, 4)] seats: usize,
    #[case] cards: usize,
    #[case] symbols: usize,
    #[case] offered: bool,
    #[case] cast: usize,
) {
    let mut driver = zemo(seats, cards, symbols);
    let abilities = driver.offered_abilities("Baron Helmut Zemo").unwrap();
    assert_eq!(!abilities.is_empty(), offered);
    if !offered {
        return;
    }
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_activation("Baron Helmut Zemo").unwrap();
    // The cost takes every card offered.
    settle(&mut driver, &|decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if options.min > 0 || options.max > 1 => {
            Some(Answer::ChooseCards(CardsChoice(options.from.clone())))
        }
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(CardsChoice(
            options.from.iter().copied().take(1).collect(),
        ))),
        _ => None,
    });
    assert_eq!(
        count_in(&driver, Zone::Exile, "Dark"),
        cards,
        "the cards stay exiled"
    );
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + cast,
        "each copy cast drew"
    );
    assert!(
        driver
            .offered_abilities("Baron Helmut Zemo")
            .unwrap()
            .is_empty(),
        "once each turn"
    );
    replays(&driver);
}

/// A planeswalker with "+1: You gain 2 life."
fn life_walker() -> Arc<CardDefinition> {
    let json = r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":2}}]}}"#;
    Arc::new(
        CardDefinition::new("Life Walker")
            .with_type(CardType::Planeswalker)
            .with_mana_cost(cost(3, &[]))
            .with_loyalty(3)
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

fn loyalty(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::Loyalty))
}

fn bolas() -> mtg_engine::PresetPermanent {
    with_counters(
        ready(card("Nicol Bolas, Dragon-God").unwrap()),
        CounterKind::Loyalty,
        4,
    )
}

#[rstest]
#[case::its_controllers_walker(0)]
#[case::an_opponents_walker(1)]
fn bolas_has_the_loyalty_abilities_of_other_planeswalkers(
    #[values(2, 3, 4)] seats: usize,
    #[case] owner: usize,
) {
    let walker = with_counters(ready(life_walker()), CounterKind::Loyalty, 3);
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    let scenario = if owner == 0 {
        scenario.battlefield(0, [bolas(), walker])
    } else {
        scenario
            .battlefield(0, [bolas()])
            .battlefield(owner, [walker])
    };
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let bolas_id = driver.id_of("Nicol Bolas, Dragon-God").unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("seat 0 has priority");
    };
    let granted: Vec<_> = options
        .abilities
        .iter()
        .filter(|ability| ability.object == bolas_id && ability.source != AbilitySource::Printed)
        .copied()
        .collect();
    assert_eq!(granted.len(), 1, "the walker's one loyalty ability");
    driver
        .answer(Answer::Priority(PriorityAction::Activate(granted[0])))
        .unwrap();
    settle(&mut driver, &|_| None);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 22);
    assert_eq!(
        loyalty(&driver, "Nicol Bolas, Dragon-God"),
        5,
        "its +1 is paid by Bolas"
    );
    assert_eq!(loyalty(&driver, "Life Walker"), 3);
    assert!(
        driver
            .offered_abilities("Nicol Bolas, Dragon-God")
            .unwrap()
            .is_empty(),
        "one loyalty ability of Bolas each turn"
    );
    replays(&driver);
}

#[rstest]
fn bolas_has_no_loyalty_abilities_of_his_own_kind_alone(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [bolas()]);
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let mut offered = driver.offered_abilities("Nicol Bolas, Dragon-God").unwrap();
    offered.sort_unstable();
    assert_eq!(
        offered,
        vec![1, 2],
        "his own +1 and −3; the −8 needs eight loyalty"
    );
}

#[rstest]
#[case::from_the_hand(true)]
#[case::a_permanent(false)]
fn bolas_plus_one_has_each_opponent_exile_a_card(
    #[values(2, 3, 4)] seats: usize,
    #[case] from_hand: bool,
) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [bolas()]);
    for seat in 1..seats {
        scenario = scenario
            .hand(seat, [bear()])
            .battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_ability("Nicol Bolas, Dragon-God", 1).unwrap();
    let choose = |driver_state: &mtg_engine::state::GameState, decision: &Decision| {
        let DecisionKind::ChooseCards(options) = &decision.kind else {
            return None;
        };
        let wanted = options.from.iter().copied().find(|id| {
            let in_hand = matches!(
                driver_state.object(*id).map(|o| o.zone()),
                Some(Zone::Hand(_))
            );
            in_hand == from_hand
        });
        Some(Answer::ChooseCards(CardsChoice(
            wanted.into_iter().collect(),
        )))
    };
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = choose(driver.state(), &decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && decision.player == pid(0)
        {
            break;
        }
        let answer = Driver::passive_answer(&decision);
        driver.answer(answer).unwrap();
    }
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 1,
        "you draw a card"
    );
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        seats - 1,
        "one card of each opponent"
    );
    for seat in 1..seats {
        let kept = usize::from(from_hand);
        assert_eq!(driver.state().zone(Zone::Hand(pid(seat))).len(), 1 - kept);
        assert_eq!(
            driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| driver
                    .state()
                    .object(**id)
                    .is_some_and(|o| o.controller() == pid(seat)))
                .count(),
            kept
        );
    }
    assert_eq!(loyalty(&driver, "Nicol Bolas, Dragon-God"), 5);
    replays(&driver);
}
