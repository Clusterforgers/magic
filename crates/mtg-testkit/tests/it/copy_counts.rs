//! Goal: the tokens a copy makes in a number the game counts (Rionya, Fire Dancer: one plus the instants and sorceries cast this turn, with haste, exiled at the end step; Living Laser: one for each card discarded this turn, none when nothing was discarded, entering tapped and attacking and not legendary, exiled at the end step) and populate that enters tapped and attacking (Ghired, Conclave Exile), at tables of two, three and four seats, replayed from the log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{lands, replays, settle};

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
fn tokens_named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    named(driver, name)
        .into_iter()
        .filter(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
        .collect()
}

/// Seat 0 holds `hand` over six Mountains and controls `permanents`; every other seat controls a guard; the library of
/// seat 0 has cards to draw.
fn table(seats: usize, hand: &[&str], permanents: &[&str]) -> Scenario {
    let mut ours: Vec<_> = (0..6).map(|_| ready(mountain())).collect();
    ours.extend(permanents.iter().map(|name| {
        if *name == "Ally" {
            ready(creature("Ally", 1, &[Color::Red], 2, 2))
        } else {
            ready(card(name).unwrap())
        }
    }));
    let mut scenario = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, ours)
        .library(0, (0..12).map(|_| mountain()));
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(creature("Guard", 1, &[Color::Red], 6, 6))])
            .life(seat, 100);
    }
    scenario
}

/// Casts `spell` aimed at `target` (none for a spell with no target) and lets it resolve.
fn cast(driver: &mut Driver, spell: &str, target: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_cast(spell)?;
    if let Some(target) = target {
        driver.choose_targets(&[target])?;
    }
    driver.pay_tapping(&["Mountain"], 0)?;
    settle(driver)
}

/// Answers the target a pending trigger asks for with the permanent named `name`.
fn aim(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let target = driver.target_named(name)?;
        driver.choose_targets(&[target])?;
    }
    settle(driver)
}

/// Whether the permanent has haste.
fn hasty(driver: &Driver, id: ObjectId) -> bool {
    keywords_of(driver.state(), id)
        .iter()
        .any(|keyword| format!("{keyword:?}").contains("Haste"))
}

#[rstest]
fn rionya_makes_one_token_and_one_more_for_each_instant_or_sorcery_cast_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] spells: usize,
) -> Result<(), ScriptError> {
    let hand: Vec<&str> = vec!["Lightning Bolt"; spells];
    let permanents = ["Rionya, Fire Dancer", "Ally"];
    let setup = table(seats, &hand, &permanents).setup()?;
    let mut driver = Driver::from_scenario(table(seats, &hand, &permanents))?;
    for _ in 0..spells {
        let player = driver.target_player(1);
        cast(&mut driver, "Lightning Bolt", Some(player))?;
    }
    driver.pass_until(Step::BeginCombat)?;
    aim(&mut driver, "Ally")?;
    let copies = tokens_named(&driver, "Ally");
    assert_eq!(
        copies.len(),
        1 + spells,
        "one plus the {spells} spells cast"
    );
    assert!(copies.iter().all(|id| hasty(&driver, *id)), "with haste");
    assert_eq!(named(&driver, "Ally").len(), copies.len() + 1);
    replays(&driver, &setup);
    let turn = driver.state().turn().number();
    driver.idle_until(turn + 1, Step::Upkeep)?;
    assert!(
        tokens_named(&driver, "Ally").is_empty(),
        "exiled at the end step"
    );
    assert_eq!(named(&driver, "Ally").len(), 1);
    Ok(())
}

#[rstest]
fn rionya_does_not_count_a_creature_spell(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let hand = ["Lightning Bolt", "Monastery Swiftspear"];
    let permanents = ["Rionya, Fire Dancer", "Ally"];
    let mut driver = Driver::from_scenario(table(seats, &hand, &permanents))?;
    let player = driver.target_player(1);
    cast(&mut driver, "Lightning Bolt", Some(player))?;
    cast(&mut driver, "Monastery Swiftspear", None)?;
    driver.pass_until(Step::BeginCombat)?;
    aim(&mut driver, "Ally")?;
    assert_eq!(tokens_named(&driver, "Ally").len(), 2, "one plus the bolt");
    Ok(())
}

#[rstest]
fn living_laser_makes_a_token_for_each_card_discarded_that_enters_attacking_and_is_exiled_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] loots: usize,
) -> Result<(), ScriptError> {
    // The second loot is the flashback of the first.
    let hand: Vec<&str> = vec!["Faithless Looting"; usize::from(loots > 0)];
    let setup = table(seats, &hand, &["Living Laser"]).setup()?;
    let mut driver = Driver::from_scenario(table(seats, &hand, &["Living Laser"]))?;
    for loot in 0..loots {
        driver.begin_cast("Faithless Looting")?;
        let lands = if loot == 0 {
            vec!["Mountain"]
        } else {
            vec!["Mountain"; 3]
        };
        driver.pay_tapping(&lands, 0)?;
        settle(&mut driver)?;
    }
    let discarded = 2 * loots;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Living Laser", 1)])?;
    settle(&mut driver)?;
    let copies = tokens_named(&driver, "Living Laser");
    assert_eq!(
        copies.len(),
        discarded,
        "one for each of the {discarded} discards"
    );
    for id in &copies {
        let object = driver.state().object(*id).unwrap();
        assert!(object.permanent().unwrap().tapped(), "tapped");
        assert!(
            !object
                .card()
                .supertypes
                .contains(&mtg_core::card::Supertype::Legendary),
            "not legendary"
        );
    }
    assert_eq!(
        driver.state().combat().attackers().len(),
        1 + discarded,
        "the copies attack with the Laser"
    );
    replays(&driver, &setup);
    let turn = driver.state().turn().number();
    driver.idle_until(turn + 1, Step::Upkeep)?;
    assert!(tokens_named(&driver, "Living Laser").is_empty());
    assert_eq!(named(&driver, "Living Laser").len(), 1);
    Ok(())
}

#[rstest]
fn ghired_populates_a_token_that_enters_tapped_and_attacking(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        let mut ours = lands(3, 0);
        ours.push(ready(card("Ghired, Conclave Exile").unwrap()));
        ours.push(ready(creature("Friend", 1, &[Color::Red], 2, 2)));
        Scenario::new(seats)
            .hand_named(0, ["Cackling Counterpart"])
            .battlefield(0, ours)
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    // A creature token to populate.
    driver.begin_cast("Cackling Counterpart")?;
    let friend = driver.target_named("Friend")?;
    driver.choose_targets(&[friend])?;
    driver.pay_tapping(&["Island", "Island", "Island"], 0)?;
    settle(&mut driver)?;
    assert_eq!(tokens_named(&driver, "Friend").len(), 1);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ghired, Conclave Exile", 1)])?;
    settle(&mut driver)?;
    let tokens = tokens_named(&driver, "Friend");
    assert_eq!(tokens.len(), 2, "the token was copied");
    let attackers = driver.state().combat().attackers();
    assert_eq!(attackers.len(), 2, "Ghired and the new token attack");
    let new = tokens
        .iter()
        .find(|id| attackers.iter().any(|attacker| attacker.object() == **id))
        .expect("the copy attacks");
    assert!(
        driver
            .state()
            .object(*new)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    replays(&driver, &setup);
    Ok(())
}
