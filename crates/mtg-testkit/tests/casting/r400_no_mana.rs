//! Goal: a spell that says "You can't spend mana to cast this spell." plays as printed at tables of two, three and four
//! seats: Hogaak, Arisen Necropolis is not castable with lands alone however many there are, is cast by convoking green
//! creatures and delving graveyard cards with every land left untapped, and is not castable when convoke and delve
//! together fall short; the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::PresetPermanent;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::pay;

fn r400_offered(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    }))
}

/// Seat 0 holds Hogaak in hand, `forests` untapped Forests, `elves` green creatures and `cards` cards in its graveyard.
#[rstest]
#[case::lands_alone(12, 0, 0, false)]
#[case::convoke_and_delve(9, 2, 5, true)]
#[case::one_short(9, 2, 4, false)]
fn r400_hogaak_is_cast_without_mana(
    #[values(2, 3, 4)] seats: usize,
    #[case] forests: usize,
    #[case] elves: usize,
    #[case] cards: usize,
    #[case] open: bool,
) -> Result<(), ScriptError> {
    let mut permanents: Vec<PresetPermanent> = (0..forests).map(|_| ready(forest())).collect();
    permanents.extend((0..elves).map(|_| {
        ready(std::sync::Arc::new(
            mtg_cards::CardDefinition::new("Green Elf")
                .with_type(mtg_core::card::CardType::Creature)
                .with_mana_cost(mtg_core::mana::ManaCost::parse("{G}").unwrap())
                .with_power_toughness(1, 1),
        ))
    }));
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, permanents)
        .hand_named(0, ["Hogaak, Arisen Necropolis"])
        .graveyard_named(0, vec!["Forest"; cards]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(r400_offered(&driver, "Hogaak, Arisen Necropolis")?, open);
    if !open {
        return Ok(());
    }
    driver.begin_cast("Hogaak, Arisen Necropolis")?;
    for _ in 0..20 {
        let landed = driver.state().zone(Zone::Battlefield).iter().any(|id| {
            driver.state().object(*id).unwrap().card().name == "Hogaak, Arisen Necropolis"
        });
        if landed {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::PayMana(options) => {
                assert!(options.sources.is_empty(), "no mana ability is offered");
                pay(&mut driver)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let state = driver.state();
    let tapped_forests = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = state.object(**id).unwrap();
            object.card().name == "Forest" && object.permanent().unwrap().tapped()
        })
        .count();
    assert_eq!(tapped_forests, 0, "no land is tapped for mana");
    assert_eq!(state.zone(Zone::Graveyard(crate::common::pid(0))).len(), 0);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
