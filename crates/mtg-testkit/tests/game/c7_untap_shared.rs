//! Goal: on a shared team turn (Two-Headed Giant and shared turns, CR 805.3) one "doesn't untap during its controller's next untap step" effect that holds down permanents of several players of the team keeps them all tapped through that one untap step and expires once (Permafrost Trap on two teammates' creatures), at four and six seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::modify::Duration;
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{elf, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Two teams of `size` sharing turns under `mode`; seat 0 holds Permafrost Trap with four Islands, and the first and last
/// seat of the other team each control a creature.
fn game(mode: TeamMode, size: usize) -> Scenario {
    let teams = Teams::even(mode, 2, u32::try_from(size).unwrap());
    Scenario::new(size * 2)
        .house_rules(HouseRules {
            teams: Some(teams),
            ..HouseRules::default()
        })
        .hand_named(0, ["Permafrost Trap"])
        .battlefield(0, (0..4).map(|_| ready(island())))
        .battlefield(size, [ready(card("Grizzly Bears").unwrap())])
        .battlefield(size * 2 - 1, [ready(elf())])
}

/// Whether the named creature is tapped.
fn tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .unwrap()
        .tapped()
}

/// The effects that last through a next untap step.
fn held(driver: &Driver) -> usize {
    driver
        .state()
        .effects()
        .iter()
        .filter(|effect| effect.duration == Duration::NextUntapStep)
        .count()
}

#[rstest]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, 2)]
#[case::shared_four(TeamMode::SharedTurns, 2)]
#[case::shared_six(TeamMode::SharedTurns, 3)]
fn one_effect_on_two_teammates_ends_once_with_their_shared_untap_step(
    #[case] mode: TeamMode,
    #[case] size: usize,
) -> Result<(), ScriptError> {
    let setup = game(mode, size).setup().unwrap();
    let mut driver = Driver::from_scenario(game(mode, size))?;
    driver.pass_until(Step::Main1)?;
    let bears = driver.target_named("Grizzly Bears")?;
    let runeclaw = driver.target_named("Llanowar Elf")?;
    driver.cast_at("Permafrost Trap", &[bears, runeclaw])?;
    driver.idle_until(1, Step::End)?;
    assert!(tapped(&driver, "Grizzly Bears") && tapped(&driver, "Llanowar Elf"));
    assert_eq!(held(&driver), 1, "one effect holds both creatures");
    driver.idle_until(2, Step::Upkeep)?;
    assert!(
        tapped(&driver, "Grizzly Bears") && tapped(&driver, "Llanowar Elf"),
        "the shared untap step keeps both tapped"
    );
    assert_eq!(held(&driver), 0, "that untap step used the effect up");
    driver.idle_until(4, Step::Upkeep)?;
    assert!(!tapped(&driver, "Grizzly Bears") && !tapped(&driver, "Llanowar Elf"));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
