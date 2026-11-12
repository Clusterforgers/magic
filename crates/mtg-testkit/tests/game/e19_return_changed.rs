//! Goal: a creature card put onto the battlefield as a changed creature ("It's a green Mutant with base power and
//! toughness 3/3", "It's a Spirit Detective") gets its colors, creature types and base power and toughness from a
//! continuous effect while it stays there: its other card types stay, counters still count, and once it leaves and
//! returns it is the printed card again, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::Target;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// "{T}: Put target creature card in a graveyard onto the battlefield under your control" with these modifications of
/// the creature for as long as it is there.
fn reviver(mods: &str) -> Arc<CardDefinition> {
    let rules = format!(
        r#"{{"Activated":{{"cost":["Tap"],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"In":"Graveyard"}}]}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":{{"Battlefield":{{"under":"You"}}}},"as":"moved"}}}},{{"Continuous":{{"affected":{{"Bound":"moved"}},"mods":{mods},"duration":"Forever"}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Reviver")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&rules).unwrap()),
    )
}

/// A red Elf Warrior artifact creature, 1/2.
fn elf() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Elf")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_subtype("Warrior")
            .with_mana_cost(mtg_testkit::cards::cost(0, &[Color::Red]))
            .with_power_toughness(1, 2),
    )
}

/// What the object is: its creature types among those named, its colors, power and toughness, and whether it is still
/// an artifact.
#[derive(Debug, PartialEq, Eq)]
struct Seen {
    types: Vec<&'static str>,
    colors: Vec<Color>,
    pt: (i32, i32),
    artifact: bool,
}

const NAMES: [&str; 5] = ["Elf", "Warrior", "Mutant", "Spirit", "Detective"];

fn seen(driver: &Driver, id: ObjectId) -> Seen {
    let state = driver.state();
    let chars = characteristics(state, id).unwrap();
    let printed = &state.object(id).unwrap().card().subtypes;
    Seen {
        types: NAMES
            .into_iter()
            .filter(|name| chars.has_subtype(printed, &Subtype((*name).to_owned())))
            .collect(),
        colors: Color::ALL
            .into_iter()
            .filter(|color| chars.colors.contains(*color))
            .collect(),
        pt: (chars.power.unwrap(), chars.toughness.unwrap()),
        artifact: chars.types.contains(CardType::Artifact),
    }
}

fn the_elf(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Elf")
        .unwrap()
}

/// Seat 0 returns seat `from`'s Elf with the Reviver at a table of `seats`.
fn returned(seats: usize, from: usize, mods: &str) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(reviver(mods))])
        .graveyard(from, [elf()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let card = driver
        .state()
        .zone(Zone::Graveyard(pid(from)))
        .first()
        .copied()
        .unwrap();
    let target = Target::Object(driver.state().object(card).unwrap().reference());
    driver.begin_activation("Reviver")?;
    driver.choose_targets(&[target])?;
    for _ in 0..10 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(driver)
}

const MUTANT: &str = r#"[{"SetPT":{"power":3,"toughness":3}},{"SetCreatureTypes":["Mutant"]},{"SetColors":["Green"]}]"#;
const SPIRIT: &str = r#"[{"SetCreatureTypes":["Spirit","Detective"]}]"#;

#[rstest]
#[case::green_mutant(MUTANT, vec!["Mutant"], vec![Color::Green], (3, 3))]
#[case::spirit_detective(SPIRIT, vec!["Spirit", "Detective"], vec![Color::Red], (1, 2))]
fn the_returned_creature_is_changed_while_it_stays(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] from: usize,
    #[case] mods: &str,
    #[case] types: Vec<&'static str>,
    #[case] colors: Vec<Color>,
    #[case] pt: (i32, i32),
) {
    let mut driver = returned(seats, from, mods).unwrap();
    let elf = the_elf(&driver);
    assert_eq!(driver.state().object(elf).unwrap().controller(), pid(0));
    let changed = Seen {
        types,
        colors,
        pt,
        artifact: true,
    };
    assert_eq!(seen(&driver, elf), changed);
    driver
        .manual(ManualAction::Counters {
            object: elf,
            counter: CounterKind::PlusOnePlusOne,
            delta: 1,
        })
        .unwrap();
    assert_eq!(
        seen(&driver, elf).pt,
        (pt.0 + 1, pt.1 + 1),
        "counters apply after the base"
    );
    driver
        .manual(ManualAction::Counters {
            object: elf,
            counter: CounterKind::PlusOnePlusOne,
            delta: -1,
        })
        .unwrap();
    driver
        .manual(ManualAction::Move {
            object: elf,
            to: ManualZone::Hand,
        })
        .unwrap();
    let card = driver.state().zone(Zone::Hand(pid(from)))[0];
    driver
        .manual_by(
            pid(from),
            &ManualAction::Move {
                object: card,
                to: ManualZone::Battlefield,
            },
        )
        .unwrap();
    let again = the_elf(&driver);
    let printed = Seen {
        types: vec!["Elf", "Warrior"],
        colors: vec![Color::Red],
        pt: (1, 2),
        artifact: true,
    };
    assert_eq!(seen(&driver, again), printed, "a new object keeps nothing");
}
