//! Goal: several draws of one event replaced one by one by Abundance (Windfall, a "draw two", two players drawing at once) are sequential (CR 121.6, 614.6): each replaced draw reveals from the library as the draws and replacements before it left it, a declined replacement draws the top card, the cards revealed go to the bottom, and the log replays to the live state; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

/// Abundance's replacement, as the card reads.
const ABUNDANCE: &str = r#"{"Replacement":{"when":{"Draw":{}},"by":{"Instead":[{"May":{"effects":[
    {"ChooseValue":{"kind":{"Options":["land","nonland"]},"as":"kind"}},
    {"If":{"condition":{"ChosenWord":{"name":"kind","word":"land"}},
      "then":[{"RevealUntil":{"filter":[{"Type":"Land"}],"as":"found","revealed_as":"revealed"}},
              {"Move":{"what":{"Bound":"found"},"to":"Hand"}},
              {"Move":{"what":{"Bound":"revealed"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}],
      "otherwise":[{"RevealUntil":{"filter":[{"Not":{"Type":"Land"}}],"as":"found","revealed_as":"revealed"}},
              {"Move":{"what":{"Bound":"found"},"to":"Hand"}},
              {"Move":{"what":{"Bound":"revealed"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}]}}]}}}"#;

/// The enchantment Abundance.
fn abundance() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(ABUNDANCE).unwrap();
    Arc::new(
        CardDefinition::new("Abundance")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

/// A free sorcery: "Draw two cards."
fn divination() -> Arc<CardDefinition> {
    let rule: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{"count":2}}]}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Divination")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_core::mana::ManaCost::default())
            .with_rules(rule),
    )
}

/// Top first, eight cards. Shallow: a Bears over a Forest over a Bears over an Island, so each land found reveals one
/// card. Deep: two Bears over a Forest over two Bears over an Island, so each land found reveals two cards and their
/// order on the bottom is asked.
fn library(deep: bool) -> Vec<Arc<CardDefinition>> {
    if deep {
        vec![
            bear(),
            bear(),
            forest(),
            bear(),
            bear(),
            island(),
            forest(),
            forest(),
        ]
    } else {
        vec![
            bear(),
            forest(),
            bear(),
            island(),
            forest(),
            bear(),
            forest(),
            forest(),
        ]
    }
}

/// How the draws are answered: the replacement declined, land or nonland chosen, or land chosen for each player's first
/// draw and the replacement declined for the next, or the other way round.
#[derive(Clone, Copy, Debug)]
enum Way {
    Declined,
    Land,
    Nonland,
    LandThenKept,
    KeptThenLand,
}

/// The seats with Abundance hold it, every seat has [`library`]; seat 0 casts `spell` (Windfall with three Islands, or
/// Divination), and seat 1 holds two cards so Windfall draws two for each player.
fn scenario(seats: usize, spell: &str, abundant: &[usize], deep: bool) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let mut permanents = Vec::new();
        if abundant.contains(&seat) {
            permanents.push(ready(abundance()));
        }
        if seat == 0 && spell == "Windfall" {
            permanents.extend([ready(island()), ready(island()), ready(island())]);
        }
        scenario = scenario
            .library(seat, library(deep))
            .battlefield(seat, permanents);
    }
    if spell == "Windfall" {
        scenario
            .hand(0, [card("Windfall").unwrap()])
            .hand(1, [bear(), bear()])
    } else {
        scenario.hand(0, [divination()])
    }
}

/// Answers every question of Abundance with `way` and the rest passively until the stack is empty; the log replays.
fn settle(driver: &mut Driver, way: Way) -> Result<(), ScriptError> {
    let mut asked = std::collections::BTreeMap::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => {
                let before: &mut usize = asked.entry(decision.player).or_default();
                *before += 1;
                Answer::YesNo(YesNoChoice(match way {
                    Way::Declined => false,
                    Way::Land | Way::Nonland => true,
                    Way::LandThenKept => *before == 1,
                    Way::KeptThenLand => *before > 1,
                }))
            }
            DecisionKind::ChooseOption(_) => {
                Answer::ChooseOption(OptionChoice(usize::from(matches!(way, Way::Nonland))))
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
                assert_eq!(again.state_hash(), driver.state().state_hash());
                return Ok(());
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    panic!("the stack did not empty");
}

/// The names of a player's library, top first.
fn top_first(driver: &Driver, seat: usize) -> Vec<String> {
    driver
        .state()
        .zone(Zone::Library(pid(seat)))
        .iter()
        .rev()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// The sorted names of a player's hand.
fn hand(driver: &Driver, seat: usize) -> Vec<String> {
    let mut hand = names(driver.state(), Zone::Hand(pid(seat)));
    hand.sort();
    hand
}

/// The hand two draws leave from [`library`] and the library's top three after them.
fn expected(way: Way, deep: bool) -> (Vec<&'static str>, Vec<&'static str>) {
    match (way, deep) {
        // Shallow declined: the Bears and the Forest drawn. Deep, land then kept: two Bears to the bottom, Forest
        // found, then the third Bears drawn. Shallow, kept then land: the Bears drawn, then the Forest found on top.
        (Way::Declined | Way::KeptThenLand, false) | (Way::LandThenKept, true) => (
            vec!["Forest", "Grizzly Bears"],
            vec!["Grizzly Bears", "Island", "Forest"],
        ),
        // Bears to the bottom, Forest found; then the second Bears to the bottom, Island found.
        (Way::Land, false) => (
            vec!["Forest", "Island"],
            vec!["Forest", "Grizzly Bears", "Forest"],
        ),
        // The first Bears; then the Forest to the bottom and the second Bears found.
        (Way::Nonland, false) => (
            vec!["Grizzly Bears", "Grizzly Bears"],
            vec!["Island", "Forest", "Grizzly Bears"],
        ),
        // Two Bears to the bottom, Forest found; then two more Bears to the bottom, Island found.
        (Way::Land, true) => (
            vec!["Forest", "Island"],
            vec!["Forest", "Forest", "Grizzly Bears"],
        ),
        // Bears to the bottom, Forest found; then the second Bears drawn.
        (Way::LandThenKept, false) => (
            vec!["Forest", "Grizzly Bears"],
            vec!["Island", "Forest", "Grizzly Bears"],
        ),
        // The Bears drawn; then the second Bears to the bottom, Forest found.
        (Way::KeptThenLand, true) => (
            vec!["Forest", "Grizzly Bears"],
            vec!["Grizzly Bears", "Grizzly Bears", "Island"],
        ),
        // The two Bears on top, drawn or found.
        (Way::Declined | Way::Nonland, true) => (
            vec!["Grizzly Bears", "Grizzly Bears"],
            vec!["Forest", "Grizzly Bears", "Grizzly Bears"],
        ),
    }
}

#[rstest]
fn each_draw_of_one_event_replaced_by_abundance_sees_the_draws_before_it(
    #[values("Windfall", "Divination")] spell: &str,
    #[values(
        Way::Declined,
        Way::Land,
        Way::Nonland,
        Way::LandThenKept,
        Way::KeptThenLand
    )]
    way: Way,
    #[values(&[0][..], &[0, 1][..])] abundant: &[usize],
    #[values(false, true)] deep: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(seats, spell, abundant, deep))?;
    driver.cast(spell)?;
    settle(&mut driver, way)?;
    let (got, top) = expected(way, deep);
    for seat in 0..seats {
        let library = top_first(&driver, seat);
        assert_eq!(library.len() + hand(&driver, seat).len(), 8, "seat {seat}");
        if spell == "Divination" && seat != 0 {
            assert_eq!(library.len(), 8, "seat {seat}");
            continue;
        }
        let (got, top) = if abundant.contains(&seat) {
            (got.clone(), top.clone())
        } else {
            expected(Way::Declined, deep)
        };
        assert_eq!(hand(&driver, seat), got, "seat {seat}");
        assert_eq!(library[..3], top[..], "seat {seat}");
    }
    Ok(())
}
