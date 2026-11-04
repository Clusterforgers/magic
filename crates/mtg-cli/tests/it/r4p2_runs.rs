//! Goal: a translated counted repeat and a die read as a number play right: the loop ends after its count and the number read is the one rolled.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::events::Event;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::r3p3_runs::{settle, translated};

fn gained(driver: &Driver) -> Vec<i32> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::LifeGained { amount, .. } => Some(i32::try_from(*amount).unwrap()),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::three(3, vec![2, 2, 2])]
#[case::one(1, vec![2])]
fn a_counted_repeat_ends_after_its_count(#[case] count: u32, #[case] want: Vec<i32>) {
    let script = format!(
        "Name:Echo\nTypes:Sorcery\nA:SP$ Repeat | MaxRepeat$ {count} | RepeatSubAbility$ B\nSVar:B:DB$ GainLife | LifeAmount$ 2\n"
    );
    let card = translated(&script, "Echo", "Sorcery", "{0}");
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [card])).unwrap();
    driver.cast("Echo").unwrap();
    settle(&mut driver);
    assert_eq!(gained(&driver), want);
}

const KITTEN: &str = "Name:Kitten\nTypes:Creature Cat\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Roll | TriggerDescription$ When this enters, roll a six-sided die. You gain life equal to the result.\nSVar:Roll:DB$ RollDice | ResultSVar$ Result | SubAbility$ Life\nSVar:Life:DB$ GainLife | LifeAmount$ Result\n";

#[rstest]
fn a_die_read_as_a_number_gains_that_much_life(#[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64) {
    let card = translated(KITTEN, "Kitten", "Creature — Cat", "{0}");
    let mut driver = Driver::from_scenario(Scenario::new(2).seed(seed).hand(0, [card])).unwrap();
    driver.cast("Kitten").unwrap();
    settle(&mut driver);
    let rolled: Vec<u32> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect();
    assert_eq!(rolled.len(), 1);
    assert_eq!(gained(&driver), vec![i32::try_from(rolled[0]).unwrap()]);
}

const SMITE: &str = "Name:Smite\nTypes:Sorcery\nA:SP$ ChooseCard | Choices$ Creature.YouCtrl | RememberChosen$ True | Mandatory$ True | SubAbility$ Hit\nSVar:Hit:DB$ Destroy | Defined$ Remembered | SubAbility$ Clean\nSVar:Clean:DB$ Cleanup | ClearRemembered$ True\n";

#[test]
fn the_remembered_choice_is_the_one_destroyed() {
    use mtg_core::ids::PlayerId;
    use mtg_engine::zones::Zone;
    use mtg_testkit::cards::bear;
    let card = translated(SMITE, "Smite", "Sorcery", "{0}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [mtg_testkit::ready(bear()), mtg_testkit::ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Smite").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    let state = driver.state();
    assert_eq!(state.zone(Zone::Graveyard(me)).len(), 2); // the bear and the spell
    assert_eq!(state.zone(Zone::Battlefield).len(), 1);
}
