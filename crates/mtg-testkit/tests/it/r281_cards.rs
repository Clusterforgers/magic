//! Goal: corpus cards of the r28-1 pack play as their rules say at two and four seats: Ersatz Gnomes makes a spell colorless; Marina Vendrell unlocks a locked door and locks an unlocked one of a Room; Victory Chimes gives its mana to the player its controller chooses; Rubble Rouser's mana sets off its damage to each opponent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use mtg_engine::choose::TargetChoice;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

#[rstest]
fn ersatz_gnomes_makes_a_spell_colorless(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Grizzly Bears"])
            .battlefield_named(0, ["Ersatz Gnomes", "Forest", "Forest"]),
    )?;
    driver.begin_cast("Grizzly Bears")?;
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        let answer = match driver.decision()?.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(mtg_engine::decision::PayChoice::Auto),
            _ => Driver::passive_answer(driver.decision()?),
        };
        driver.answer(answer)?;
    }
    let bears = *driver.state().stack().last().unwrap();
    assert!(
        !mtg_engine::characteristics(driver.state(), bears)
            .unwrap()
            .colors
            .is_empty()
    );
    let first = driver.offered_abilities("Ersatz Gnomes")?[0];
    driver.begin_ability("Ersatz Gnomes", first)?;
    let spell = driver.state().object(bears).unwrap().reference();
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
        mtg_engine::choose::Target::Object(spell),
    ])))?;
    // The Gnomes' ability resolves; the spell is still on the stack.
    driver.pass()?;
    for _ in 1..seats {
        driver.pass()?;
    }
    assert_eq!(driver.state().stack().last(), Some(&bears));
    assert!(
        mtg_engine::characteristics(driver.state(), bears)
            .unwrap()
            .colors
            .is_empty()
    );
    Ok(())
}

/// Activates the named card's only offered ability, answering each option with `pick` (by the words of the option) and
/// any other question passively, until the stack is empty again; the events from the activation on.
fn activate(
    driver: &mut Driver,
    name: &str,
    pick: &dyn Fn(&[ChoiceOption]) -> usize,
) -> Result<Vec<Event>, ScriptError> {
    let mut log = driver.begin_activation(name)?;
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        || !driver.state().stack().is_empty()
    {
        let answer = match &driver.decision()?.kind {
            DecisionKind::ChooseOption(options) => {
                Answer::ChooseOption(OptionChoice(pick(&options.options)))
            }
            DecisionKind::ChooseTargets(options) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![options.legal[0]]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(mtg_engine::decision::PayChoice::Auto),
            _ => Driver::passive_answer(driver.decision()?),
        };
        log.extend(driver.answer(answer)?);
    }
    Ok(log)
}

/// The option whose words start with `words`.
fn labelled(words: &'static str) -> impl Fn(&[ChoiceOption]) -> usize {
    move |options: &[ChoiceOption]| {
        options
            .iter()
            .position(|o| matches!(o, ChoiceOption::Label(l) if l.starts_with(words)))
            .unwrap()
    }
}

#[rstest]
fn marina_vendrell_unlocks_then_locks_a_door(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield_named(0, ["Marina Vendrell", "Glassworks // Shattered Yard"]),
    )?;
    // A Room with both doors locked has no name of its own.
    let room = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().dk1c5_doors(*id).is_some())
        .unwrap();
    assert_eq!(driver.state().dk1c5_doors(room), Some([false, false]));
    let log = activate(
        &mut driver,
        "Marina Vendrell",
        &labelled("Unlock Glassworks"),
    )?;
    assert!(
        log.iter()
            .any(|e| matches!(e, Event::DoorUnlocked { object, door: 0, .. } if *object == room))
    );
    assert_eq!(driver.state().dk1c5_doors(room), Some([true, false]));
    // In the same player's next turn, with Marina untapped again, it locks the door it unlocked.
    let turn = driver.state().turn().number() + u32::try_from(seats).unwrap();
    driver.idle_until(turn, mtg_engine::turn::Step::Main1)?;
    let log = activate(&mut driver, "Marina Vendrell", &labelled("Lock Glassworks"))?;
    assert!(
        log.iter()
            .any(|e| matches!(e, Event::DoorLocked { object, door: 0, .. } if *object == room))
    );
    assert_eq!(driver.state().dk1c5_doors(room), Some([false, false]));
    Ok(())
}

#[rstest]
fn victory_chimes_gives_its_mana_to_the_chosen_player(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).battlefield_named(0, ["Victory Chimes"]))?;
    let chosen = PlayerId(seats as u64 - 1);
    let log = activate(&mut driver, "Victory Chimes", &|options| {
        options
            .iter()
            .position(|o| *o == ChoiceOption::Player(chosen))
            .unwrap()
    })?;
    assert!(
        log.iter().any(|e| matches!(
            e,
            Event::ManaAdded { player, mana: ManaType::Colorless, .. } if *player == chosen
        )),
        "{log:?}"
    );
    Ok(())
}

#[rstest]
fn rubble_rousers_mana_deals_one_damage_to_each_opponent(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield_named(0, ["Rubble Rouser"])
            .graveyard_named(0, ["Forest"]),
    )?;
    activate(&mut driver, "Rubble Rouser", &|_| 0)?;
    assert_eq!(driver.state().zone(Zone::Graveyard(PlayerId(0))).len(), 0);
    // The reflexive trigger resolves once everyone passes.
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    for seat in 1..seats {
        assert_eq!(
            driver.state().player(PlayerId(seat as u64)).unwrap().life(),
            19
        );
    }
    Ok(())
}
