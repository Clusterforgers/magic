//! Goal: a permanent that its controller may choose not to untap (CR 502.3: the active player determines which permanents untap) is asked about in the untap step for every such tapped permanent whatever the cards in hand, the answer keeps the chosen ones tapped and untaps the rest at tables of two to four seats, a refused or empty answer untaps everything, an untapped such permanent or another seat's is never asked about, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::{CombatQuirk, GameRule};
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::PresetPermanent;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

/// A creature whose controller may choose not to untap it.
fn golem(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
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

/// Seat 0 at the beginning of its untap step with the permanents given, seat 0 holding `hand` cards.
fn table(
    seats: usize,
    hand: usize,
    permanents: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(u32::try_from(seats).unwrap() + 1, 0, Step::Untap)
        .hand(0, (0..hand).map(|_| bear()))
        .battlefield(0, permanents);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| island()));
    }
    Driver::from_scenario(scenario)
}

fn is_tapped(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

/// What the pending question offers: the seat asked, the permanents and the bounds.
fn question(driver: &Driver) -> Option<(PlayerId, Vec<ObjectId>, u32, u32)> {
    let decision = driver.decision().ok()?;
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        return None;
    };
    (driver.state().turn().step() == Step::Untap).then(|| {
        (
            decision.player,
            options.from.clone(),
            options.min,
            options.max,
        )
    })
}

#[rstest]
#[case::two_seats_empty_hand(2, 0)]
#[case::two_seats_full_hand(2, 7)]
#[case::three_seats(3, 3)]
#[case::four_seats(4, 1)]
fn every_tapped_permanent_that_may_stay_tapped_is_asked_about(
    #[case] seats: usize,
    #[case] hand: usize,
) -> Result<(), ScriptError> {
    let driver = table(
        seats,
        hand,
        vec![
            tapped(golem("Ice Golem")),
            tapped(golem("Frost Golem")),
            tapped(free_creature("Bear", 2, 2)),
            ready(golem("Awake Golem")),
        ],
    )?;
    let (ice, frost) = (driver.id_of("Ice Golem")?, driver.id_of("Frost Golem")?);
    let (player, mut from, min, max) = question(&driver).expect("a question");
    from.sort();
    assert_eq!(
        (player, from, min, max),
        (PlayerId::from_index(0), vec![ice, frost], 0, 2)
    );
    Ok(())
}

#[rstest]
#[case::keep_one(&["Ice Golem"])]
#[case::keep_both(&["Ice Golem", "Frost Golem"])]
#[case::keep_none(&[])]
fn the_permanents_chosen_stay_tapped_and_the_rest_untap(
    #[case] kept: &[&str],
) -> Result<(), ScriptError> {
    let game = || {
        Scenario::new(2)
            .at(3, 0, Step::Untap)
            .battlefield(
                0,
                vec![
                    tapped(golem("Ice Golem")),
                    tapped(golem("Frost Golem")),
                    tapped(free_creature("Bear", 2, 2)),
                ],
            )
            .library(0, (0..20).map(|_| island()))
            .library(1, (0..20).map(|_| island()))
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let chosen = kept
        .iter()
        .map(|name| driver.id_of(name))
        .collect::<Result<Vec<_>, _>>()?;
    driver.answer(Answer::ChooseCards(CardsChoice(chosen.clone())))?;
    assert!(question(&driver).is_none(), "asked a second time");
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    for name in ["Ice Golem", "Frost Golem"] {
        let id = driver.id_of(name)?;
        assert_eq!(is_tapped(&driver, id), chosen.contains(&id), "{name}");
    }
    assert!(!is_tapped(&driver, driver.id_of("Bear")?));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn a_permanent_that_was_not_offered_cannot_stay_tapped() -> Result<(), ScriptError> {
    let mut driver = table(
        2,
        0,
        vec![
            tapped(golem("Ice Golem")),
            tapped(free_creature("Bear", 2, 2)),
        ],
    )?;
    let bear = driver.id_of("Bear")?;
    let refused = driver.answer(Answer::ChooseCards(CardsChoice(vec![bear])));
    assert!(matches!(
        refused,
        Err(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::NotOffered(_)
        )))
    ));
    assert!(question(&driver).is_some(), "the question stands");
    driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
    assert!(!is_tapped(&driver, driver.id_of("Ice Golem")?));
    Ok(())
}

#[rstest]
#[case::untapped_only(vec![ready(golem("Ice Golem")), tapped(free_creature("Bear", 2, 2))])]
#[case::nothing_that_may_stay(vec![tapped(free_creature("Bear", 2, 2))])]
fn nothing_is_asked_when_no_such_permanent_would_untap(
    #[case] permanents: Vec<PresetPermanent>,
) -> Result<(), ScriptError> {
    let driver = table(2, 3, permanents)?;
    assert!(question(&driver).is_none());
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    assert!(!is_tapped(
        &driver,
        driver
            .id_of("Bear")
            .or_else(|_| driver.id_of("Ice Golem"))?
    ));
    Ok(())
}

#[test]
fn a_permanent_of_another_seat_is_not_asked_about() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .at(3, 0, Step::Untap)
        .battlefield(1, vec![tapped(golem("Ice Golem"))])
        .library(0, (0..20).map(|_| island()))
        .library(1, (0..20).map(|_| island()));
    let driver = Driver::from_scenario(scenario)?;
    assert!(question(&driver).is_none());
    assert!(is_tapped(&driver, driver.id_of("Ice Golem")?));
    Ok(())
}

/// An artifact whose ability, for {T}, gives each creature its controller has +2/+2 for as long as it remains tapped
/// (Ashnod's Battle Gear in the large); `keep` lets its controller choose not to untap it.
fn battery(keep: bool) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_value(serde_json::json!({"Activated": {
        "cost": ["Tap"],
        "effects": [{"Continuous": {
            "affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
            "mods": [{"ModifyPT": {"power": 2, "toughness": 2}}],
            "duration": "WhileSourceTapped"}}]}}))
    .unwrap();
    let mut card = CardDefinition::new("Battery")
        .with_type(CardType::Artifact)
        .with_rules(ability);
    if keep {
        card = card.with_rules(Ability::Rule {
            rule: GameRule::Combat {
                who: Ref::This,
                quirk: CombatQuirk::MayStayTapped,
            },
            condition: None,
            from: ZoneKind::Battlefield,
        });
    }
    Arc::new(card)
}

fn bear_power(driver: &Driver) -> Option<i32> {
    mtg_engine::characteristics::characteristics(driver.state(), driver.id_of("Bear").ok()?)?.power
}

/// The effect lasts while the source stays tapped (CR 611.2b): it ends when the source untaps in its controller's next
/// untap step, and holds for as long as the controller chooses to keep the source tapped.
#[rstest]
#[case::untaps(false, 2)]
#[case::kept_tapped(true, 4)]
fn an_effect_lasts_as_long_as_its_source_stays_tapped(
    #[case] keep: bool,
    #[case] power_after: i32,
) -> Result<(), ScriptError> {
    let game = || {
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .battlefield(
                0,
                [ready(battery(keep)), ready(free_creature("Bear", 2, 2))],
            )
            .library(0, (0..20).map(|_| island()))
            .library(1, (0..20).map(|_| island()))
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    driver.begin_activation("Battery")?;
    for _ in 0..20 {
        if !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
    }
    assert!(is_tapped(&driver, driver.id_of("Battery")?));
    assert_eq!(bear_power(&driver), Some(4));
    // Seat 0's next turn: the untap step asks about a Battery that may stay tapped.
    if keep {
        driver.idle_until(4, Step::Untap)?;
        let battery = driver.id_of("Battery")?;
        driver.answer(Answer::ChooseCards(CardsChoice(vec![battery])))?;
    }
    driver.idle_until(4, Step::Upkeep)?;
    assert_eq!(is_tapped(&driver, driver.id_of("Battery")?), keep);
    assert_eq!(bear_power(&driver), Some(power_after));
    assert_eq!(driver.state().effects().len(), usize::from(keep));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
