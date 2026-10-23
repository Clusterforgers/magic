//! Goal: a token made for each token its controller controls (Second Harvest, Rhys the Redeemed) copies tokens only, its controller's alone, whatever they are copies of, and nothing when there are none; a permanent that enters as a copy with exceptions (Auton Soldier, Malleable Impostor, Imposter Mech, Machine God's Effigy) is offered the creatures its words name, takes the copiable values of the one chosen with the exceptions applied, leaves the original as it was and replays from the log, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::ability_reader::abilities_of;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, characteristics};
use mtg_testkit::cards::{creature, forest, island, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{replays, settle};
use crate::common::pid;

const HERO: &str = "Hero of Bladehold";

/// The permanents of the battlefield named `name`, in id order.
fn named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let mut found: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect();
    found.sort();
    found
}

/// The tokens among the permanents named `name`.
fn tokens(driver: &Driver, name: &str) -> Vec<ObjectId> {
    named(driver, name)
        .into_iter()
        .filter(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
        .collect()
}

fn has_keyword(driver: &Driver, id: ObjectId, name: &str) -> bool {
    keywords_of(driver.state(), id)
        .iter()
        .any(|keyword| keyword.name() == name)
}

fn is_type(driver: &Driver, id: ObjectId, kind: CardType) -> bool {
    characteristics(driver.state(), id)
        .unwrap()
        .types
        .contains(kind)
}

/// Seat 0 controls Hero of Bladehold with six Islands, six Plains and four Forests, holds `hand` and may have Rhys;
/// every other seat controls a creature of its own.
fn table(seats: usize, hand: &[&str], rhys: bool) -> Scenario {
    let mut ours = vec![ready(card(HERO).unwrap())];
    if rhys {
        ours.push(ready(card("Rhys the Redeemed").unwrap()));
    }
    ours.extend((0..6).map(|_| ready(island())));
    ours.extend((0..6).map(|_| ready(plains())));
    ours.extend((0..4).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name).unwrap()))
        .battlefield(0, ours);
    for seat in 1..seats {
        let guard = creature(&format!("Guard {seat}"), 1, &[Color::Red], 2, 2);
        scenario = scenario.battlefield(seat, [ready(guard)]);
    }
    scenario
}

/// Casts Cackling Counterpart on Hero of Bladehold: one more token that is a copy of it.
fn make_a_token(driver: &mut Driver) -> Result<(), ScriptError> {
    let target = driver.target_named(HERO)?;
    driver.begin_cast("Cackling Counterpart")?;
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&["Island", "Island", "Island"], 0)?;
    settle(driver)
}

#[rstest]
#[case::second_harvest(false)]
#[case::rhys_the_redeemed(true)]
fn a_token_is_made_for_each_token_its_controller_controls(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] made: usize,
    #[case] rhys: bool,
) -> Result<(), ScriptError> {
    let hand: Vec<&str> = std::iter::repeat_n("Cackling Counterpart", made)
        .chain(if rhys { None } else { Some("Second Harvest") })
        .collect();
    let setup = table(seats, &hand, rhys).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &hand, rhys))?;
    for _ in 0..made {
        make_a_token(&mut driver)?;
    }
    let before = tokens(&driver, HERO);
    assert_eq!(before.len(), made);
    if rhys {
        driver.begin_ability("Rhys the Redeemed", 1)?;
        driver.pay_tapping(&["Plains"; 6], 0)?;
    } else {
        driver.cast_tapping("Second Harvest", &["Forest"; 4])?;
    }
    settle(&mut driver)?;
    let after = tokens(&driver, HERO);
    // One new token for each old one, and the nontoken Hero and the creatures of the other seats are left alone.
    assert_eq!(after.len(), 2 * made);
    assert_eq!(named(&driver, HERO).len(), 1 + 2 * made);
    for seat in 1..seats {
        assert_eq!(named(&driver, &format!("Guard {seat}")).len(), 1);
    }
    for new in after.iter().filter(|id| !before.contains(id)) {
        let object = driver.state().object(*new).unwrap();
        assert_eq!(object.controller(), pid(0));
        let of = object.permanent().unwrap().copy().unwrap().of().id;
        assert!(before.contains(&of), "a copy of one of the tokens");
    }
    if rhys {
        // The ability taps Rhys, and the Elf Warrior of his other ability is not made.
        assert!(named(&driver, "Elf Warrior").is_empty());
    }
    replays(&driver, &setup);
    Ok(())
}

/// The creatures of the other seats and the one seat 0 controls, for a permanent that enters as a copy.
fn copy_table(seats: usize, card_name: &str) -> Scenario {
    let mut lands: Vec<_> = (0..6).map(|_| ready(island())).collect();
    lands.push(ready(creature("Own Guard", 1, &[Color::Blue], 1, 1)));
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(card_name).unwrap()])
        .battlefield(0, lands);
    scenario = scenario.battlefield(
        1,
        [ready(legend("Opposing Legend", 1, &[Color::Blue], 2, 3))],
    );
    for seat in 2..seats {
        let guard = creature(&format!("Guard {seat}"), 1, &[Color::Red], 2, 2);
        scenario = scenario.battlefield(seat, [ready(guard)]);
    }
    scenario
}

/// Casts `spell` with `taps` Islands and passes until its controller is asked what to copy.
fn cast_until_asked(
    driver: &mut Driver,
    spell: &str,
    taps: usize,
) -> Result<Vec<ObjectId>, ScriptError> {
    driver.cast_tapping(spell, &vec!["Island"; taps])?;
    for _ in 0..50 {
        if let Some(DecisionKind::ChooseCards(options)) =
            driver.engine().decision().map(|d| d.kind.clone())
        {
            assert_eq!(driver.decision()?.player, pid(0));
            assert!(driver.engine().replacing());
            return Ok(options.from);
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the copy".into()))
}

/// The copy that entered: the permanent named `name` that is not `original`.
fn the_copy(driver: &Driver, name: &str, original: ObjectId) -> ObjectId {
    *named(driver, name)
        .iter()
        .find(|id| **id != original)
        .unwrap()
}

#[rstest]
#[case::auton_soldier("Auton Soldier", 6)]
#[case::malleable_impostor("Malleable Impostor", 4)]
#[case::imposter_mech("Imposter Mech", 2)]
#[case::machine_gods_effigy("Machine God's Effigy", 4)]
fn a_permanent_that_enters_as_a_copy_is_offered_the_creatures_its_words_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] taps: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(copy_table(seats, spell))?;
    let own = named(&driver, "Own Guard")[0];
    let legend = named(&driver, "Opposing Legend")[0];
    let mut options = cast_until_asked(&mut driver, spell, taps)?;
    options.sort();
    // "a creature an opponent controls" leaves out the creature of the caster; "any creature" offers every one.
    let mut expected: Vec<ObjectId> = (1..seats)
        .flat_map(|seat| {
            let name = if seat == 1 {
                "Opposing Legend".to_owned()
            } else {
                format!("Guard {seat}")
            };
            named(&driver, &name)
        })
        .collect();
    if spell != "Malleable Impostor" && spell != "Imposter Mech" {
        expected.push(own);
    }
    expected.sort();
    assert_eq!(options, expected);
    assert!(options.contains(&legend));
    Ok(())
}

/// Casts `spell`, copies the legendary creature of seat 1 and settles; returns the copy and the original.
fn copy_the_legend(
    seats: usize,
    spell: &str,
    taps: usize,
) -> Result<(Driver, GameSetup, ObjectId, ObjectId), ScriptError> {
    let setup = copy_table(seats, spell).setup().unwrap();
    let mut driver = Driver::from_scenario(copy_table(seats, spell))?;
    let legend = named(&driver, "Opposing Legend")[0];
    cast_until_asked(&mut driver, spell, taps)?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![legend])))?;
    settle(&mut driver)?;
    let copy = the_copy(&driver, "Opposing Legend", legend);
    Ok((driver, setup, copy, legend))
}

#[rstest]
fn auton_soldier_is_an_artifact_creature_with_myriad_that_is_not_legendary(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, setup, copy, legend) = copy_the_legend(seats, "Auton Soldier", 6)?;
    let card = driver.state().object(copy).unwrap().card().clone();
    assert!(!card.supertypes.contains(&Supertype::Legendary));
    assert!(is_type(&driver, copy, CardType::Artifact));
    assert!(is_type(&driver, copy, CardType::Creature));
    assert!(has_keyword(&driver, copy, "Myriad"));
    // The legend and its copy, which is not one, both stay; the original has neither the type nor the keyword.
    assert_eq!(named(&driver, "Opposing Legend").len(), 2);
    assert!(
        driver
            .state()
            .object(legend)
            .unwrap()
            .card()
            .supertypes
            .contains(&Supertype::Legendary)
    );
    assert!(!is_type(&driver, legend, CardType::Artifact));
    assert!(!has_keyword(&driver, legend, "Myriad"));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn malleable_impostor_is_a_faerie_shapeshifter_with_flying_beside_the_creature_it_copies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, setup, copy, legend) = copy_the_legend(seats, "Malleable Impostor", 4)?;
    let card = driver.state().object(copy).unwrap().card().clone();
    for kind in ["Faerie", "Shapeshifter"] {
        assert!(card.subtypes.contains(&Subtype(kind.into())), "{kind}");
    }
    assert!(has_keyword(&driver, copy, "Flying"));
    assert!(!has_keyword(&driver, legend, "Flying"));
    // It is still a legend of the same name under another player's control: no legend rule for the two of them.
    assert!(card.supertypes.contains(&Supertype::Legendary));
    assert_eq!(named(&driver, "Opposing Legend").len(), 2);
    assert_eq!(driver.state().object(copy).unwrap().controller(), pid(0));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn imposter_mech_is_a_vehicle_artifact_with_crew_and_no_longer_a_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, setup, copy, legend) = copy_the_legend(seats, "Imposter Mech", 2)?;
    let card = driver.state().object(copy).unwrap().card().clone();
    assert!(is_type(&driver, copy, CardType::Artifact));
    assert!(!is_type(&driver, copy, CardType::Creature));
    assert!(card.subtypes.contains(&Subtype("Vehicle".into())));
    assert!(has_keyword(&driver, copy, "Crew"));
    assert!(is_type(&driver, legend, CardType::Creature));
    assert!(!has_keyword(&driver, legend, "Crew"));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn machine_gods_effigy_is_only_an_artifact_that_taps_for_blue_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, setup, copy, legend) = copy_the_legend(seats, "Machine God's Effigy", 4)?;
    assert!(is_type(&driver, copy, CardType::Artifact));
    assert!(!is_type(&driver, copy, CardType::Creature));
    // The copied values, and the ability the exception gives it, which the legend lacks.
    assert_eq!(
        abilities_of(driver.state(), copy).len(),
        abilities_of(driver.state(), legend).len() + 1
    );
    replays(&driver, &setup);
    Ok(())
}
