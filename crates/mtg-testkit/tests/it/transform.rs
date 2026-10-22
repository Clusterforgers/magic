//! Goal: a transforming double-faced card turns over on the battlefield as the same object: Delver of Secrets from the reveal of its upkeep trigger (an instant or sorcery on top and the player's yes, at two to four seats) and Ludevic's Test Subject from its fifth counter, with the other face's name, power, toughness and abilities; its counters, tapped state, summoning sickness and attachments stay; it enters front face up whether cast or put onto the battlefield, and leaves as its front face; a card with one face does not transform; and a log with transformations replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, apply_event};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, with_counters};
use rstest::rstest;

use crate::common::casting::resolve;
use crate::common::{name_of, names, pid};

const DELVER: &str = "Delver of Secrets // Insectile Aberration";
const EGG: &str = "Ludevic's Test Subject // Ludevic's Abomination";

fn stats(driver: &Driver, id: ObjectId) -> (i32, i32) {
    let found = characteristics(driver.state(), id).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

/// Seat 0 controls Delver; its upkeep comes round at turn `seats + 1`, with `top` on the library.
fn delver_at_upkeep(seats: usize, top: &str) -> Result<(Driver, ObjectId), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield_named(0, [DELVER])
        .library_named(0, [top]);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let delver = driver.id_of("Delver of Secrets")?;
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Upkeep)?;
    Ok((driver, delver))
}

#[rstest]
#[case::an_instant_and_yes("Lightning Bolt", true, true)]
#[case::a_sorcery_and_yes("Rampant Growth", true, true)]
#[case::an_instant_and_no("Lightning Bolt", false, false)]
#[case::a_creature_and_yes("Grizzly Bears", true, false)]
fn delvers_upkeep_trigger_turns_it_over_for_an_instant_or_a_sorcery_that_is_revealed(
    #[values(2, 3, 4)] seats: usize,
    #[case] top: &str,
    #[case] reveal: bool,
    #[case] turns: bool,
) -> Result<(), ScriptError> {
    let (mut driver, delver) = delver_at_upkeep(seats, top)?;
    assert_eq!(stats(&driver, delver), (1, 1));
    // The trigger resolves: look at the top card, then the player may reveal it.
    driver.pass()?;
    for _ in 1..seats {
        driver.pass()?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
        driver.answer(Answer::YesNo(YesNoChoice(reveal)))?;
    }
    let object = driver.state().object(delver).unwrap();
    let (name, power) = (object.card().name.clone(), stats(&driver, delver));
    if turns {
        assert_eq!(name, "Insectile Aberration");
        assert_eq!(power, (3, 2));
        assert!(has_keyword(driver.state(), delver, &Keyword::Flying));
        assert_eq!(object.face(), Some(1));
    } else {
        assert_eq!(name, "Delver of Secrets");
        assert_eq!(power, (1, 1));
        assert_eq!(object.face(), None);
    }
    Ok(())
}

#[test]
fn the_same_object_turns_over_with_everything_it_remembers() -> Result<(), ScriptError> {
    let mut egg = with_counters(
        sick(card(EGG).unwrap()),
        CounterKind::Other("hatchling".into()),
        2,
    );
    egg.tapped = true;
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [egg]))?;
    let id = driver.id_of("Ludevic's Test Subject")?;
    let before = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .clone();
    let mut turned = driver.state().clone();
    apply_event(
        &mut turned,
        &Event::Transformed {
            object: id,
            face: Some(1),
        },
    )
    .unwrap();
    let now = turned.object(id).unwrap();
    assert_eq!(now.card().name, "Ludevic's Abomination");
    let kept = now.permanent().unwrap();
    assert_eq!(
        (
            kept.tapped(),
            kept.summoning_sick(),
            kept.counters().count()
        ),
        (
            before.tapped(),
            before.summoning_sick(),
            before.counters().count()
        )
    );
    assert!(kept.tapped() && kept.summoning_sick());
    assert_eq!(now.counter(), driver.state().object(id).unwrap().counter());
    // And back.
    apply_event(
        &mut turned,
        &Event::Transformed {
            object: id,
            face: None,
        },
    )
    .unwrap();
    assert_eq!(
        turned.object(id).unwrap().card().name,
        "Ludevic's Test Subject"
    );
    Ok(())
}

#[test]
fn ludevics_test_subject_hatches_at_the_fifth_counter() -> Result<(), ScriptError> {
    let mut permanents = vec![ready(card(EGG).unwrap())];
    permanents.extend((0..10).map(|_| ready(island())));
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, permanents))?;
    let egg = driver.id_of("Ludevic's Test Subject")?;
    for count in 1..=4u32 {
        driver.begin_activation("Ludevic's Test Subject")?;
        driver.pay_tapping(&["Island", "Island"], 0)?;
        resolve(&mut driver)?;
        let kept = driver.state().object(egg).unwrap();
        assert_eq!(kept.card().name, "Ludevic's Test Subject");
        assert_eq!(
            kept.permanent()
                .unwrap()
                .counters()
                .map(|(_, held)| held)
                .sum::<u32>(),
            count
        );
    }
    driver.begin_activation("Ludevic's Test Subject")?;
    driver.pay_tapping(&["Island", "Island"], 0)?;
    resolve(&mut driver)?;
    let hatched = driver.state().object(egg).unwrap();
    assert_eq!(hatched.card().name, "Ludevic's Abomination");
    assert_eq!(stats(&driver, egg), (13, 13));
    assert!(has_keyword(driver.state(), egg, &Keyword::Trample));
    assert_eq!(hatched.permanent().unwrap().counters().count(), 0);
    Ok(())
}

#[test]
fn a_card_enters_front_face_up_and_leaves_as_its_front_face() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand_named(0, [DELVER])
            .battlefield(0, [ready(island())]),
    )?;
    driver.cast_tapping("Delver of Secrets", &["Island"])?;
    resolve(&mut driver)?;
    let delver = driver.id_of("Delver of Secrets")?;
    assert_eq!(driver.state().object(delver).unwrap().face(), None);
    // Turned over, then it dies: the card in the graveyard is the front face again.
    let mut turned = driver.state().clone();
    apply_event(
        &mut turned,
        &Event::Transformed {
            object: delver,
            face: Some(1),
        },
    )
    .unwrap();
    apply_event(
        &mut turned,
        &Event::CreatureDied {
            object: delver,
            cause: mtg_engine::events::DeathCause::LethalDamage,
        },
    )
    .unwrap();
    assert_eq!(names(&turned, Zone::Graveyard(pid(0))), [DELVER_FRONT]);
    assert_eq!(name_of(&turned, delver), DELVER_FRONT);
    assert_eq!(turned.object(delver).unwrap().face(), None);
    Ok(())
}

const DELVER_FRONT: &str = "Delver of Secrets";

#[test]
fn a_permanent_that_is_not_a_transforming_card_is_refused_by_the_fold() {
    let driver =
        Driver::from_scenario(Scenario::new(2).battlefield_named(0, ["Grizzly Bears"])).unwrap();
    let bears = driver.id_of("Grizzly Bears").unwrap();
    let mut state = driver.state().clone();
    assert!(
        apply_event(
            &mut state,
            &Event::Transformed {
                object: bears,
                face: Some(1)
            }
        )
        .is_err()
    );
    assert_eq!(state.object(bears).unwrap().face(), None);
}

#[rstest]
fn a_game_with_a_transformation_replays_to_the_same_state(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, _) = delver_at_upkeep(seats, "Lightning Bolt")?;
    driver.pass()?;
    for _ in 1..seats {
        driver.pass()?;
    }
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let setup = driver.engine().setup().clone();
    let replayed = mtg_engine::replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Transformed { .. }))
    );
    let _ = Zone::Battlefield;
    Ok(())
}

#[test]
fn only_the_front_face_of_a_double_faced_card_is_the_card_in_a_hand() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).hand_named(0, [DELVER]))?;
    let delver = driver.state().zone(Zone::Hand(pid(0)))[0];
    assert_eq!(name_of(driver.state(), delver), DELVER_FRONT);
    assert_eq!(driver.state().object(delver).unwrap().face(), None);
    assert_eq!(stats(&driver, delver), (1, 1));
    Ok(())
}
