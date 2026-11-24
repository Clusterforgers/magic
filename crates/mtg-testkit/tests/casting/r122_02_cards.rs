//! Goal: the r122-02 cards play whole, at two and four seats: Eluge's flood counters take {U} (or else {1}) off the first
//! instant or sorcery spell of the turn only; The Face of Boe casts a spell with suspend from the hand for its suspend
//! cost; Esper Origins cast from a graveyard returns as its Saga with a finality counter, and from the hand goes to the
//! graveyard; The Tale of Tamiyo mills two and draws while the pair shares a card type; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, with_counters};
use rstest::rstest;

/// Answers what is asked until seat 0 holds priority with an empty stack: cards named in `pick` are chosen (else the
/// fewest allowed), every yes-or-no is yes, targets go to seat 1 or the first allowed, mana is paid automatically. The
/// costs asked are returned in order.
fn settle(driver: &mut Driver, pick: &str) -> Vec<String> {
    let mut costs = Vec::new();
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if decision.player == PlayerId(0) && driver.state().stack().is_empty() =>
            {
                return costs;
            }
            DecisionKind::Priority(_) => {
                Answer::Priority(mtg_engine::decision::PriorityAction::Pass)
            }
            DecisionKind::ChooseCards(options) => {
                let named: Vec<ObjectId> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| driver.state().object(*id).unwrap().card().name == pick)
                    .take(1)
                    .collect();
                let fewest = usize::try_from(options.min).unwrap();
                let chosen = if named.is_empty() {
                    options.from.iter().copied().take(fewest).collect()
                } else {
                    named
                };
                Answer::ChooseCards(CardsChoice(chosen))
            }
            DecisionKind::YesNo(options) => Answer::YesNo(YesNoChoice(!options.only_no)),
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_player(1);
                Answer::ChooseTargets(TargetChoice::Targets(vec![target]))
            }
            DecisionKind::PayMana(options) => {
                costs.push(options.cost.to_string());
                Answer::PayMana(PayChoice::Auto)
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("never settled");
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

/// The cost asked for the first and the second Rift Bolt of the turn with `flooded` Mountains holding a flood counter
/// beside Eluge: each counter takes {U}, or {1} when the cost has no {U} left, from the first spell only.
#[rstest]
#[case::one_counter(1, "{1}{R}")]
#[case::two_counters(2, "{R}")]
#[case::more_counters_than_generic(3, "{R}")]
fn eluge_reduces_the_first_instant_or_sorcery(
    #[values(2, 4)] seats: usize,
    #[case] flooded: usize,
    #[case] first: &str,
) {
    let flood = CounterKind::Other("flood".into());
    // An Island keeps Eluge (its power and toughness count Islands) on the battlefield.
    let mut mine = vec![
        ready(card("Eluge, the Shoreless Sea").unwrap()),
        ready(island()),
    ];
    mine.extend((0..flooded).map(|_| with_counters(ready(mountain()), flood.clone(), 1)));
    mine.extend((0..3).map(|_| ready(mountain())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .hand_named(0, ["Rift Bolt", "Rift Bolt"]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    settle(&mut driver, "");
    driver.begin_cast("Rift Bolt").unwrap();
    let costs = settle(&mut driver, "");
    assert_eq!(costs, vec![first.to_owned()]);
    driver.begin_cast("Rift Bolt").unwrap();
    let costs = settle(&mut driver, "");
    assert_eq!(
        costs,
        vec!["{2}{R}".to_owned()],
        "the second spell pays in full"
    );
    replays(&driver);
}

#[rstest]
fn eluge_floods_a_land_as_it_attacks(#[values(2, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::BeginCombat)
        .battlefield(
            0,
            [
                ready(card("Eluge, the Shoreless Sea").unwrap()),
                ready(island()),
                ready(forest()),
            ],
        );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    for _ in 0..50 {
        let decision = driver.decision().unwrap().clone();
        if decision.kind.name() == "declare-attackers" {
            break;
        }
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    driver.attack(&[("Eluge, the Shoreless Sea", 1)]).unwrap();
    let land = driver.target_named("Forest").unwrap();
    driver.choose_targets(&[land]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let forest = driver.id_of("Forest").unwrap();
    let object = driver.state().object(forest).unwrap();
    assert_eq!(
        object
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::Other("flood".into())),
        1
    );
    let types = mtg_engine::characteristics::characteristics(driver.state(), forest).unwrap();
    assert!(types.subtypes.iter().any(|s| s.0 == "Island"));
    // Eluge counts the flooded Forest among its Islands.
    let eluge = driver.id_of("Eluge, the Shoreless Sea").unwrap();
    let eluge = mtg_engine::characteristics::characteristics(driver.state(), eluge).unwrap();
    assert_eq!(eluge.power, Some(2));
    replays(&driver);
}

#[rstest]
fn the_face_of_boe_casts_for_the_suspend_cost(#[values(2, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("The Face of Boe").unwrap()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand_named(0, ["Rift Bolt"]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    settle(&mut driver, "");
    driver.begin_activation("The Face of Boe").unwrap();
    let costs = settle(&mut driver, "Rift Bolt");
    assert_eq!(
        costs,
        vec!["{R}".to_owned()],
        "the suspend cost, not {{2}}{{R}}"
    );
    driver.expect_life(1, 17).unwrap();
    replays(&driver);
}

fn origins(seats: usize, from_graveyard: bool) -> Driver {
    let mine: Vec<_> = (0..6).map(|_| ready(forest())).collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    let scenario = if from_graveyard {
        scenario.graveyard_named(0, ["Esper Origins // Summon: Esper Maduin"])
    } else {
        scenario.hand_named(0, ["Esper Origins // Summon: Esper Maduin"])
    };
    let bears = vec![creature("Bear", 1, &[], 2, 2); 6];
    Driver::from_scenario(libraries(scenario, seats).library(0, bears)).unwrap()
}

#[rstest]
fn esper_origins_from_a_graveyard_returns_as_its_saga(#[values(2, 4)] seats: usize) {
    let mut driver = origins(seats, true);
    settle(&mut driver, "");
    driver.begin_cast("Esper Origins").unwrap();
    settle(&mut driver, "");
    driver.expect_life(0, 22).unwrap();
    let saga = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == "Summon: Esper Maduin")
        .expect("the Saga is on the battlefield");
    let permanent = driver.state().object(saga).unwrap().permanent().unwrap();
    assert_eq!(
        permanent.counters_of(&CounterKind::Other("finality".into())),
        1
    );
    assert_eq!(permanent.counters_of(&CounterKind::lore()), 1);
    // Chapter I put the revealed Bear, a permanent card, into the hand.
    assert!(
        driver
            .state()
            .zone(Zone::Hand(PlayerId(0)))
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Bear")
    );
    replays(&driver);
}

#[rstest]
fn esper_origins_from_the_hand_stays_a_sorcery(#[values(2, 4)] seats: usize) {
    let mut driver = origins(seats, false);
    settle(&mut driver, "");
    driver.begin_cast("Esper Origins").unwrap();
    settle(&mut driver, "");
    driver.expect_life(0, 22).unwrap();
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(PlayerId(0)))
            .iter()
            .any(|id| driver
                .state()
                .object(*id)
                .unwrap()
                .card()
                .name
                .starts_with("Esper Origins"))
    );
    assert!(
        !driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Summon: Esper Maduin")
    );
    replays(&driver);
}

/// Chapter I mills a land pair (shares a type: draw, repeat), then a creature and an instant (stops).
#[rstest]
fn the_tale_of_tamiyo_repeats_while_the_pair_shares_a_type(#[values(2, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..3).map(|_| ready(island())).collect::<Vec<_>>())
        .hand_named(0, ["The Tale of Tamiyo"]);
    let library = vec![
        mountain(),
        forest(),
        card("Opt").unwrap(),
        creature("Bear", 1, &[], 2, 2),
        card("Lightning Bolt").unwrap(),
        island(),
        island(),
    ];
    let mut driver = Driver::from_scenario(libraries(scenario, seats).library(0, library)).unwrap();
    settle(&mut driver, "");
    driver.begin_cast("The Tale of Tamiyo").unwrap();
    settle(&mut driver, "");
    let names = |zone: Zone| -> Vec<String> {
        driver
            .state()
            .zone(zone)
            .iter()
            .map(|id| driver.state().object(*id).unwrap().card().name.clone())
            .collect()
    };
    let graveyard = names(Zone::Graveyard(PlayerId(0)));
    assert_eq!(graveyard.len(), 4, "{graveyard:?}");
    assert!(names(Zone::Hand(PlayerId(0))).contains(&"Opt".to_owned()));
    assert_eq!(names(Zone::Library(PlayerId(0))).len(), 2);
    replays(&driver);
}

/// Bloodbraid Elf cascades past a Forest and a Mountain into Lightning Bolt: with Averna its controller puts the Forest
/// onto the battlefield tapped before the cast; without Averna nothing is asked and both lands go to the bottom.
#[rstest]
fn averna_puts_a_cascaded_land_onto_the_battlefield(
    #[values(2, 4)] seats: usize,
    #[values(true, false)] averna: bool,
) {
    let mut mine: Vec<_> = (0..2).map(|_| ready(mountain())).collect();
    mine.extend((0..2).map(|_| ready(forest())));
    if averna {
        mine.push(ready(card("Averna, the Chaos Bloom").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .hand_named(0, ["Bloodbraid Elf"]);
    let library = vec![
        island(),
        mountain(),
        card("Lightning Bolt").unwrap(),
        island(),
        island(),
    ];
    let mut driver = Driver::from_scenario(libraries(scenario, seats).library(0, library)).unwrap();
    settle(&mut driver, "");
    driver.begin_cast("Bloodbraid Elf").unwrap();
    settle(&mut driver, "Island");
    let islands: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == "Island")
        .collect();
    assert_eq!(islands.len(), usize::from(averna));
    if let Some(island) = islands.first() {
        assert!(
            driver
                .state()
                .object(*island)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        );
    }
    driver.expect_life(1, 17).unwrap();
    replays(&driver);
}
