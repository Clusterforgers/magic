//! Goal: the legend rule (CR 704.5j) works as the rules say, at tables of two, three and four seats: a player who controls two, three or four legendary permanents with one name is asked which to keep and the rest go to their owners' graveyards together, put and not destroyed (an indestructible one goes too); a player with several names is asked for each in turn; copies of other players are not counted and each player who has copies is asked in turn order from the active player; the player who controls the permanents chooses, not their owner; an answer that does not name exactly one of the permanents is refused and leaves the game as it was; an engine rebuilt from the log asks the same question; and the corpus legends do what their cards say.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, characteristics, replay};
use mtg_testkit::attach_cards::{legendary_bear, legendary_ogre};
use mtg_testkit::cards::{bear, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn indestructible_legend(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_supertype(Supertype::Legendary)
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::Indestructible)),
    )
}

fn name_of(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// The question the engine is asking: who, and which permanents.
fn question(driver: &Driver) -> Option<(PlayerId, Vec<ObjectId>)> {
    match &driver.decision().ok()?.kind {
        DecisionKind::ChooseCards(options) if driver.state().resolving().is_none() => {
            Some((driver.decision().ok()?.player, options.from.clone()))
        }
        _ => None,
    }
}

fn keep(driver: &mut Driver, card: ObjectId) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::ChooseCards(CardsChoice(vec![card])))
}

fn on_battlefield(driver: &Driver, seat: usize) -> Vec<String> {
    let mut found: Vec<String> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(seat))
        .map(|o| o.card().name.clone())
        .collect();
    found.sort();
    found
}

fn copies(card: &Arc<CardDefinition>, count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(card.clone())).collect()
}

#[rstest]
fn the_controller_of_two_to_four_copies_chooses_one_and_the_rest_go_to_the_graveyard_together(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3, 4)] count: usize,
    #[values(0, 1, 2, 3)] kept: usize,
) -> Result<(), ScriptError> {
    if kept >= count {
        return Ok(());
    }
    let scenario = || Scenario::new(seats).battlefield(0, copies(&legendary_bear(), count));
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let (who, from) = question(&driver).expect("the rule asks at once");
    assert_eq!(who, pid(0));
    assert_eq!(from.len(), count);
    assert!(
        from.windows(2).all(|pair| pair[0] < pair[1]),
        "oldest first"
    );
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        unreachable!()
    };
    assert_eq!((options.min, options.max), (1, 1));
    assert!(options.text.contains("Legendary Bear"), "{}", options.text);
    let begun = driver.engine().events().len();
    let events = keep(&mut driver, from[kept])?;
    // The rest are put into their owner's graveyard in one batch, none of them destroyed.
    let put: Vec<ObjectId> = events
        .iter()
        .filter_map(|event| match event {
            Event::ZoneChanged {
                object,
                from: Zone::Battlefield,
                to: Zone::Graveyard(owner),
                cause: ZoneCause::StateBased,
            } if *owner == pid(0) => Some(*object),
            _ => None,
        })
        .collect();
    let mut expected: Vec<ObjectId> = from
        .iter()
        .copied()
        .filter(|id| *id != from[kept])
        .collect();
    expected.sort();
    let mut put_sorted = put.clone();
    put_sorted.sort();
    assert_eq!(put_sorted, expected);
    assert!(
        !driver.engine().events()[begun..]
            .iter()
            .any(|e| matches!(e, Event::CreatureDied { .. }))
    );
    assert_eq!(on_battlefield(&driver, 0), ["Legendary Bear"]);
    assert!(driver.state().zone(Zone::Battlefield).contains(&from[kept]));
    assert!(question(&driver).is_none());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn a_player_with_several_names_is_asked_for_each_in_turn() -> Result<(), ScriptError> {
    let mut permanents = copies(&legendary_ogre(), 3);
    permanents.extend(copies(&legendary_bear(), 2));
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, permanents))?;
    let (_, first) = question(&driver).unwrap();
    assert_eq!(first.len(), 2, "the Bears come first, by name");
    assert_eq!(name_of(&driver, first[0]), "Legendary Bear");
    keep(&mut driver, first[1])?;
    assert_eq!(
        on_battlefield(&driver, 0),
        [
            "Legendary Bear",
            "Legendary Ogre",
            "Legendary Ogre",
            "Legendary Ogre"
        ]
    );
    let (who, second) = question(&driver).expect("the Ogres are asked next");
    assert_eq!((who, second.len()), (pid(0), 3));
    assert_eq!(name_of(&driver, second[0]), "Legendary Ogre");
    keep(&mut driver, second[0])?;
    assert_eq!(
        on_battlefield(&driver, 0),
        ["Legendary Bear", "Legendary Ogre"]
    );
    assert!(question(&driver).is_none());
    Ok(())
}

#[rstest]
fn the_copies_of_other_players_are_not_counted(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, copies(&legendary_bear(), 1));
    }
    let driver = Driver::from_scenario(scenario)?;
    assert!(question(&driver).is_none());
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    for seat in 0..seats {
        assert_eq!(on_battlefield(&driver, seat), ["Legendary Bear"]);
    }
    Ok(())
}

#[rstest]
fn each_player_with_copies_is_asked_in_turn_order_from_the_active_player(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 1, mtg_engine::turn::Step::Main1);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, copies(&legendary_bear(), 2));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let mut asked = Vec::new();
    while let Some((who, from)) = question(&driver) {
        asked.push(who.index());
        keep(&mut driver, from[0])?;
    }
    let expected: Vec<usize> = (0..seats).map(|n| (1 + n) % seats).collect();
    assert_eq!(asked, expected);
    for seat in 0..seats {
        assert_eq!(on_battlefield(&driver, seat), ["Legendary Bear"]);
    }
    Ok(())
}

#[test]
fn a_legend_is_put_into_the_graveyard_not_destroyed_so_indestructible_does_not_save_it()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, copies(&indestructible_legend("Eternal Legend"), 2)),
    )?;
    let (_, from) = question(&driver).unwrap();
    let events = keep(&mut driver, from[0])?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ZoneChanged {
            cause: ZoneCause::StateBased,
            to: Zone::Graveyard(_),
            ..
        }
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        Event::ZoneChanged {
            cause: ZoneCause::Destroyed | ZoneCause::Buried,
            ..
        }
    )));
    assert_eq!(on_battlefield(&driver, 0), ["Eternal Legend"]);
    Ok(())
}

#[test]
fn control_decides_who_chooses_and_who_owns_the_card_that_goes() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Control Magic"])
            .battlefield(
                0,
                [
                    ready(island()),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                    ready(legendary_bear()),
                ],
            )
            .battlefield(1, [ready(legendary_bear())]),
    )?;
    assert!(question(&driver).is_none(), "one Bear each is no duplicate");
    let theirs = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.owner() == pid(1) && o.card().name == "Legendary Bear")
        })
        .unwrap();
    let target = Target::Object(driver.state().object(theirs).unwrap().reference());
    driver.cast_at("Control Magic", &[target])?;
    for _ in 0..10 {
        if question(&driver).is_some() {
            break;
        }
        driver.pass()?;
    }
    let (who, from) = question(&driver).expect("seat 0 controls two Bears now");
    assert_eq!(who, pid(0));
    assert_eq!(from.len(), 2);
    // Keeping its own Bear puts the other seat's into that seat's graveyard.
    let own = from.iter().copied().find(|id| *id != theirs).unwrap();
    keep(&mut driver, own)?;
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(1))),
        ["Legendary Bear"]
    );
    // Its Aura lost its host with the Bear, so it follows.
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(0))),
        ["Control Magic"]
    );
    Ok(())
}

#[rstest]
#[case::nothing(vec![])]
#[case::two_cards(vec![0, 1])]
fn an_answer_that_does_not_name_exactly_one_is_refused_and_changes_nothing(
    #[case] picks: Vec<usize>,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, copies(&legendary_bear(), 2)))?;
    let (_, from) = question(&driver).unwrap();
    let hash = driver.state().state_hash();
    let cards: Vec<ObjectId> = picks.iter().map(|at| from[*at]).collect();
    let refused = driver.answer(Answer::ChooseCards(CardsChoice(cards)));
    assert!(
        matches!(
            refused,
            Err(ScriptError::Engine(EngineError::Illegal(
                IllegalAnswer::WrongCount { .. }
            )))
        ),
        "{refused:?}"
    );
    assert_eq!(driver.state().state_hash(), hash);
    assert_eq!(question(&driver).map(|(_, f)| f), Some(from));
    Ok(())
}

#[test]
fn a_card_that_is_not_one_of_the_copies_is_refused() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(legendary_bear()),
            ready(legendary_bear()),
            ready(bear()),
        ],
    ))?;
    let stranger = driver.id_of("Grizzly Bears")?;
    let refused = keep(&mut driver, stranger);
    assert!(
        matches!(
            refused,
            Err(ScriptError::Engine(EngineError::Illegal(
                IllegalAnswer::NotOffered(_)
            )))
        ),
        "{refused:?}"
    );
    Ok(())
}

#[rstest]
fn a_player_who_concedes_while_asked_is_out_and_the_question_goes_with_them(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(1, copies(&legendary_bear(), 2))
            .battlefield(seats - 1, copies(&legendary_ogre(), 2)),
    )?;
    let (who, _) = question(&driver).unwrap();
    assert_eq!(who, pid(1));
    driver.concede(pid(1))?;
    if seats == 2 {
        assert!(driver.engine().result().is_some(), "the other player wins");
        return Ok(());
    }
    // The next group, if another seat has one, is asked of that seat.
    let next = question(&driver).map(|(who, _)| who);
    assert_eq!(next, (seats > 2).then(|| pid(seats - 1)));
    Ok(())
}

#[test]
fn an_engine_rebuilt_from_the_log_asks_the_same_question() -> Result<(), ScriptError> {
    let scenario = || Scenario::new(3).battlefield(1, copies(&legendary_bear(), 3));
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let asked = driver.decision()?.clone();
    let resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec()).unwrap();
    assert_eq!(resumed.decision(), Some(&asked));
    assert_eq!(asked.player, pid(1));
    let (_, from) = question(&driver).unwrap();
    keep(&mut driver, from[2])?;
    let resumed = Engine::resume(setup, driver.engine().events().to_vec()).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

#[test]
fn arvad_the_cursed_pumps_the_other_legendary_creatures_and_kongming_the_other_creatures()
-> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(card("Arvad the Cursed").unwrap()),
            ready(card("Kongming, \"Sleeping Dragon\"").unwrap()),
            ready(bear()),
        ],
    ))?;
    let stats = |name: &str| {
        let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
        (chars.power.unwrap(), chars.toughness.unwrap())
    };
    // Arvad is 3/3 and Kongming's own bonus reaches her; Kongming is 2/2 and Arvad's bonus reaches him.
    assert_eq!(stats("Arvad the Cursed"), (4, 4));
    assert_eq!(stats("Kongming, \"Sleeping Dragon\""), (4, 4));
    assert_eq!(stats("Grizzly Bears"), (3, 3));
    Ok(())
}
