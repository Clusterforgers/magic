//! Goal: a manual action taken out of band between the questions of an announcement or of one resolution step never leaves the game stuck: a pick for a cost part it tapped or moved is asked again among the candidates of the new state (or the activation ends when none is left), a choice of cards a seat gave before it moved that card is asked again while the answers it left valid stand, and a question the manual action raised itself (a permanent that chooses as it enters) is answered as that question and not as one of the announcement, at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, cost, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::part_cards::levy;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// What a manual action does to the object it is about.
#[derive(Debug, Clone, Copy)]
enum Touch {
    Tap,
    ToHand,
    ToGraveyard,
}

impl Touch {
    fn action(self, object: ObjectId) -> ManualAction {
        match self {
            Touch::Tap => ManualAction::Tap { object },
            Touch::ToHand => ManualAction::Move {
                object,
                to: ManualZone::Hand,
            },
            Touch::ToGraveyard => ManualAction::Move {
                object,
                to: ManualZone::Graveyard,
            },
        }
    }
}

/// The cards the pending choice of cards offers, by name.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a choice of cards was expected: {:?}", driver.decision());
    };
    options
        .from
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect()
}

#[rstest]
fn a_pick_for_a_cost_part_a_manual_action_takes_away_is_asked_again_among_the_candidates_left(
    #[values(2, 3, 4)] seats: usize,
    #[values(Touch::Tap, Touch::ToHand, Touch::ToGraveyard)] touch: Touch,
    #[values(true, false)] others: bool,
) -> Result<(), ScriptError> {
    // "{1}, Tap an untapped creature you control: Draw a card."
    let creatures = if others {
        vec!["Bear", "Wolf", "Ox"]
    } else {
        vec!["Bear"]
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(
                0,
                [ready(levy("Levy")), ready(mountain())].into_iter().chain(
                    creatures
                        .iter()
                        .map(|name| ready(free_creature(name, 2, 2))),
                ),
            )
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1))),
    )?;
    let bear = driver.id_of("Bear")?;
    driver.begin_activation("Levy")?;
    if others {
        assert_eq!(offered(&driver), ["Bear", "Wolf", "Ox"]);
        driver.answer(Answer::ChooseCards(CardsChoice(vec![bear])))?;
    }
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    driver.manual_by(pid(0), &touch.action(bear))?;
    if !others {
        // Nothing is left to tap for the cost: the activation ends without a trace and priority returns.
        assert!(!driver.engine().announcing());
        assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
        assert_eq!(driver.state().stack(), []);
        return Ok(());
    }
    assert_eq!(offered(&driver), ["Wolf", "Ox"]);
    let wolf = driver.id_of("Wolf")?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![wolf])))?;
    driver.pay_tapping(&["Mountain"], 0)?;
    assert_eq!(driver.state().stack().len(), 1);
    assert!(
        driver
            .state()
            .object(wolf)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    Ok(())
}

/// A free sorcery: "Each player sacrifices a creature."
fn purge() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Purge")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::Sacrifice {
                    who: Ref::EachPlayer,
                    filter: Selector(vec![Pred::Type(CardType::Creature)]),
                    count: Amount::Fixed(1),
                }],
            }),
    )
}

/// What the manual action between the first and the second seat's answer does.
#[derive(Debug, Clone, Copy)]
enum Between {
    /// Moves the creature the first seat chose: that answer no longer fits and the first seat is asked again.
    MovesThePick,
    /// Taps the creature the first seat chose: the answer still fits and stands.
    TapsThePick,
    /// Moves a creature of the second seat: the first answer stands, the second seat is asked among those left.
    MovesTheNext,
}

#[rstest]
fn a_choice_of_cards_a_manual_action_unsettles_between_two_seats_is_asked_again(
    #[values(2, 3, 4)] seats: usize,
    #[values(Between::MovesThePick, Between::TapsThePick, Between::MovesTheNext)] between: Between,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [purge()]);
    for seat in 0..seats {
        scenario = scenario
            .battlefield(
                seat,
                ["A", "B", "C"].map(|kind| ready(free_creature(&format!("{kind}{seat}"), 1, 1))),
            )
            .library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Purge")?;
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        driver.pass()?;
    }
    assert_eq!(driver.decision()?.player, pid(0));
    let pick = driver.id_of("A0")?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
    assert_eq!(driver.decision()?.player, pid(1));
    let (owner, action) = match between {
        Between::MovesThePick => (0, Touch::ToHand.action(pick)),
        Between::TapsThePick => (0, Touch::Tap.action(pick)),
        Between::MovesTheNext => (1, Touch::ToHand.action(driver.id_of("A1")?)),
    };
    driver.manual_by(pid(owner), &action)?;
    // The step asked again is in the log: a game resumed from it stands at the same question.
    let resumed = mtg_engine::Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )?;
    assert_eq!(resumed.decision(), driver.engine().decision());
    let (asked, left) = match between {
        Between::MovesThePick => (0, ["B0", "C0"]),
        Between::TapsThePick => (1, ["A1", "B1"]),
        Between::MovesTheNext => (1, ["B1", "C1"]),
    };
    assert_eq!(driver.decision()?.player, pid(asked));
    assert_eq!(&offered(&driver)[..2], left);
    // Every seat asked from here on sacrifices the first creature it is offered.
    while let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
        let first = options.from[0];
        driver.answer(Answer::ChooseCards(CardsChoice(vec![first])))?;
    }
    assert!(driver.state().resolving().is_none());
    for seat in 0..seats {
        let gone: Vec<String> = names(driver.state(), Zone::Graveyard(pid(seat)))
            .into_iter()
            .filter(|name| name != "Purge")
            .collect();
        let expected = match (between, seat) {
            (Between::TapsThePick, 0) => "A0".to_owned(),
            (Between::MovesThePick, 0) => "B0".to_owned(),
            (Between::MovesTheNext, 1) => "B1".to_owned(),
            _ => format!("A{seat}"),
        };
        assert_eq!(gone, [expected], "seat {seat}");
    }
    Ok(())
}

#[rstest]
fn a_question_a_manual_action_raises_mid_announcement_is_its_own_and_the_announcement_goes_on(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] mover: usize,
) -> Result<(), ScriptError> {
    // Hall of Triumph: "As Hall of Triumph enters, choose a color."
    let hall = || card("Hall of Triumph").unwrap();
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    scenario = if mover == 0 {
        scenario.hand(0, [burn("Bolt", 3), hall()])
    } else {
        scenario.hand(0, [burn("Bolt", 3)]).hand(mover, [hall()])
    };
    let mut driver = Driver::from_scenario(
        scenario
            .battlefield(0, [ready(mountain())])
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1))),
    )?;
    let hall = *driver.state().zone(Zone::Hand(pid(mover))).last().unwrap();
    driver.begin_cast("Bolt")?;
    driver.choose_targets(&[driver.target_player(1)])?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    driver.manual_by(
        pid(mover),
        &ManualAction::Move {
            object: hall,
            to: ManualZone::Battlefield,
        },
    )?;
    assert_eq!(driver.decision()?.player, pid(mover));
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseOption(_)
    ));
    driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Hall of Triumph".to_owned()));
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    driver.pay_tapping(&["Mountain"], 0)?;
    assert_eq!(driver.state().stack().len(), 1);
    Ok(())
}
