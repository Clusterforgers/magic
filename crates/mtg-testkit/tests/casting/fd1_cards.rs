//! Goal: the fd1 cards play whole, at two to four seats. Illusionary Mask offers the creature cards whose mana cost some or
//! all of the mana spent on X could pay, by amount and type, casts the chosen one face down as a 2/2 its controller alone
//! looks at, and that creature is turned face up before it becomes tapped, deals combat damage or is dealt damage, so it
//! fights as what it is. Magar of the Magic Strings puts an instant or sorcery card from its controller's graveyard onto the
//! battlefield face down as a 3/3 with the name noted; its combat damage to a player lets its controller cast a copy of the
//! card free, and it is exiled instead of going anywhere else. Each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, NumberChoice, TargetChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::Event;
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn_own_creature, island, keyworded_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
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
/// first legal targets are taken, `pick` answers what it knows and every yes-or-no is `yes`.
fn settle(driver: &mut Driver, yes: bool, pick: &dyn Fn(&Decision) -> Option<Answer>) {
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
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(yes)),
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


/// A creature card of this name and mana cost, a `power`/`power` with vigilance when `vigilant`.
fn beast(name: &str, cost: &str, power: i32, vigilant: bool) -> Arc<CardDefinition> {
    let keywords = if vigilant {
        vec![mtg_cards::keyword::Keyword::Vigilance]
    } else {
        Vec::new()
    };
    let made = keyworded_creature(name, 0, &[], power, power, &keywords);
    let mut made = (*made).clone();
    made.mana_cost = Some(ManaCost::parse(cost).unwrap());
    Arc::new(made)
}

/// Seat 0 in its first main phase with Illusionary Mask, `islands` Islands, a Mountain and `hand`.
fn masked_table(seats: usize, islands: usize, hand: Vec<Arc<CardDefinition>>) -> Driver {
    let mut board = vec![ready(card("Illusionary Mask").unwrap())];
    board.extend((0..islands).map(|_| ready(island())));
    board.push(ready(mountain()));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, hand);
    Driver::from_scenario(libraries(scenario, seats)).unwrap()
}

/// Activates the Mask for `x` paid with Islands, and casts the one creature card of the hand face down when it is
/// offered; whether it was.
fn mask(driver: &mut Driver, x: u32) -> bool {
    let offered = std::cell::Cell::new(false);
    driver.begin_activation("Illusionary Mask").unwrap();
    for _ in 0..10 {
        match &driver.decision().unwrap().kind {
            DecisionKind::ChooseNumber(_) => {
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                let islands = vec!["Island"; usize::try_from(x).unwrap()];
                driver.pay_tapping(&islands, 0).unwrap();
                break;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    settle(driver, true, &|decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            offered.set(offered.get() || !options.from.is_empty());
            Some(Answer::ChooseCards(CardsChoice(
                options.from.iter().copied().take(1).collect(),
            )))
        }
        _ => None,
    });
    offered.get()
}

fn face_down_on_battlefield(driver: &Driver) -> Option<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
}

#[rstest]
#[case::the_same_symbols("{U}{U}", true)]
#[case::one_generic("{1}{U}", true)]
#[case::all_generic("{2}", true)]
#[case::hybrid("{W/U}{W/U}", true)]
#[case::more_than_was_spent("{2}{U}", false)]
#[case::a_colour_not_spent("{G}", false)]
#[case::red_was_not_spent("{R}", false)]
fn illusionary_mask_casts_a_creature_the_mana_spent_on_x_could_pay_face_down(
    #[values(2, 3, 4)] seats: usize,
    #[case] cost: &str,
    #[case] cast: bool,
) {
    let mut driver = masked_table(seats, 2, vec![beast("Hidden Beast", cost, 4, false)]);
    let offered = mask(&mut driver, 2);
    assert_eq!(offered, cast, "{cost}");
    let down = face_down_on_battlefield(&driver);
    assert_eq!(down.is_some(), cast);
    if let Some(id) = down {
        let object = driver.state().object(id).unwrap();
        assert_eq!(object.face_down(), Some(FaceDown::R6008Masked));
        assert_eq!(object.card().name, "");
        assert_eq!(object.whole_card().name, "Hidden Beast");
        assert!(driver.state().may_look_at(pid(0), id));
        for seat in 1..seats {
            assert!(!driver.state().may_look_at(pid(seat), id), "seat {seat}");
        }
        let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
        assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    } else {
        assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 1);
    }
    replays(&driver);
}

fn turned_up(driver: &Driver, id: ObjectId) -> bool {
    driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::TurnedFaceUp { object, .. } if *object == id))
}

#[rstest]
#[case::tapped_to_attack(false)]
#[case::assigning_combat_damage(true)]
fn a_masked_creature_attacks_as_what_it_is(
    #[values(2, 3, 4)] seats: usize,
    #[case] vigilant: bool,
) {
    let mut driver = masked_table(seats, 4, vec![beast("Hidden Beast", "{2}{U}{U}", 5, vigilant)]);
    assert!(mask(&mut driver, 4));
    let id = face_down_on_battlefield(&driver).unwrap();
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::DeclareAttackers).unwrap();
    driver
        .answer(Answer::DeclareAttackers(vec![(id, AttackTarget::Player(pid(1)))]))
        .unwrap();
    let object = driver.state().object(id).unwrap();
    // Tapping to attack turns it face up; with vigilance it attacks face down until it would assign its damage.
    assert_eq!(object.face_down().is_none(), !vigilant);
    driver.idle_until(turn, Step::Main2).unwrap();
    assert!(driver.state().object(id).unwrap().face_down().is_none());
    assert!(turned_up(&driver, id));
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 15, "it dealt 5");
    replays(&driver);
}

#[rstest]
fn a_masked_creature_is_turned_face_up_before_it_is_dealt_damage(#[values(2, 3, 4)] seats: usize) {
    let mut driver = masked_table(
        seats,
        4,
        vec![
            beast("Hidden Beast", "{2}{U}{U}", 5, false),
            burn_own_creature("Ping"),
        ],
    );
    assert!(mask(&mut driver, 4));
    let id = face_down_on_battlefield(&driver).unwrap();
    assert!(!turned_up(&driver, id));
    driver.begin_cast("Ping").unwrap();
    settle(&mut driver, true, &|_| None);
    let object = driver.state().object(id).unwrap();
    assert!(object.face_down().is_none());
    assert_eq!(object.zone(), Zone::Battlefield, "a 5/5 survives 1 damage");
    assert_eq!(object.permanent().unwrap().damage(), 1);
    replays(&driver);
}

/// A sorcery for {0} with "Draw a card."
fn study() -> Arc<CardDefinition> {
    let json = r#"{"Spell":{"effects":[{"Draw":{}}]}}"#;
    Arc::new(
        CardDefinition::new("Quick Study")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[Color::Blue]))
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

/// Seat 0 has activated Magar of the Magic Strings on Quick Study in its graveyard.
fn magar(seats: usize) -> (Driver, ObjectId) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Magar of the Magic Strings").unwrap()),
                ready(swamp()),
                ready(swamp()),
                ready(swamp()),
                ready(swamp()),
                ready(mountain()),
                ready(island()),
            ],
        )
        .graveyard(0, [study()])
        .hand(0, [card("Murder").unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_activation("Magar of the Magic Strings").unwrap();
    settle(&mut driver, true, &|_| None);
    let id = face_down_on_battlefield(&driver).unwrap();
    (driver, id)
}

#[rstest]
fn magar_puts_the_card_down_as_a_3_3_with_its_name_noted(#[values(2, 3, 4)] seats: usize) {
    let (driver, id) = magar(seats);
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.face_down(), Some(FaceDown::R6008Listed));
    assert_eq!(object.whole_card().name, "Quick Study");
    assert_eq!(object.card().name, "");
    assert_eq!(object.r6008_listed().unwrap().noted(), Some("Quick Study"));
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert!(chars.types.contains(CardType::Creature));
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    assert!(driver.state().may_look_at(pid(0), id));
    for seat in 1..seats {
        assert!(!driver.state().may_look_at(pid(seat), id), "seat {seat}");
    }
    replays(&driver);
}

#[rstest]
fn magar_s_creature_casts_a_copy_of_the_noted_card_when_it_deals_combat_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] takes: bool,
) {
    let (mut driver, id) = magar(seats);
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::DeclareAttackers).unwrap();
    driver
        .answer(Answer::DeclareAttackers(vec![(id, AttackTarget::Player(pid(1)))]))
        .unwrap();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.pass_until(Step::CombatDamage).unwrap();
    settle(&mut driver, takes, &|_| None);
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.face_down(), Some(FaceDown::R6008Listed), "it stays face down");
    let drawn = usize::from(takes);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + drawn);
    // The copy ceased to exist once it resolved; the card itself stays on the battlefield.
    assert_eq!(object.zone(), Zone::Battlefield);
    replays(&driver);
}

#[rstest]
fn magar_s_creature_is_exiled_instead_of_dying(#[values(2, 3, 4)] seats: usize) {
    let (mut driver, id) = magar(seats);
    let card = driver.state().object(id).unwrap().whole_card().name.clone();
    driver.begin_cast("Murder").unwrap();
    settle(&mut driver, true, &|decision| match &decision.kind {
        DecisionKind::ChooseTargets(options) if options.legal.contains(&mtg_engine::choose::Target::Object(id)) => {
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                mtg_engine::choose::Target::Object(id),
            ])))
        }
        _ => None,
    });
    let exiled = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == card);
    assert!(exiled, "the card was exiled, not put into the graveyard");
    assert!(
        !driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == card)
    );
    replays(&driver);
}
