//! Goal: the p170 cards play whole at two to four seats: Scotty, Miracle Worker gives the instant and sorcery cards in
//! his controller's graveyard beam me up for their mana cost during that player's main phase only, never returns Scotty
//! himself for that given cost while a card's own beam me up may, and leaves creature cards alone; Prowl, Stoic
//! Strategist exiles a tapped creature as it attacks, its owner may play it, and that play draws a card and converts
//! Prowl, whose back face converts back on the second creature entering in a turn; Myra the Magnificent opens an
//! Attraction on an instant cast from hand and marks an Attraction with a midway counter so that each visit of it casts
//! a copy of the exiled card; each game replays and each card is Verified with the engine's capabilities.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::attraction::Lights;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, NumberChoice, TargetChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, island, plains, un33_attraction, un33_opener};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::{name_of, names, pid};

fn p170_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 controls Scotty, `others` and three Islands and two Plains, with `graveyard` in its graveyard, at `step` of
/// turn 2 with `active` the active seat.
fn p170_scotty(
    seats: usize,
    graveyard: &[&str],
    others: usize,
    active: usize,
    step: Step,
) -> Result<Driver, ScriptError> {
    let battlefield: Vec<PresetPermanent> =
        std::iter::once(ready(card("Scotty, Miracle Worker").unwrap()))
            .chain((0..others).map(|n| ready(free_creature(&format!("Ensign {n}"), 1, 1))))
            .chain(lands(3, island))
            .chain(lands(2, plains))
            .collect();
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| island()))
        })
        .at(2, active, step)
        .graveyard_named(0, graveyard.to_vec())
        .battlefield(0, battlefield);
    Driver::from_scenario(scenario)
}

/// Whether the player deciding may cast the card called `name` now.
fn p170_castable(driver: &Driver, name: &str) -> bool {
    match &driver.decision().map(|decision| decision.kind.clone()) {
        Ok(DecisionKind::Priority(options)) => options
            .castable
            .iter()
            .any(|id| name_of(driver.state(), *id) == name),
        _ => false,
    }
}

#[rstest]
fn scotty_beams_up_an_instant_for_its_mana_cost_returning_another_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(Step::Main1, Step::Main2)] step: Step,
) -> Result<(), ScriptError> {
    let mut driver = p170_scotty(seats, &["Opt"], 1, 0, step)?;
    assert!(p170_castable(&driver, "Opt"));
    let ensign = driver.id_of("Ensign 0").unwrap();
    driver.begin_cast("Opt")?;
    for _ in 0..16 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(_) => choose_way(&mut driver, "Beam me up")?,
            DecisionKind::ChooseCards(options) => {
                assert!(
                    !options
                        .from
                        .contains(&driver.id_of("Scotty, Miracle Worker").unwrap()),
                    "Scotty can't be beamed up for the cost he gives"
                );
                driver.answer(Answer::ChooseCards(CardsChoice(vec![ensign])))?;
            }
            DecisionKind::PayMana(_) => {
                pay(&mut driver)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let state = driver.state();
    assert_eq!(names(state, Zone::Exile), ["Opt"], "the spell is exiled");
    assert!(names(state, Zone::Hand(pid(0))).contains(&"Ensign 0".to_owned()));
    let ways: Vec<Option<CastWay>> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .collect();
    assert_eq!(ways, [Some(CastWay::R116BeamMeUp)]);
    p170_replays(&driver);
    Ok(())
}

#[rstest]
#[case::only_scotty_to_return(&["Opt"], 0, 0, Step::Main1, "Opt", false)]
#[case::another_creature(&["Opt"], 1, 0, Step::Main1, "Opt", true)]
#[case::not_a_main_phase(&["Opt"], 1, 0, Step::BeginCombat, "Opt", false)]
#[case::in_the_upkeep(&["Opt"], 1, 0, Step::Upkeep, "Opt", false)]
#[case::a_creature_card(&["Grizzly Bears"], 1, 0, Step::Main1, "Grizzly Bears", false)]
#[case::its_own_beam_returns_scotty(&["Pure Energy"], 0, 0, Step::Main1, "Pure Energy", true)]
fn scotty_gives_beam_me_up_only_when_its_cost_can_be_paid(
    #[values(2, 4)] seats: usize,
    #[case] graveyard: &[&str],
    #[case] others: usize,
    #[case] active: usize,
    #[case] step: Step,
    #[case] name: &str,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let driver = p170_scotty(seats, graveyard, others, active, step)?;
    assert_eq!(p170_castable(&driver, name), castable);
    Ok(())
}

/// Answers `pick` gives first, then mana automatically, the first legal targets, yes to every question and passes,
/// `steps` times at most or until `stop` holds.
fn p170_run(
    driver: &mut Driver,
    steps: usize,
    pick: &dyn Fn(&Driver, &Decision) -> Option<Answer>,
    stop: &dyn Fn(&Driver) -> bool,
) {
    for _ in 0..steps {
        if stop(driver) || driver.state().result().is_some() {
            return;
        }
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(driver, &decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min.max(1) as usize)
                    .collect(),
            )),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
}

fn p170_none(_: &Driver, _: &Decision) -> Option<Answer> {
    None
}

/// Whether the turn is `turn` at `step` with an empty stack and a player to act.
fn p170_settled(turn: u32, step: Step) -> impl Fn(&Driver) -> bool {
    move |driver: &Driver| {
        let now = driver.state().turn();
        now.number() == turn
            && now.step() == step
            && driver.state().stack().is_empty()
            && matches!(
                driver.decision().map(|d| &d.kind),
                Ok(DecisionKind::Priority(_))
            )
    }
}

fn p170_zone(driver: &Driver, id: ObjectId) -> Option<Zone> {
    driver
        .state()
        .object(id)
        .map(mtg_engine::object::GameObject::zone)
}

/// The card called `name` in `zone`.
fn p170_find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap()
}

fn p170_hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn p170_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 12])
    })
}

#[rstest]
#[case::played(true, true)]
#[case::left_in_exile(true, false)]
#[case::an_untapped_creature_stays(false, false)]
fn prowl_exiles_a_tapped_creature_and_its_play_converts_prowl(
    #[values(2, 3, 4)] seats: usize,
    #[case] victim_tapped: bool,
    #[case] play: bool,
) {
    let bear = free_creature("Bear", 2, 2);
    let victim = if victim_tapped {
        tapped(bear)
    } else {
        ready(bear)
    };
    let prowl = "Prowl, Stoic Strategist // Prowl, Pursuit Vehicle";
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card(prowl).unwrap())])
        .battlefield(1, [victim]);
    let mut driver = Driver::from_scenario(p170_libraries(scenario, seats)).unwrap();
    let bear = driver.id_of("Bear").unwrap();
    let prowl = driver.id_of("Prowl, Stoic Strategist").unwrap();
    let attack = |_: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::DeclareAttackers(_) if decision.player == pid(0) => Some(
            Answer::DeclareAttackers(vec![(prowl, AttackTarget::Player(pid(1)))]),
        ),
        _ => None,
    };
    p170_run(&mut driver, 60, &attack, &p170_settled(2, Step::Main2));
    let exiled = p170_zone(&driver, bear) == Some(Zone::Exile);
    assert_eq!(exiled, victim_tapped, "only a tapped creature is exiled");
    p170_run(&mut driver, 200, &p170_none, &p170_settled(3, Step::Main1));
    let drawn = p170_hand(&driver, 0);
    if play {
        driver.cast("Bear").unwrap();
        p170_run(&mut driver, 30, &p170_none, &p170_settled(3, Step::Main1));
        assert_eq!(p170_zone(&driver, bear), Some(Zone::Battlefield));
        assert_eq!(driver.state().object(bear).unwrap().controller(), pid(1));
    }
    assert_eq!(
        p170_hand(&driver, 0),
        drawn + usize::from(play),
        "the play draws"
    );
    let face = name_of(driver.state(), prowl);
    let expected = if play {
        "Prowl, Pursuit Vehicle"
    } else {
        "Prowl, Stoic Strategist"
    };
    assert_eq!(face, expected, "the play converts Prowl");
    p170_replays(&driver);
}

#[rstest]
#[case::one_creature(1, 1, "Prowl, Pursuit Vehicle")]
#[case::two_creatures(2, 2, "Prowl, Stoic Strategist")]
fn prowl_pursuit_vehicle_grows_and_converts_on_the_second_resolution(
    #[values(2, 3, 4)] seats: usize,
    #[case] creatures: usize,
    #[case] counters: u32,
    #[case] face: &str,
) {
    let mut hand = vec![card("Prowl, Stoic Strategist // Prowl, Pursuit Vehicle").unwrap()];
    hand.extend((0..creatures).map(|n| free_creature(&format!("Scout {n}"), 1, 1)));
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, lands(3, plains));
    let mut driver = Driver::from_scenario(p170_libraries(scenario, seats)).unwrap();
    let prowl = p170_find(&driver, Zone::Hand(pid(0)), "Prowl, Stoic Strategist");
    driver.begin_cast("Prowl, Stoic Strategist").unwrap();
    p170_run(&mut driver, 30, &p170_none, &p170_settled(2, Step::Main1));
    assert_eq!(name_of(driver.state(), prowl), "Prowl, Pursuit Vehicle");
    for n in 0..creatures {
        driver.cast(&format!("Scout {n}")).unwrap();
        p170_run(&mut driver, 30, &p170_none, &p170_settled(2, Step::Main1));
    }
    let on = driver.state().object(prowl).unwrap().permanent().unwrap();
    assert_eq!(on.counters_of(&CounterKind::PlusOnePlusOne), counters);
    assert_eq!(name_of(driver.state(), prowl), face);
    p170_replays(&driver);
}

#[rstest]
fn myra_marks_an_attraction_whose_visits_cast_copies_of_the_exiled_card(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .attractions(0, [(un33_attraction("Ride"), Lights::of(1..=6))])
        .hand(0, [un33_opener("Open One", Some(1))])
        .graveyard_named(0, vec!["Opt"])
        .battlefield(
            0,
            [
                ready(card("Myra the Magnificent").unwrap()),
                ready(island()),
                ready(island()),
            ],
        );
    let mut driver = Driver::from_scenario(p170_libraries(scenario, seats)).unwrap();
    let opt = p170_find(&driver, Zone::Graveyard(pid(0)), "Opt");
    driver.cast("Open One").unwrap();
    p170_run(&mut driver, 30, &p170_none, &p170_settled(2, Step::Main1));
    let ride = driver.id_of("Ride").unwrap();
    assert_eq!(
        p170_zone(&driver, ride),
        Some(Zone::Battlefield),
        "Myra opens it"
    );
    driver.begin_activation("Myra the Magnificent").unwrap();
    let one = |_: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(1))),
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(CardsChoice(
            options.from.first().copied().into_iter().collect(),
        ))),
        _ => None,
    };
    p170_run(&mut driver, 30, &one, &p170_settled(2, Step::Main1));
    assert_eq!(p170_zone(&driver, opt), Some(Zone::Exile));
    let midway = CounterKind::Other("midway".to_owned());
    let on = driver.state().object(ride).unwrap().permanent().unwrap();
    assert_eq!(on.counters_of(&midway), 1);
    let start = driver.engine().events().len();
    let copies = |driver: &Driver| {
        driver.engine().events()[start..]
            .iter()
            .filter(|event| matches!(event, Event::SpellCast { .. }))
            .count()
    };
    // Each of the next two turns' visits casts a copy; the card stays in exile.
    let next = 2 + u32::try_from(seats).unwrap();
    p170_run(&mut driver, 300, &one, &p170_settled(next, Step::Main1));
    assert_eq!(copies(&driver), 1, "the visit casts a copy");
    p170_run(
        &mut driver,
        300,
        &one,
        &p170_settled(next + u32::try_from(seats).unwrap(), Step::Main1),
    );
    assert_eq!(copies(&driver), 2, "every visit casts a copy");
    assert_eq!(p170_zone(&driver, opt), Some(Zone::Exile));
    p170_replays(&driver);
}

#[rstest]
#[case("Scotty, Miracle Worker")]
#[case("Prowl, Stoic Strategist // Prowl, Pursuit Vehicle")]
#[case("Myra the Magnificent")]
fn p170_the_cards_are_verified(#[case] name: &str) {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{name}: {support:?}"
    );
}
