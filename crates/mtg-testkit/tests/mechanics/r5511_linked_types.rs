//! Goal: types and stats a permanent takes from cards it is linked to: Duplicant has the power, toughness and creature
//! types of the creature card it exiled and is still a Shapeshifter, and is its printed self when it exiled nothing;
//! Illusionary Terrain makes basic lands of the first type its controller chose the second type (every player's, with
//! the mana ability of that type and not of the old one); at 2 to 4 seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::Subtype;
use mtg_core::mana::ManaType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A 6/4 Wurm.
fn wurm() -> std::sync::Arc<mtg_cards::CardDefinition> {
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Big Wurm")
            .with_type(mtg_core::card::CardType::Creature)
            .with_subtype("Wurm")
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{4}{G}{G}").unwrap())
            .with_power_toughness(6, 4),
    )
}

/// Seat 0 casts Duplicant with six Plains; seat 1 has `victim`.
fn duplicant_table(seats: usize, victim: &str) -> Scenario {
    let victim = if victim == "Big Wurm" {
        wurm()
    } else {
        card(victim).unwrap()
    };
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![card("Duplicant").unwrap()])
        .battlefield(0, (0..6).map(|_| ready(plains())))
        .battlefield(1, [ready(victim)])
}

#[rstest]
#[case::wurm("Big Wurm", true, (6, 4), "Wurm")]
#[case::bears("Grizzly Bears", true, (2, 2), "Bear")]
#[case::declined("Grizzly Bears", false, (2, 4), "")]
fn r5511_duplicant_takes_the_exiled_creatures_stats_and_types(
    #[case] victim: &str,
    #[case] exile: bool,
    #[case] stats: (i32, i32),
    #[case] kind: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(duplicant_table(seats, victim))?;
    driver.cast("Duplicant")?;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_named(victim)?;
                driver.choose_targets(&[target])?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(exile)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let chars = characteristics(driver.state(), driver.id_of("Duplicant")?).unwrap();
    assert_eq!((chars.power.unwrap(), chars.toughness.unwrap()), stats);
    let printed = &driver
        .state()
        .object(driver.id_of("Duplicant")?)
        .unwrap()
        .card()
        .subtypes
        .clone();
    assert!(chars.has_subtype(printed, &Subtype("Shapeshifter".into())));
    if !kind.is_empty() {
        assert!(chars.has_subtype(printed, &Subtype(kind.into())));
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::islands_are_swamps("Island, then Swamp", "Island", ManaType::Black)]
#[case::forests_are_plains("Forest, then Plains", "Forest", ManaType::White)]
fn r5511_illusionary_terrain_makes_the_first_type_the_second(
    #[case] chosen: &str,
    #[case] land: &str,
    #[case] makes: ManaType,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, vec![card("Illusionary Terrain").unwrap()])
            .battlefield(
                0,
                [
                    ready(island()),
                    ready(island()),
                    ready(island()),
                    ready(forest()),
                    ready(swamp()),
                ],
            )
            .battlefield(1, [ready(island()), ready(forest())]),
    )?;
    driver.cast_tapping("Illusionary Terrain", &["Island", "Island"])?;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| {
                        matches!(option, mtg_engine::choose::ChoiceOption::Label(label) if label == chosen)
                    })
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let theirs = driver
        .state()
        .objects()
        .find(|object| {
            object.card().name == land && object.controller() == mtg_core::ids::PlayerId(1)
        })
        .map(mtg_engine::object::GameObject::id)
        .unwrap();
    let chars = characteristics(driver.state(), theirs).unwrap();
    assert!(!chars.has_subtype(&[], &Subtype(land.into())));
    assert!(
        driver.tap_for_mana_of(land, makes).is_ok(),
        "our {land} makes the mana of its new type"
    );
    assert_replays(&driver);
    Ok(())
}
