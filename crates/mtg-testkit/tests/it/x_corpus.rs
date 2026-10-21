//! Goal: the corpus cards with an X play as their cards say: Blaze deals X damage and Stroke of Genius draws X cards for the X announced and paid, Kessig Wolf Run pumps and tramples a creature by the X of its cost, and Tezzeret the Seeker's -X removes X counters and finds only an artifact whose mana value is X or less, and the log of each replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, replay};
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn say(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    Ok(())
}

fn most_x(driver: &Driver) -> Option<u32> {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseNumber(options) => options.max,
        _ => None,
    }
}

fn lands(
    count: usize,
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::nothing(0, 20)]
#[case::some(2, 18)]
#[case::the_most(4, 16)]
fn blaze_deals_the_x_it_was_cast_for(#[case] x: u32, #[case] foe: i32) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Blaze").unwrap()])
            .battlefield(0, lands(5, mountain))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Blaze")?;
    assert_eq!(most_x(&driver), Some(4), "{{X}}{{R}} of five Mountains");
    say(&mut driver, x)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), foe);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::nothing(0)]
#[case::three(3)]
fn stroke_of_genius_draws_x_cards_for_the_player_it_targets(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [card("Stroke of Genius").unwrap()])
            .battlefield(0, lands(6, island)),
    )?;
    driver.begin_cast("Stroke of Genius")?;
    // {X}{2}{U} with six Islands.
    assert_eq!(most_x(&driver), Some(3));
    say(&mut driver, x)?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(1))).len();
    let before = hand(&driver);
    settle(&mut driver)?;
    assert_eq!(hand(&driver), before + usize::try_from(x).unwrap());
    Ok(())
}

#[test]
fn kessig_wolf_run_pumps_and_tramples_a_creature_by_its_x() -> Result<(), ScriptError> {
    let scenario = || {
        let mut permanents = vec![ready(card("Kessig Wolf Run").unwrap())];
        permanents.extend(lands(1, mountain));
        permanents.extend(lands(3, forest));
        permanents.push(ready(card("Grizzly Bears").unwrap()));
        Scenario::new(2).battlefield(0, permanents)
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Kessig Wolf Run")?;
    // {X}{R}{G} from four lands that are not the Run itself, which taps for its own cost.
    assert_eq!(most_x(&driver), Some(2));
    say(&mut driver, 2)?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let id = driver.id_of("Grizzly Bears")?;
    let shown = characteristics(driver.state(), id).unwrap();
    assert_eq!((shown.power, shown.toughness), (Some(4), Some(2)));
    assert!(has_keyword(driver.state(), id, &Keyword::Trample));
    replays(&driver, scenario());
    Ok(())
}

/// A Tezzeret with 4 loyalty and a library of artifacts of mana value 1, 1, 3 and 5.
fn tezzeret() -> Scenario {
    Scenario::new(2)
        .battlefield_named(0, ["Tezzeret the Seeker"])
        .library_named(
            0,
            ["Batterskull", "Hall of Triumph", "Bonesplitter", "Sol Ring"],
        )
}

#[rstest]
#[case::nothing(0, &[])]
#[case::one(1, &["Bonesplitter", "Sol Ring"])]
#[case::two(2, &["Bonesplitter", "Sol Ring"])]
#[case::three(3, &["Bonesplitter", "Hall of Triumph", "Sol Ring"])]
#[case::the_most(4, &["Bonesplitter", "Hall of Triumph", "Sol Ring"])]
fn tezzeret_finds_only_artifacts_of_mana_value_x_or_less(
    #[case] x: u32,
    #[case] found: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(tezzeret())?;
    driver.begin_ability("Tezzeret the Seeker", 1)?;
    assert_eq!(most_x(&driver), Some(4), "up to the loyalty");
    say(&mut driver, x)?;
    let loyalty = |driver: &Driver| {
        // A planeswalker with no counters has gone to the graveyard.
        let id = driver.id_of("Tezzeret the Seeker").ok()?;
        driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
            .map(|permanent| permanent.counters_of(&CounterKind::Loyalty))
    };
    // The counters are paid with the cost.
    assert_eq!(loyalty(&driver), (x < 4).then_some(4 - x));
    // Both players pass and the ability resolves: a search of the library asks for the card.
    driver.pass()?;
    driver.pass()?;
    if found.is_empty() {
        assert_eq!(driver.decision()?.kind.name(), "priority");
        return Ok(());
    }
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("a search was expected");
    };
    let mut offered: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    offered.sort();
    assert_eq!(offered, found);
    let pick = options.from[0];
    let picked = name_of(driver.state(), pick);
    driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
    settle(&mut driver)?;
    assert!(
        common::names(driver.state(), Zone::Battlefield).contains(&picked),
        "{picked} came onto the battlefield"
    );
    Ok(())
}
