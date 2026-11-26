//! Goal: the p159 cards play whole, at two to four seats: Cryptic Spires asks its owner for two colors per copy before
//! any hand, its mana is one of them and a copy of it reads them; Enduring Angel turns a life total that would reach 0
//! into its transformation and 3 life, also for several hits at once, and a copy that cannot transform loses the game;
//! Professor Hojo takes {2} from the first activated ability on its controller's turn that targets one of their
//! creatures and draws once a turn when they are targeted; Yavimaya Bloomsage prepares at 7 power and Channel gives a
//! mana ability paid in life until end of turn; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::choose::{OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{
    Answer, Decision, DecisionKind, MulliganChoice, PayChoice, PriorityAction,
};
use mtg_engine::format::Normal;
use mtg_engine::setup::GameSetup;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, replay};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const SPIRES: &str = "Cryptic Spires";
const ANGEL: &str = "Enduring Angel // Angelic Enforcer";

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    })
}

/// Answers every question until the stack is empty and seat 0 has priority again: mana is paid automatically, the
/// first legal targets are taken, `pick` answers first and every yes-or-no is yes.
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

fn nothing(_: &Decision) -> Option<Answer> {
    None
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn pool(driver: &Driver, seat: usize, mana: ManaType) -> u32 {
    driver.state().player(pid(seat)).unwrap().mana().get(mana)
}

fn mana_of(color: Color) -> ManaType {
    match color {
        Color::White => ManaType::White,
        Color::Blue => ManaType::Blue,
        Color::Black => ManaType::Black,
        Color::Red => ManaType::Red,
        Color::Green => ManaType::Green,
    }
}

/// The driver of `driver`'s game with the colors circled for `object` as though chosen before the game.
fn circled(driver: &Driver, object: ObjectId, colors: &[Color]) -> Driver {
    let mut events = driver.engine().events().to_vec();
    events.push(Event::ColorsCircled {
        player: pid(0),
        object,
        colors: colors.to_vec(),
    });
    Driver::new(Engine::resume(driver.engine().setup().clone(), events).unwrap()).unwrap()
}

#[rstest]
fn cryptic_spires_asks_for_two_colors_of_each_copy_before_any_hand(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 4, 9)] pick: usize,
) {
    let deck = |spires: usize| -> Vec<Arc<CardDefinition>> {
        (0..40)
            .map(|at| {
                if at < spires {
                    card(SPIRES).unwrap()
                } else {
                    forest()
                }
            })
            .collect()
    };
    let decks = (0..seats)
        .map(|seat| deck(if seat == 0 { 3 } else { 0 }))
        .collect();
    let mut setup = GameSetup::new(Arc::new(Normal), 7, decks);
    setup.validate_decks = false;
    let mut driver = Driver::new(Engine::new(setup).unwrap()).unwrap();
    let mut asked = Vec::new();
    while let DecisionKind::ChooseOption(options) = driver.decision().unwrap().kind.clone() {
        let player = driver.decision().unwrap().player;
        assert_eq!(player, pid(0), "only the owner of the copies is asked");
        assert!(
            driver.state().zone(Zone::Hand(player)).is_empty(),
            "no hand before the colors"
        );
        assert_eq!(options.options.len(), 10, "every pair of colors");
        assert!(!asked.contains(&options.source), "each copy once");
        asked.push(options.source);
        let events = driver
            .answer(Answer::ChooseOption(OptionChoice(pick)))
            .unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            Event::ColorsCircled { object, colors, .. } if *object == options.source && colors.len() == 2
        )));
    }
    assert_eq!(asked.len(), 3, "one question per copy");
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Mulligan(_)
    ));
    while matches!(driver.decision().unwrap().kind, DecisionKind::Mulligan(_)) {
        driver
            .answer(Answer::Mulligan(MulliganChoice::Keep))
            .unwrap();
    }
    replays(&driver);
}

#[rstest]
fn cryptic_spires_adds_a_circled_color_and_a_copy_reads_the_colors_of_what_it_copies(
    #[values(2, 3, 4)] seats: usize,
    #[values((Color::White, Color::Blue), (Color::Black, Color::Green))] colors: (Color, Color),
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(card(SPIRES).unwrap()), ready(card(SPIRES).unwrap())],
        )
        .hand(0, [card("Vesuva").unwrap()]);
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let spires: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == SPIRES)
        .collect();
    let mut driver = circled(&driver, spires[0], &[colors.0, colors.1]);
    let ways: Vec<ManaType> = mtg_engine::mana_ways(driver.state(), spires[0])
        .iter()
        .flat_map(|(_, options)| options.iter().map(|option| option.produces))
        .collect();
    assert_eq!(
        ways,
        vec![mana_of(colors.0), mana_of(colors.1)],
        "the circled colors"
    );
    assert!(
        mtg_engine::mana_ways(driver.state(), spires[1])
            .iter()
            .all(|(_, o)| o.is_empty())
    );
    assert!(
        !driver
            .state()
            .object(spires[0])
            .unwrap()
            .permanent()
            .unwrap()
            .tapped(),
        "untapped"
    );
    // The copy with no colors circled adds nothing.
    let options = match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.clone(),
        other => panic!("priority, not {other:?}"),
    };
    let offered: Vec<ObjectId> = options
        .mana_abilities
        .iter()
        .map(|m| m.source.object)
        .collect();
    assert!(offered.contains(&spires[0]));
    assert!(
        !offered.contains(&spires[1]),
        "an uncircled Spires adds no mana"
    );
    driver.tap_for_mana_of(SPIRES, mana_of(colors.1)).unwrap();
    assert_eq!(pool(&driver, 0, mana_of(colors.1)), 1);
    // Vesuva enters as a copy of the circled Spires and adds its colors.
    let vesuva = *driver.state().zone(Zone::Hand(pid(0))).first().unwrap();
    driver.play_land("Vesuva").unwrap();
    settle(&mut driver, &|decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if options.from.contains(&spires[0]) => {
            Some(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                spires[0],
            ])))
        }
        DecisionKind::ChooseTargets(options)
            if options
                .legal
                .iter()
                .any(|t| t.object().is_some_and(|o| o.id == spires[0])) =>
        {
            Some(Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .filter(|t| t.object().is_some_and(|o| o.id == spires[0]))
                    .collect(),
            )))
        }
        _ => None,
    });
    let kinds = mtg_engine::mana_ways(driver.state(), vesuva);
    let made: Vec<ManaType> = kinds
        .iter()
        .flat_map(|(_, options)| options.iter().map(|option| option.produces))
        .collect();
    assert!(
        made.contains(&mana_of(colors.0)) && made.contains(&mana_of(colors.1)),
        "{made:?}"
    );
    replays(&driver);
}

/// Targets the player of `seat` when a question of targets offers them.
fn at_seat(seat: usize) -> impl Fn(&Decision) -> Option<Answer> {
    move |decision| match &decision.kind {
        DecisionKind::ChooseTargets(options)
            if options
                .legal
                .contains(&mtg_engine::choose::Target::Player(pid(seat))) =>
        {
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                mtg_engine::choose::Target::Player(pid(seat)),
            ])))
        }
        _ => None,
    }
}

fn transformed(driver: &Driver, id: ObjectId) -> bool {
    driver.state().object(id).unwrap().face() == Some(1)
}

#[rstest]
fn enduring_angel_transforms_in_place_of_a_life_total_reaching_zero(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Lightning Bolt", 2), ("Lightning Bolt", 3), ("Sign in Blood", 1), ("Lightning Bolt", 4))]
    spell: (&str, i32),
) {
    let (name, before) = spell;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .life(0, before)
        .battlefield(
            0,
            [
                ready(card(ANGEL).unwrap()),
                ready(mtg_testkit::cards::mountain()),
                ready(mtg_testkit::cards::swamp()),
                ready(mtg_testkit::cards::swamp()),
            ],
        )
        .hand(0, [card(name).unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let angel = driver.id_of("Enduring Angel").unwrap();
    driver.begin_cast(name).unwrap();
    settle(&mut driver, &at_seat(0));
    let taken = if name == "Lightning Bolt" { 3 } else { 2 };
    if before > taken {
        assert_eq!(
            life(&driver, 0),
            before - taken,
            "a total above 0 is not replaced"
        );
        assert!(!transformed(&driver, angel));
    } else {
        assert_eq!(life(&driver, 0), 3, "the life total becomes 3");
        assert!(transformed(&driver, angel), "the Angel transformed");
        assert!(driver.state().is_alive(pid(0)));
    }
    replays(&driver);
}

#[rstest]
fn enduring_angel_replaces_combat_damage_from_several_attackers_at_once(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .life(0, 5)
        .battlefield(0, [ready(card(ANGEL).unwrap())])
        .battlefield(
            1,
            [
                ready(mtg_testkit::cards::free_creature("Ogre", 3, 3)),
                ready(mtg_testkit::cards::free_creature("Brute", 3, 3)),
            ],
        );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let angel = driver.id_of("Enduring Angel").unwrap();
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver.attack(&[("Ogre", 0), ("Brute", 0)]).unwrap();
    driver.idle_until(1, Step::End).unwrap();
    assert_eq!(life(&driver, 0), 3, "six damage at once leaves 3 life");
    assert!(transformed(&driver, angel));
    replays(&driver);
}

#[rstest]
fn a_copy_of_enduring_angel_that_cannot_transform_loses_the_game(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .life(0, 2)
        .battlefield(1, [ready(card(ANGEL).unwrap())])
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(island()),
                ready(island()),
                ready(mtg_testkit::cards::mountain()),
            ],
        )
        .hand(0, [card("Clone").unwrap(), card("Lightning Bolt").unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_cast("Clone").unwrap();
    settle(&mut driver, &nothing);
    driver.begin_cast("Lightning Bolt").unwrap();
    for _ in 0..100 {
        if !driver.state().is_alive(pid(0)) || driver.decision().is_err() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = at_seat(0)(&decision).unwrap_or_else(|| match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        });
        driver.answer(answer).unwrap();
    }
    assert!(
        !driver.state().is_alive(pid(0)),
        "the Clone cannot transform"
    );
    replays(&driver);
}

/// An artifact with "{2}: Target creature gets +1/+1 until end of turn.", usable at instant speed.
fn pump_rod() -> Arc<CardDefinition> {
    let json = r#"{"Activated":{"cost":"{2}","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#;
    Arc::new(
        CardDefinition::new("Pump Rod")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn untapped_forests(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == "Forest")
        .filter(|object| object.permanent().is_some_and(|p| !p.tapped()))
        .count()
}

/// Activates the Pump Rod targeting the creature named `at` and settles the stack.
fn pump(driver: &mut Driver, at: &str) {
    let target = driver.id_of(at).unwrap();
    driver.begin_activation("Pump Rod").unwrap();
    settle(driver, &move |decision| match &decision.kind {
        DecisionKind::ChooseTargets(options) => Some(Answer::ChooseTargets(TargetChoice::Targets(
            options
                .legal
                .iter()
                .copied()
                .filter(|t| t.object().is_some_and(|o| o.id == target))
                .collect(),
        ))),
        _ => None,
    });
}

#[rstest]
fn professor_hojo_discounts_the_first_ability_targeting_your_creature_and_draws_once(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] foe_first: bool,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Professor Hojo").unwrap()),
                ready(pump_rod()),
                ready(mtg_testkit::cards::free_creature("Bear", 2, 2)),
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(1, [ready(mtg_testkit::cards::free_creature("Wolf", 2, 2))]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let hand = hand_size(&driver, 0);
    if foe_first {
        pump(&mut driver, "Wolf");
        assert_eq!(
            untapped_forests(&driver),
            2,
            "an opponent's creature is no discount"
        );
        assert_eq!(hand_size(&driver, 0), hand, "nor a draw");
    }
    let before = untapped_forests(&driver);
    pump(&mut driver, "Bear");
    assert_eq!(
        untapped_forests(&driver),
        before,
        "the first costs {{2}} less"
    );
    assert_eq!(
        hand_size(&driver, 0),
        hand + 1,
        "a target of yours draws a card"
    );
    pump(&mut driver, "Bear");
    assert_eq!(
        untapped_forests(&driver),
        before - 2,
        "the second costs in full"
    );
    assert_eq!(hand_size(&driver, 0), hand + 1, "it draws once a turn");
    replays(&driver);
}

#[rstest]
fn professor_hojo_gives_no_discount_on_another_players_turn(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats).at(1, 1, Step::Main1).battlefield(
        0,
        [
            ready(card("Professor Hojo").unwrap()),
            ready(pump_rod()),
            ready(forest()),
            ready(forest()),
        ],
    );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    // Seat 1 passes priority to seat 0, who activates on seat 1's turn.
    while driver.decision().unwrap().player != pid(0) {
        driver.pass().unwrap();
    }
    let hand = hand_size(&driver, 0);
    let hojo = driver.id_of("Professor Hojo").unwrap();
    driver.begin_activation("Pump Rod").unwrap();
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .filter(|t| t.object().is_some_and(|o| o.id == hojo))
                    .collect(),
            )),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert_eq!(
        untapped_forests(&driver),
        0,
        "it costs {{2}} on another turn"
    );
    assert_eq!(
        hand_size(&driver, 0),
        hand + 1,
        "the draw is not limited to its controller's turn"
    );
    replays(&driver);
}

const BLOOMSAGE: &str = "Yavimaya Bloomsage // Channel";

fn prepared(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::prepared)
}

#[rstest]
fn yavimaya_bloomsage_prepares_when_its_target_reaches_seven_power(
    #[values(2, 3, 4)] seats: usize,
    #[values((6, true), (5, false))] grown: (i32, bool),
) {
    let (power, prepares) = grown;
    let scenario = Scenario::new(seats).at(1, 0, Step::Main2).battlefield(
        0,
        [
            ready(card(BLOOMSAGE).unwrap()),
            ready(mtg_testkit::cards::free_creature("Giant", power, power)),
        ],
    );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let giant = driver.id_of("Giant").unwrap();
    driver.pass().unwrap();
    for _ in 0..200 {
        if driver.state().turn().step() == Step::End
            && driver.state().stack().is_empty()
            && matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
            && driver
                .state()
                .object(giant)
                .unwrap()
                .permanent()
                .unwrap()
                .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne)
                > 0
        {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .filter(|t| t.object().is_some_and(|o| o.id == giant))
                    .collect(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert_eq!(
        driver
            .state()
            .object(giant)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(prepared(&driver, "Yavimaya Bloomsage"), prepares);
    replays(&driver);
}

#[rstest]
fn channel_gives_a_mana_ability_paid_in_life_until_end_of_turn(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card(BLOOMSAGE).unwrap()),
            ready(forest()),
            ready(forest()),
        ],
    );
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let bloomsage = driver.id_of("Yavimaya Bloomsage").unwrap();
    let mut events = driver.engine().events().to_vec();
    events.push(Event::Prepared { object: bloomsage });
    let mut driver =
        Driver::new(Engine::resume(driver.engine().setup().clone(), events).unwrap()).unwrap();
    driver.cast_prepared("Yavimaya Bloomsage").unwrap();
    settle(&mut driver, &nothing);
    let emblem = *driver.state().zone(Zone::Command(pid(0))).last().unwrap();
    let before = life(&driver, 0);
    let options = match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.clone(),
        other => panic!("priority, not {other:?}"),
    };
    let channel = options
        .mana_abilities
        .iter()
        .find(|m| m.source.object == emblem)
        .expect("the emblem's mana ability is offered")
        .clone();
    driver
        .answer(Answer::Priority(PriorityAction::ActivateMana(
            channel.source,
        )))
        .unwrap();
    assert_eq!(life(&driver, 0), before - 1, "1 life is paid");
    assert_eq!(pool(&driver, 0, ManaType::Colorless), 1, "for {{C}}");
    driver.pass_until(Step::Cleanup).ok();
    driver.idle_until(2, Step::Upkeep).ok();
    assert!(
        driver
            .state()
            .object(emblem)
            .is_none_or(|object| object.zone() != Zone::Command(pid(0))),
        "the ability ends with the turn"
    );
    replays(&driver);
}
