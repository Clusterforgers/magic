//! Goal: the cards that choose cards and move them, and the tokens counted by what a player controls, play as printed at tables of two, three and four seats: Time Wipe returns the creature its player picks (or none) and destroys all creatures, Worldsoul's Rage deals X damage and puts up to X land cards of the hand and the graveyard onto the battlefield tapped, Nervous Gardener searches for a land with a basic land type when it is turned face up, Arachnogenesis makes a Spider for each creature attacking its controller and stops the combat damage of the other creatures, Curious Herd makes a Beast for each artifact the chosen opponent controls and Evangel of Heliod makes a Soldier for each white mana symbol among its controller's permanents; every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::c4_control::{controller, replays};
use crate::common::casting::{choose_way, pay};
use crate::common::{names, pid};
use mtg_engine::PresetPermanent;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers what the spell asks until it is done: the number X, the target (the first opponent), the payment, and the cards
/// named in `picks` (every card of the question when `all`, else the first offered up to the maximum).
fn resolve(driver: &mut Driver, x: u32, picks: &[&str]) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_player(1);
                driver.choose_targets(&[target])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(options) => {
                let wanted: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| {
                        let name = &driver.state().object(*id).unwrap().card().name;
                        picks.contains(&name.as_str())
                    })
                    .take(options.max as usize)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(wanted)))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the spell".into()))
}

fn in_zone(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|found| *found == name)
        .count()
}

fn on_table(driver: &Driver, name: &str) -> usize {
    in_zone(driver, Zone::Battlefield, name)
}

fn lands(white: usize, blue: usize, green: usize, red: usize) -> Vec<PresetPermanent> {
    let mut lands: Vec<PresetPermanent> = Vec::new();
    lands.extend((0..white).map(|_| ready(plains())));
    lands.extend((0..blue).map(|_| ready(island())));
    lands.extend((0..green).map(|_| ready(forest())));
    lands.extend((0..red).map(|_| ready(mountain())));
    lands
}

#[rstest]
#[case::returns_the_bear(&["Bear", "Boar"], "Bear")]
#[case::returns_the_boar(&["Bear", "Boar"], "Boar")]
#[case::a_table_without_creatures_returns_none(&[], "")]
fn time_wipe_returns_the_creature_picked_and_destroys_all_creatures(
    #[case] mine: &[&str],
    #[case] back: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let creatures: Vec<PresetPermanent> = mine
        .iter()
        .map(|name| ready(free_creature(name, 2, 2)))
        .collect();
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Time Wipe").unwrap()])
        .battlefield(0, lands(2, 3, 0, 0).into_iter().chain(creatures));
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Ogre {seat}"), 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Time Wipe")?;
    resolve(&mut driver, 0, &[back])?;
    let table = names(driver.state(), Zone::Battlefield);
    assert!(
        !table
            .iter()
            .any(|name| name == "Bear" || name == "Boar" || name.starts_with("Ogre")),
        "{table:?}"
    );
    if !back.is_empty() {
        assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), back), 1);
    }
    for seat in 1..seats {
        assert_eq!(
            in_zone(&driver, Zone::Graveyard(pid(seat)), &format!("Ogre {seat}")),
            1
        );
    }
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::nothing(0)]
#[case::two(2)]
#[case::more_than_there_are(5)]
fn worldsouls_rage_deals_x_and_puts_up_to_x_lands_tapped(
    #[case] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [
                card("Worldsoul's Rage").unwrap(),
                forest(),
                forest(),
                plains(),
            ],
        )
        .graveyard(0, [mountain()])
        .battlefield(0, lands(0, 0, 4, 3));
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(1)).unwrap().life();
    driver.begin_cast("Worldsoul's Rage")?;
    resolve(&mut driver, x, &["Forest", "Plains", "Mountain"])?;
    assert_eq!(
        driver.state().player(pid(1)).unwrap().life(),
        life - x.cast_signed()
    );
    let lands_in = 7 + (x.min(4) as usize);
    let table = names(driver.state(), Zone::Battlefield);
    let lands_now = table
        .iter()
        .filter(|name| matches!(name.as_str(), "Forest" | "Plains" | "Mountain"))
        .count();
    assert_eq!(lands_now, lands_in, "{table:?}");
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_attackers_at_the_player(2)]
#[case::one_attacker(1)]
fn arachnogenesis_makes_a_spider_for_each_creature_attacking_you_and_stops_other_combat_damage(
    #[case] attackers: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut hunters: Vec<PresetPermanent> = (0..attackers)
        .map(|n| ready(free_creature(&format!("Hunter {n}"), 2, 2)))
        .collect();
    // In a game of three or more, one more creature attacks someone else and is not counted.
    hunters.push(ready(free_creature("Stray", 2, 2)));
    let mut scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .hand(0, [card("Arachnogenesis").unwrap()])
        .battlefield(0, lands(0, 0, 3, 0))
        .battlefield(1, hunters);
    for seat in 2..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Ogre {seat}"), 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let mut attacks: Vec<(String, usize)> =
        (0..attackers).map(|n| (format!("Hunter {n}"), 0)).collect();
    attacks.push(("Stray".to_owned(), if seats > 2 { 2 } else { 0 }));
    let attacks: Vec<(&str, usize)> = attacks.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    driver.attack(&attacks)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Arachnogenesis")?;
    resolve(&mut driver, 0, &[])?;
    let at_you = attackers + usize::from(seats == 2);
    assert_eq!(on_table(&driver, "Spider"), at_you);
    assert_eq!(controller(&driver, "Spider"), 0);
    driver.idle_until(1, Step::Main2)?;
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life,
        "the damage is prevented"
    );
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::none(0)]
#[case::one(1)]
#[case::three(3)]
fn curious_herd_makes_a_beast_for_each_artifact_the_chosen_opponent_controls(
    #[case] artifacts: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Curious Herd").unwrap()])
        .battlefield(0, lands(0, 0, 4, 0))
        .battlefield_named(1, vec!["Sol Ring"; artifacts]);
    for seat in 2..seats {
        scenario = scenario.battlefield_named(seat, ["Sol Ring", "Sol Ring"]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Curious Herd")?;
    resolve(&mut driver, 0, &[])?;
    assert_eq!(on_table(&driver, "Beast"), artifacts);
    assert_eq!(
        on_table(&driver, "Sol Ring"),
        artifacts + 2 * seats.saturating_sub(2)
    );
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::alone(0, 0, 2)]
#[case::with_another_white_permanent(1, 0, 4)]
#[case::an_opponents_white_permanents_do_not_count(0, 1, 2)]
fn evangel_of_heliod_makes_a_soldier_for_each_white_symbol_among_your_permanents(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] soldiers: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Evangel of Heliod").unwrap()])
        .battlefield(
            0,
            lands(6, 0, 0, 0)
                .into_iter()
                .chain((0..mine).map(|_| ready(card("Evangel of Heliod").unwrap()))),
        );
    scenario = scenario.battlefield(
        1,
        (0..theirs).map(|_| ready(card("Evangel of Heliod").unwrap())),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Evangel of Heliod")?;
    resolve(&mut driver, 0, &[])?;
    assert_eq!(on_table(&driver, "Soldier"), soldiers);
    replays(&driver);
    Ok(())
}

#[rstest]
fn nervous_gardener_fetches_a_land_with_a_basic_type_when_it_is_turned_face_up(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Nervous Gardener").unwrap()])
        .battlefield(0, lands(0, 0, 4, 0))
        .library(0, [forest(), mountain(), card("Command Tower").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Nervous Gardener")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Cast face down")?;
    }
    pay(&mut driver)?;
    for _ in 0..4 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let id = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down permanent");
    driver.answer(Answer::Priority(
        mtg_engine::decision::PriorityAction::TurnFaceUp(id),
    ))?;
    pay(&mut driver)?;
    // The trigger waits on the stack until both players pass.
    for _ in 0..8 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!(
            "the search asks, not {}",
            driver.decision().unwrap().kind.name()
        );
    };
    let offered: Vec<String> = options
        .from
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    assert!(
        !offered.contains(&"Command Tower".to_owned()),
        "{offered:?}"
    );
    assert_eq!(offered.len(), 2, "{offered:?}");
    resolve(&mut driver, 0, &["Forest"])?;
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), "Forest"), 1);
    replays(&driver);
    Ok(())
}
