//! Goal: a group that becomes a copy (Mirrorweave, Nanogene Conversion), a token made for each creature a target player controls (Clone Legion) or five times when kicked (Rite of Replication), and a permanent that enters as a copy of a card in a graveyard (Body Double), tapped (Vesuva) or for a turn (Cursed Mirror) take the copiable values with the exceptions, leave the originals as they were, end with the turn where they should and replay from the log, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics;
use mtg_engine::choose::{CardsChoice, CostsChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, island, mountain};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{replays, settle};
use crate::common::pid;

/// Seat 0 holds `spell` over ten Islands and four Mountains and controls a legend and a bear; seat 1 controls a legend
/// and, with `graveyard`, has a creature card in its graveyard; every other seat controls a guard.
fn table(seats: usize, hand: &[&str]) -> Scenario {
    let mut ours: Vec<_> = (0..10).map(|_| ready(island())).collect();
    ours.extend((0..4).map(|_| ready(mountain())));
    ours.push(ready(legend("Own Legend", 1, &[Color::Blue], 2, 2)));
    ours.push(ready(creature("Own Bear", 1, &[Color::Blue], 3, 3)));
    let mut scenario = Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name).unwrap()))
        .battlefield(0, ours)
        .battlefield(
            1,
            [
                ready(legend("Opposing Legend", 1, &[Color::Blue], 2, 3)),
                ready(creature("Opposing Guard", 1, &[Color::Red], 1, 1)),
            ],
        )
        .graveyard(
            1,
            [creature("Fallen Giant", 1, &[Color::Black], 5, 5)],
        );
    for seat in 2..seats {
        let guard = creature(&format!("Guard {seat}"), 1, &[Color::Red], 2, 2);
        scenario = scenario.battlefield(seat, [ready(guard)]);
    }
    scenario
}

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

/// The card named `name` in the zone.
fn in_zone(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| driver.state().object(**id).unwrap().card().name == name)
        .unwrap()
}

/// Every creature on the battlefield.
fn creatures(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            characteristics(driver.state(), *id)
                .is_some_and(|chars| chars.types.contains(CardType::Creature))
        })
        .collect()
}

fn is_legendary(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .unwrap()
        .card()
        .supertypes
        .contains(&Supertype::Legendary)
}

fn stats(driver: &Driver, id: ObjectId) -> (Option<i32>, Option<i32>) {
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power, chars.toughness)
}

/// Casts `spell` at the permanent `target` with `taps` Islands and lets it resolve.
fn cast_on(driver: &mut Driver, spell: &str, target: &str, taps: usize) -> Result<(), ScriptError> {
    let target = driver.target_named(target)?;
    driver.begin_cast(spell)?;
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&vec!["Island"; taps], 0)?;
    settle(driver)
}

/// Passes until a decision about what to copy is asked and returns what it offers.
fn asked_what_to_copy(driver: &mut Driver) -> Result<Vec<ObjectId>, ScriptError> {
    for _ in 0..50 {
        if let Some(DecisionKind::ChooseCards(options)) =
            driver.engine().decision().map(|d| d.kind.clone())
        {
            assert_eq!(driver.decision()?.player, pid(0));
            return Ok(options.from);
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the copy".into()))
}

#[rstest]
#[case::mirrorweave("Mirrorweave", "Own Bear", false)]
#[case::nanogene_conversion("Nanogene Conversion", "Own Legend", true)]
fn every_other_creature_becomes_a_copy_until_the_turn_ends(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] target: &str,
    #[case] not_legendary: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, &[spell]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &[spell]))?;
    let before = creatures(&driver);
    let chosen = driver.id_of(target)?;
    let names: Vec<String> = before
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    cast_on(&mut driver, spell, target, 4)?;
    let (power, toughness) = stats(&driver, chosen);
    for id in &before {
        assert_eq!(driver.state().object(*id).unwrap().card().name, target);
        assert_eq!(stats(&driver, *id), (power, toughness));
        // The chosen creature keeps what it is; the copies of a legend lose it when the words say so.
        let kept = *id == chosen && target == "Own Legend";
        assert_eq!(is_legendary(&driver, *id), kept || (!not_legendary && target == "Own Legend"));
    }
    assert_eq!(named(&driver, target).len(), before.len());
    replays(&driver, &setup);
    // The cleanup step ends every copy: each creature is what it was.
    driver.idle_until(2, Step::Upkeep)?;
    for (id, name) in before.iter().zip(&names) {
        assert_eq!(&driver.state().object(*id).unwrap().card().name, name);
        assert!(driver.state().object(*id).unwrap().copy_effect().is_none());
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn clone_legion_makes_a_token_copy_of_each_creature_the_target_player_controls(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Clone Legion"]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Clone Legion"]))?;
    let foe = driver.target_player(1);
    driver.begin_cast("Clone Legion")?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&vec!["Island"; 9], 0)?;
    settle(&mut driver)?;
    for name in ["Opposing Legend", "Opposing Guard"] {
        let all = named(&driver, name);
        assert_eq!(all.len(), 2, "{name} and its copy");
        let token = all
            .into_iter()
            .find(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
            .unwrap();
        assert_eq!(driver.state().object(token).unwrap().controller(), pid(0));
    }
    // Nothing of seat 0 or of the other seats is copied.
    assert_eq!(named(&driver, "Own Bear").len(), 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn rite_of_replication_makes_five_tokens_when_kicked_and_one_when_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] kicked: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Rite of Replication"]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Rite of Replication"]))?;
    let target = driver.target_named("Opposing Guard")?;
    driver.begin_cast("Rite of Replication")?;
    driver.expect_decision(0, "choose-costs")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(if kicked {
        vec![0]
    } else {
        vec![]
    })))?;
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&vec!["Island"; if kicked { 9 } else { 4 }], 0)?;
    settle(&mut driver)?;
    let tokens = named(&driver, "Opposing Guard")
        .into_iter()
        .filter(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
        .count();
    assert_eq!(tokens, if kicked { 5 } else { 1 });
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn body_double_may_enter_as_a_copy_of_a_creature_card_in_any_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] copies: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Body Double"]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Body Double"]))?;
    let giant = in_zone(&driver, Zone::Graveyard(pid(1)), "Fallen Giant");
    let body = in_zone(&driver, Zone::Hand(pid(0)), "Body Double");
    driver.cast_tapping("Body Double", &vec!["Island"; 5])?;
    let offered = asked_what_to_copy(&mut driver)?;
    // Only cards in graveyards: no permanent of the battlefield is offered.
    assert_eq!(offered, vec![giant]);
    driver.answer(Answer::ChooseCards(CardsChoice(if copies {
        vec![giant]
    } else {
        vec![]
    })))?;
    settle(&mut driver)?;
    if copies {
        let permanent = named(&driver, "Fallen Giant");
        assert_eq!(permanent.len(), 1);
        assert_eq!(stats(&driver, permanent[0]), (Some(5), Some(5)));
    } else {
        // Body Double is a 0/0 that copies nothing: it dies.
        assert!(named(&driver, "Fallen Giant").is_empty());
        assert!(named(&driver, "Body Double").is_empty());
        assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), "Body Double"), body);
    }
    // The card stays in the graveyard either way.
    assert!(driver.state().zone(Zone::Graveyard(pid(1))).contains(&giant));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn vesuva_enters_tapped_as_a_copy_of_a_land_and_untapped_when_it_copies_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] copies: bool,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Vesuva"]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Vesuva"]))?;
    let island = driver.id_of("Island")?;
    let vesuva = in_zone(&driver, Zone::Hand(pid(0)), "Vesuva");
    driver.play_land("Vesuva")?;
    let offered = asked_what_to_copy(&mut driver)?;
    assert!(offered.contains(&island));
    assert!(
        offered
            .iter()
            .all(|id| driver.state().object(*id).unwrap().card().name != "Own Bear"),
        "lands only"
    );
    driver.answer(Answer::ChooseCards(CardsChoice(if copies {
        vec![island]
    } else {
        vec![]
    })))?;
    settle(&mut driver)?;
    let object = driver.state().object(vesuva).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(object.copy_effect().is_some(), copies);
    assert_eq!(object.card().name, if copies { "Island" } else { "Vesuva" });
    assert_eq!(object.permanent().unwrap().tapped(), copies);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn cursed_mirror_is_a_hasty_copy_of_a_creature_until_the_turn_ends(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Cursed Mirror"]).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Cursed Mirror"]))?;
    let legend = driver.id_of("Opposing Legend")?;
    driver.cast_tapping("Cursed Mirror", &["Mountain", "Mountain", "Mountain"])?;
    let offered = asked_what_to_copy(&mut driver)?;
    assert!(offered.contains(&legend));
    driver.answer(Answer::ChooseCards(CardsChoice(vec![legend])))?;
    settle(&mut driver)?;
    let copy = *named(&driver, "Opposing Legend")
        .iter()
        .find(|id| **id != legend)
        .unwrap();
    assert!(
        keywords_of(driver.state(), copy)
            .iter()
            .any(|keyword| keyword.name() == "Haste")
    );
    assert!(characteristics(driver.state(), copy).unwrap().types.contains(CardType::Creature));
    assert!(!characteristics(driver.state(), copy).unwrap().types.contains(CardType::Artifact));
    replays(&driver, &setup);
    // The cleanup step ends it: the artifact is Cursed Mirror again.
    driver.idle_until(2, Step::Upkeep)?;
    let object = driver.state().object(copy).unwrap();
    assert_eq!(object.card().name, "Cursed Mirror");
    assert!(object.copy_effect().is_none());
    assert!(characteristics(driver.state(), copy).unwrap().types.contains(CardType::Artifact));
    replays(&driver, &setup);
    Ok(())
}
