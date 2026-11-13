//! Goal: Kenessos, Priest of Thassa plays as printed at tables of two, three and four seats: it makes a scry of N cards a
//! scry of N plus one (Opt scries 1 card without it and 2 with it), and its activated ability
//! puts a Kraken, Leviathan, Octopus or Serpent creature card from the top of the library onto the battlefield when
//! the controller agrees, and otherwise (another card, or a refusal) offers to put it on the bottom; the log replays to
//! the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ScryChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::pay;
use crate::common::pid;

const KENESSOS: &str = "Kenessos, Priest of Thassa";

/// A 3/3 creature card named `name` of the creature type `subtype`.
fn r5500_creature(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_mana_cost(ManaCost::parse("{2}{U}").unwrap())
            .with_power_toughness(3, 3),
    )
}

fn r5500_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 controls `copies` Kenessos (a legend, so at most one) and an Island and casts Opt: the scry offers 1 + `copies` cards.
#[rstest]
fn r5500_each_kenessos_scries_one_more(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] copies: usize,
) -> Result<(), ScriptError> {
    let mut mine: Vec<_> = (0..copies)
        .map(|_| ready(card(KENESSOS).unwrap()))
        .collect();
    mine.push(ready(island()));
    let library: Vec<_> = (0..6)
        .map(|i| r5500_creature(&format!("Fish {i}"), "Fish"))
        .collect();
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .library(0, library)
        .hand(0, [card("Opt").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Opt", &["Island"])?;
    for _ in 0..20 {
        if let DecisionKind::Scry(options) = &driver.decision()?.kind {
            assert_eq!(options.cards.len(), 1 + copies);
            let cards = options.cards.clone();
            driver.answer(Answer::Scry(ScryChoice {
                top: cards,
                bottom: Vec::new(),
            }))?;
            r5500_replays(&driver);
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("Opt never scried".to_owned()))
}

/// Seat 0 activates Kenessos with a library of `subtype` creatures and answers every "may" with `yes`: a Serpent goes
/// onto the battlefield when accepted; otherwise the card is put on the bottom when accepted, and stays on top when not.
#[rstest]
#[case::a_serpent_put_in_play("Serpent", true, Some(Zone::Battlefield))]
#[case::a_kraken_put_in_play("Kraken", true, Some(Zone::Battlefield))]
#[case::a_serpent_left_on_top("Serpent", false, None)]
#[case::a_fish_to_the_bottom("Fish", true, None)]
#[case::a_fish_left_on_top("Fish", false, None)]
fn r5500_kenessos_looks_for_a_sea_monster(
    #[values(2, 3, 4)] seats: usize,
    #[case] subtype: &str,
    #[case] yes: bool,
    #[case] lands_in: Option<Zone>,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card(KENESSOS).unwrap())];
    mine.extend((0..4).map(|_| ready(island())));
    mine.push(ready(forest()));
    let library: Vec<_> = (0..4)
        .map(|i| r5500_creature(&format!("Beast {i}"), subtype))
        .collect();
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .library(0, library);
    let mut driver = Driver::from_scenario(scenario)?;
    let top = *driver.state().zone(Zone::Library(pid(0))).last().unwrap();
    driver.begin_activation(KENESSOS)?;
    let mut asked = 0;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && asked > 0 {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            asked += 1;
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            pay(&mut driver)?;
        } else {
            driver.answer_passively()?;
        }
        if driver.state().stack().is_empty() && asked > 0 {
            break;
        }
    }
    let library = driver.state().zone(Zone::Library(pid(0)));
    match lands_in {
        Some(zone) => assert_eq!(driver.state().object(top).unwrap().zone(), zone),
        None if yes => assert_eq!(library.first(), Some(&top), "put on the bottom"),
        None => assert_eq!(library.last(), Some(&top), "left on top"),
    }
    // A sea monster asks to put it in play, then, when refused, to put it on the bottom; any other card asks the latter.
    let monster = subtype != "Fish";
    assert_eq!(asked, if monster && !yes { 2 } else { 1 });
    r5500_replays(&driver);
    Ok(())
}
