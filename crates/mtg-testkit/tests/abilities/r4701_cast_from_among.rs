//! Goal: the r47-01 casts from among cards play at two to four seats: Glimpse the Cosmos is cast from its graveyard for
//! {U} only while its caster controls a Giant and is exiled as it leaves the stack; Tinybones lets its controller cast a
//! permanent card from the graveyard of the player it damaged with mana of any type; Sanwell casts an artifact creature
//! from the six cards it exiles and puts the rest on the bottom; Sunbird's Invocation casts a card of the revealed top
//! free and puts the rest on the bottom.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// How many objects named `name` lie in `zone`.
fn r4701_count(driver: &Driver, zone: Zone, name: &str) -> usize {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|object| object.card().name == name)
        })
        .count()
}

/// Whether the deciding player is offered to cast the named card now.
fn r4701_castable(driver: &Driver, name: &str) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.iter().any(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|object| object.card().name == name)
        }),
        _ => false,
    }
}

/// Answers until the stack is empty: the named card is chosen and targeted wherever it is offered, every yes-or-no
/// question is answered yes, mana is paid automatically and priority is passed.
fn r4701_drive(driver: &mut Driver, wanted: &str) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_named(wanted)?;
                driver.choose_targets(&[target])?;
            }
            DecisionKind::ChooseCards(options) => {
                let picked: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| {
                        driver
                            .state()
                            .object(*id)
                            .is_some_and(|object| object.card().name == wanted)
                    })
                    .take(1)
                    .collect();
                let picked = if picked.is_empty() && options.from.len() <= 3 {
                    options.from.iter().copied().take(1).collect()
                } else {
                    picked
                };
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("r4701 drive".to_owned()))
}

#[rstest]
#[case::with_a_giant(2, true)]
#[case::without_a_giant(3, false)]
#[case::with_a_giant_four_seats(4, true)]
fn r4701_glimpse_the_cosmos_from_the_graveyard_for_blue(
    #[case] seats: usize,
    #[case] giant: bool,
) -> Result<(), ScriptError> {
    let mut side = vec![ready(island())];
    if giant {
        side.push(ready(card("Arbor Colossus").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .battlefield(0, side)
        .graveyard(0, [card("Glimpse the Cosmos").unwrap()])
        .library(0, [swamp(), swamp(), swamp(), swamp()])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let player = PlayerId::from_index(0);
    assert_eq!(r4701_castable(&driver, "Glimpse the Cosmos"), giant);
    if giant {
        driver.cast("Glimpse the Cosmos")?;
        r4701_drive(&mut driver, "Swamp")?;
        assert_eq!(r4701_count(&driver, Zone::Exile, "Glimpse the Cosmos"), 1);
        assert_eq!(r4701_count(&driver, Zone::Hand(player), "Swamp"), 1);
    }
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r4701_tinybones_casts_from_the_damaged_players_graveyard(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Tinybones, the Pickpocket").unwrap()),
                ready(swamp()),
                ready(swamp()),
            ],
        )
        .graveyard(1, [card("Grizzly Bears").unwrap()])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Tinybones, the Pickpocket", 1)])?;
    for _ in 0..40 {
        if driver.state().turn().step() == Step::CombatDamage && !driver.state().stack().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    r4701_drive(&mut driver, "Grizzly Bears")?;
    // Two Swamps paid for {1}{G}: any type of mana could be spent.
    assert_eq!(r4701_count(&driver, Zone::Battlefield, "Grizzly Bears"), 1);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r4701_sanwell_casts_an_artifact_creature_from_the_six(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let library = [
        card("Bonded Construct").unwrap(),
        forest(),
        forest(),
        card("Grizzly Bears").unwrap(),
        forest(),
        forest(),
        forest(),
    ];
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [ready(card("Sanwell, Avenger Ace").unwrap()), ready(swamp())],
        )
        .library(0, library)
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let player = PlayerId::from_index(0);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Sanwell, Avenger Ace", 1)])?;
    r4701_drive(&mut driver, "Bonded Construct")?;
    assert_eq!(
        r4701_count(&driver, Zone::Battlefield, "Bonded Construct"),
        1
    );
    // The rest went back under the library, the Bears (no artifact) among them.
    assert_eq!(driver.state().zone(Zone::Library(player)).len(), 6);
    assert_eq!(r4701_count(&driver, Zone::Exile, "Grizzly Bears"), 0);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r4701_sunbirds_invocation_casts_from_the_revealed_top(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Sunbird's Invocation").unwrap()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .hand(0, [card("Grizzly Bears").unwrap()])
        .library(0, [card("Grizzly Bears").unwrap(), swamp(), swamp()])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let player = PlayerId::from_index(0);
    driver.cast("Grizzly Bears")?;
    r4701_drive(&mut driver, "Grizzly Bears")?;
    assert_eq!(r4701_count(&driver, Zone::Battlefield, "Grizzly Bears"), 2);
    assert_eq!(driver.state().zone(Zone::Library(player)).len(), 2);
    Ok(())
}
