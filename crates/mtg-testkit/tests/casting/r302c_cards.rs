//! Goal: pack r30-2c's spells play as printed at two to four seats: Banefire with X of 5 or more can't be countered and its damage can't be prevented (with X of 4 both can), kicked Molten Disaster silences every seat while unkicked it does not, Dragonlord's Prerogative can't be countered when its Dragon was revealed or a Dragon was in play as it was cast and can be otherwise, Shifting Loyalties exchanges two permanents that share a card type and refuses two that share none, and Unexplained Absence exiles one chosen permanent per player and gives each of those players a cloaked card from their own library.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, CostsChoice, NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A creature with no cost and "Prevent all damage that would be dealt to this creature."
fn shielded(toughness: i32) -> Arc<CardDefinition> {
    let rules = r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":"Prevent"}}"#;
    Arc::new(
        CardDefinition::new("Shielded")
            .with_type(CardType::Creature)
            .with_power_toughness(1, toughness)
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    )
}

fn dragon() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Wyrm")
            .with_type(CardType::Creature)
            .with_subtype("Dragon")
            .with_power_toughness(4, 4),
    )
}

/// Answers the announcement of the spell being cast: the optional costs `costs`, X, the targets, a card a cost picks
/// (the first offered) and the mana, paid automatically.
fn announce(
    driver: &mut Driver,
    x: u32,
    costs: &[usize],
    targets: &[Target],
) -> Result<(), ScriptError> {
    for _ in 0..12 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::ChooseCosts(_) => {
                driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs.to_vec())))?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(targets)?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from[0];
                driver.answer(Answer::ChooseCards(CardsChoice(vec![first])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => return Ok(()),
        }
    }
    Err(ScriptError::Stuck("the announcement".into()))
}

/// Every seat passes until the stack is empty, answering anything else passively.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 1 counters the spell on top of the stack with a Counterspell, all others passing until it has priority.
fn counter_it(driver: &mut Driver, spell: &str) -> Result<(), ScriptError> {
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    driver.begin_cast("Counterspell")?;
    let target = on_stack(driver, spell);
    announce(driver, 0, &[], &[target])
}

/// The named spell on the stack as a target.
fn on_stack(driver: &Driver, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .stack()
        .iter()
        .copied()
        .find(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.card().name == name)
        })
        .unwrap();
    Target::Object(state.object(id).unwrap().reference())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn with_counterspell(mut scenario: Scenario) -> Scenario {
    scenario = scenario
        .hand(1, [card("Counterspell").unwrap()])
        .battlefield(1, [ready(island()), ready(island())]);
    scenario
}

#[rstest]
fn banefire_with_x_of_five_or_more_cannot_be_countered(
    #[values(2, 3, 4)] seats: usize,
    #[values(4, 5)] x: u32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [card("Banefire").unwrap()])
        .battlefield(0, (0..6).map(|_| ready(mountain())).collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(with_counterspell(scenario))?;
    driver.begin_cast("Banefire")?;
    let target = driver.target_player(1);
    announce(&mut driver, x, &[], &[target])?;
    counter_it(&mut driver, "Banefire")?;
    settle(&mut driver)?;
    let dealt = if x >= 5 { i32::try_from(x).unwrap() } else { 0 };
    assert_eq!(life(&driver, 1), 20 - dealt);
    Ok(())
}

#[rstest]
fn banefire_with_x_of_five_or_more_deals_damage_that_cannot_be_prevented(
    #[values(2, 3, 4)] seats: usize,
    #[values(4, 5)] x: u32,
) -> Result<(), ScriptError> {
    let toughness = i32::try_from(x).unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Banefire").unwrap()])
            .battlefield(0, (0..6).map(|_| ready(mountain())).collect::<Vec<_>>())
            .battlefield(1, [ready(shielded(toughness))]),
    )?;
    driver.begin_cast("Banefire")?;
    let target = driver.target_named("Shielded")?;
    announce(&mut driver, x, &[], &[target])?;
    settle(&mut driver)?;
    let alive = names(driver.state(), Zone::Battlefield).contains(&"Shielded".to_owned());
    assert_eq!(alive, x < 5, "the damage is prevented only below 5");
    Ok(())
}

#[rstest]
fn molten_disaster_has_split_second_only_when_kicked(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] kicked: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Molten Disaster").unwrap()])
        .battlefield(0, (0..4).map(|_| ready(mountain())).collect::<Vec<_>>());
    for seat in 1..seats {
        scenario = scenario
            .hand(seat, [card("Lightning Bolt").unwrap()])
            .battlefield(seat, [ready(mountain())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Molten Disaster")?;
    let costs: &[usize] = if kicked { &[0] } else { &[] };
    announce(&mut driver, 1, costs, &[])?;
    driver.pass()?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 1 holds priority");
    };
    assert_eq!(options.castable.is_empty(), kicked, "split second silences");
    settle(&mut driver)?;
    for seat in 0..seats {
        assert_eq!(life(&driver, seat), 19, "seat {seat} was dealt X damage");
    }
    Ok(())
}

#[rstest]
#[case::the_dragon_revealed(true, false, false)]
#[case::a_dragon_in_play(false, true, false)]
#[case::no_dragon(false, false, true)]
fn dragonlords_prerogative_cannot_be_countered_with_a_dragon(
    #[values(2, 3, 4)] seats: usize,
    #[case] reveal: bool,
    #[case] in_play: bool,
    #[case] countered: bool,
) -> Result<(), ScriptError> {
    let mut board: Vec<_> = (0..6).map(|_| ready(island())).collect();
    if in_play {
        board.push(ready(dragon()));
    }
    let scenario = Scenario::new(seats)
        .hand(0, [card("Dragonlord's Prerogative").unwrap(), dragon()])
        .battlefield(0, board)
        .library(0, (0..6).map(|_| forest()).collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(with_counterspell(scenario))?;
    driver.begin_cast("Dragonlord's Prerogative")?;
    let costs: &[usize] = if reveal { &[0] } else { &[] };
    announce(&mut driver, 0, costs, &[])?;
    counter_it(&mut driver, "Dragonlord's Prerogative")?;
    settle(&mut driver)?;
    let drawn = driver.state().zone(Zone::Hand(pid(0))).len();
    // The Wyrm stays in hand (a revealed card is not discarded).
    let held = 1;
    assert_eq!(drawn, if countered { held } else { held + 4 });
    Ok(())
}

#[rstest]
fn shifting_loyalties_exchanges_two_permanents_that_share_a_card_type(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Shifting Loyalties").unwrap()])
            .battlefield(
                0,
                (0..6)
                    .map(|_| ready(island()))
                    .chain([ready(free_creature("Mine", 1, 1))])
                    .collect::<Vec<_>>(),
            )
            .battlefield(1, [ready(free_creature("Theirs", 3, 3)), ready(forest())]),
    )?;
    driver.begin_cast("Shifting Loyalties")?;
    let mine = driver.target_named("Mine")?;
    let land = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            driver.state().object(*id).is_some_and(|object| {
                object.card().name == "Forest" && object.controller() == pid(1)
            })
        })
        .unwrap();
    driver.choose_targets(&[mine])?;
    let land_target = Target::Object(driver.state().object(land).unwrap().reference());
    assert!(
        driver.choose_targets(&[land_target]).is_err(),
        "a land shares no card type with a creature"
    );
    let theirs = driver.target_named("Theirs")?;
    announce(&mut driver, 0, &[], &[theirs])?;
    settle(&mut driver)?;
    let controller = |name: &str| {
        let id = driver.id_of(name).unwrap();
        mtg_engine::characteristics::controller_of(driver.state(), id)
    };
    assert_eq!(controller("Mine"), Some(pid(1)));
    assert_eq!(controller("Theirs"), Some(pid(0)));
    Ok(())
}

#[rstest]
fn unexplained_absence_exiles_one_permanent_per_player_and_cloaks_for_each(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let relic = |seat: usize| {
        Arc::new(CardDefinition::new(format!("Relic {seat}")).with_type(CardType::Artifact))
    };
    let mut scenario = Scenario::new(seats).hand(0, [card("Unexplained Absence").unwrap()]);
    for seat in 0..seats {
        let lands = if seat == 0 { 4 } else { 0 };
        scenario = scenario
            .battlefield(
                seat,
                (0..lands)
                    .map(|_| ready(plains()))
                    .chain([ready(relic(seat))])
                    .collect::<Vec<_>>(),
            )
            .library(seat, (0..3).map(|_| forest()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Unexplained Absence")?;
    let targets: Vec<Target> = (1..seats)
        .map(|seat| driver.target_named(&format!("Relic {seat}")).unwrap())
        .collect();
    announce(&mut driver, 0, &[], &targets)?;
    settle(&mut driver)?;
    let battlefield = names(driver.state(), Zone::Battlefield);
    assert!(
        battlefield.contains(&"Relic 0".to_owned()),
        "seat 0's relic stays"
    );
    for seat in 1..seats {
        assert!(!battlefield.contains(&format!("Relic {seat}")));
        let cloaked = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                driver.state().object(**id).is_some_and(|object| {
                    mtg_engine::characteristics::controller_of(driver.state(), **id)
                        == Some(pid(seat))
                        && object.face_down().is_some()
                })
            })
            .count();
        assert_eq!(cloaked, 1, "seat {seat} cloaked one card");
        assert_eq!(driver.state().zone(Zone::Library(pid(seat))).len(), 2);
    }
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 3);
    Ok(())
}
