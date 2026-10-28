//! Goal: replacements that read the event they replace and the choice to tap or untap play whole at tables of two, three and four seats with the other permanent on the next seat or the last one: Plague Drone turns an opponent's life gain into that much life lost (the controller still gains), Pariah sends damage dealt to its controller to the enchanted creature from the same source, Ravenous Slime exiles an opponent's creature that would die and grows by its power, Szadek mills the player it would hurt and grows instead, and Tidal Force taps or untaps its target as its controller picks; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{OptionChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, cost, gain_life, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::{attached_to, tapped};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for `{W}`: "You gain `amount` life."
fn heal(amount: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Heal")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(amount)],
            }),
    )
}

/// The seat that holds the other permanent: the next one or the last one.
const fn holder(seats: usize, next_seat: bool) -> usize {
    if next_seat { 1 } else { seats - 1 }
}

/// Passes until a player has priority over an empty stack: `target` answers each choice of targets, `yes` each yes or
/// no and `option` each choice of an option.
fn settle(driver: &mut Driver, target: Option<&Target>, option: usize) -> Result<(), ScriptError> {
    for _ in 0..300 {
        let decision = driver.engine().decision().map(|d| d.kind.clone());
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        match decision {
            Some(DecisionKind::Priority(_)) if idle => return Ok(()),
            Some(DecisionKind::Priority(_)) => {
                driver.pass()?;
            }
            Some(DecisionKind::ChooseTargets(_)) if target.is_some() => {
                driver
                    .choose_targets(target.into_iter().cloned().collect::<Vec<_>>().as_slice())?;
            }
            Some(DecisionKind::YesNo(_)) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            Some(DecisionKind::ChooseOption(_)) => {
                driver.answer(Answer::ChooseOption(OptionChoice(option)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn object(driver: &Driver, name: &str) -> Target {
    let id = driver.id_of(name).unwrap();
    Target::Object(driver.state().object(id).unwrap().reference())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn replays(setup: &Scenario, driver: &Driver) {
    let setup = setup.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn plague_drone_turns_an_opponents_life_gain_into_a_loss(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
    #[values(true, false)] opposed: bool,
    #[values(1, 4)] amount: i32,
) -> Result<(), ScriptError> {
    let drone = || ready(card("Plague Drone").unwrap());
    let mut mine = vec![ready(plains())];
    if !opposed {
        mine.push(drone());
    }
    let mut game = Scenario::new(seats)
        .hand(0, [heal(amount)])
        .battlefield(0, mine);
    if opposed {
        game = game.battlefield(holder(seats, next_seat), [drone()]);
    }
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    driver.cast("Heal")?;
    settle(&mut driver, None, 0)?;
    let expected = if opposed { 20 - amount } else { 20 + amount };
    assert_eq!(life(&driver, 0), expected);
    replays(&game, &driver);
    Ok(())
}

#[rstest]
fn pariah_sends_the_damage_to_the_enchanted_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
    #[values(1, 3)] amount: i32,
) -> Result<(), ScriptError> {
    let seat = holder(seats, next_seat);
    let pariah = attached_to(ready(card("Pariah").unwrap()), seat, 0);
    let game = Scenario::new(seats)
        .hand(0, [burn("Bolt", amount)])
        .battlefield(0, [ready(mountain())])
        .battlefield(seat, [ready(card("Grizzly Bears").unwrap()), pariah]);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    driver.begin_cast("Bolt")?;
    driver.choose_targets(&[Target::Player(pid(seat))])?;
    driver.pay_tapping(&["Mountain"], 0)?;
    settle(&mut driver, None, 0)?;
    assert_eq!(life(&driver, seat), 20, "the player is dealt nothing");
    let dead = names(driver.state(), Zone::Graveyard(pid(seat)));
    assert_eq!(dead.contains(&"Grizzly Bears".to_owned()), amount >= 2);
    replays(&game, &driver);
    Ok(())
}

#[rstest]
fn ravenous_slime_exiles_the_creature_and_grows_by_its_power(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
) -> Result<(), ScriptError> {
    let seat = holder(seats, next_seat);
    let game = Scenario::new(seats)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(
            0,
            [ready(mountain()), ready(card("Ravenous Slime").unwrap())],
        )
        .battlefield_named(seat, ["Grizzly Bears"]);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    let bears = object(&driver, "Grizzly Bears");
    driver.begin_cast("Bolt")?;
    driver.choose_targets(&[bears])?;
    driver.pay_tapping(&["Mountain"], 0)?;
    settle(&mut driver, None, 0)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Grizzly Bears"]);
    let slime = driver.id_of("Ravenous Slime").unwrap();
    let slime = driver.state().object(slime).unwrap();
    assert_eq!(
        slime
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        2
    );
    replays(&game, &driver);
    Ok(())
}

#[rstest]
fn szadek_mills_and_grows_in_place_of_combat_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
) -> Result<(), ScriptError> {
    let seat = holder(seats, next_seat);
    let game = Scenario::new(seats)
        .battlefield_named(0, ["Szadek, Lord of Secrets"])
        .library_named(seat, ["Grizzly Bears"; 8]);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Szadek, Lord of Secrets", seat)])?;
    driver.idle_until(1, Step::EndCombat)?;
    assert_eq!(life(&driver, seat), 20);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(seat))).len(), 5);
    let szadek = driver.id_of("Szadek, Lord of Secrets").unwrap();
    let szadek = driver.state().object(szadek).unwrap();
    assert_eq!(
        szadek
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        5
    );
    replays(&game, &driver);
    Ok(())
}

#[rstest]
fn tidal_force_taps_or_untaps_its_target_as_its_controller_picks(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
    #[values(true, false)] was_tapped: bool,
    #[values(0, 1)] option: usize,
) -> Result<(), ScriptError> {
    let seat = holder(seats, next_seat);
    let bears = card("Grizzly Bears").unwrap();
    let bears = if was_tapped {
        tapped(bears)
    } else {
        ready(bears)
    };
    let game = Scenario::new(seats)
        .battlefield_named(0, ["Tidal Force"])
        .battlefield(seat, [bears])
        .at(1, 0, Step::Untap);
    let mut driver = Driver::from_scenario(game.clone())?;
    let target = object(&driver, "Grizzly Bears");
    driver.idle_until(1, Step::Upkeep)?;
    settle(&mut driver, Some(&target), option)?;
    let id = driver.id_of("Grizzly Bears").unwrap();
    let now = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(now, option == 0, "tap is the first option");
    replays(&game, &driver);
    Ok(())
}
