//! Goal: control gained "for as long as you control ~ and ~ remains tapped" (CR 611.2b) holds while the source stays
//! tapped under its controller, ends when the source untaps or another player gains control of it, and "for as long as
//! ~ remains tapped" alone survives that change of control, at tables of two to four seats; the log replays to the same
//! state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::{CombatQuirk, GameRule};
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::Answer;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature that may stay tapped and, for {T}, gains control of target creature for the given duration.
fn soulsinger(duration: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_value(serde_json::json!({"Activated": {
        "cost": ["Tap"],
        "targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Continuous": {
            "affected": {"Target": 0},
            "mods": [{"SetController": "You"}],
            "duration": duration}}]}}))
    .unwrap();
    Arc::new(
        CardDefinition::new("Soulsinger")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 3)
            .with_rules(ability)
            .with_rules(Ability::Rule {
                rule: GameRule::Combat {
                    who: Ref::This,
                    quirk: CombatQuirk::MayStayTapped,
                },
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

fn controller(driver: &Driver, name: &str) -> Option<PlayerId> {
    mtg_engine::characteristics::controller_of(driver.state(), driver.id_of(name).ok()?)
}

/// What happens after the Bear is taken.
#[derive(Debug, Clone, Copy)]
enum After {
    /// Seat 0 keeps the source tapped through its next untap step.
    KeptTapped,
    /// The source untaps in seat 0's next untap step.
    Untaps,
    /// Seat 1 gains control of the source, which stays tapped.
    SourceTaken,
}

#[rstest]
fn control_lasts_while_the_source_stays_tapped_under_you(
    #[values(2, 3, 4)] seats: usize,
    #[values(After::KeptTapped, After::Untaps, After::SourceTaken)] after: After,
    #[values("WhileYouControlSourceTapped", "WhileSourceTapped")] duration: &str,
) -> Result<(), ScriptError> {
    let game = || {
        let mut scenario = Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(soulsinger(duration))])
            .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
        for seat in 0..seats {
            scenario = scenario.library(seat, (0..20).map(|_| island()));
        }
        scenario
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    driver.begin_activation("Soulsinger")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    for _ in 0..20 {
        if !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
    }
    assert_eq!(controller(&driver, "Bear"), Some(PlayerId(0)));
    let next = 2 + u32::try_from(seats).unwrap();
    let both = duration == "WhileYouControlSourceTapped";
    let expected = match after {
        After::KeptTapped => {
            driver.idle_until(next, Step::Untap)?;
            let source = driver.id_of("Soulsinger")?;
            driver.answer(Answer::ChooseCards(CardsChoice(vec![source])))?;
            driver.idle_until(next, Step::Upkeep)?;
            PlayerId(0)
        }
        After::Untaps => {
            driver.idle_until(next, Step::Upkeep)?;
            PlayerId(1)
        }
        After::SourceTaken => {
            let source = driver.id_of("Soulsinger")?;
            driver.add_effect(
                driver.on(source),
                Modification::SetController(PlayerId(1)),
                Duration::Forever,
            )?;
            if both { PlayerId(1) } else { PlayerId(0) }
        }
    };
    assert_eq!(controller(&driver, "Bear"), Some(expected), "{after:?}");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
