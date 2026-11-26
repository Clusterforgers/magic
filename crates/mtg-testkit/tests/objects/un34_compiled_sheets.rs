//! Goal: the Unfinity sticker sheets as the compiler reads them from their Oracle text play at tables of two to four
//! seats: a free name sticker, a keyword sticker, a run of keywords on one sticker and a power/toughness sticker each
//! change the permanent they are put on for their printed tickets, and every sheet that reads whole is `Verified` by
//! what the engine runs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
use mtg_cards::keyword::Keyword;
use mtg_cards::support::{PrintedCard, PrintedFace, Support, support};
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::un34_stickerer;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The tickets seat 0 holds.
const HELD: u32 = 5;

fn ticket() -> CounterKind {
    CounterKind::Other("ticket".to_owned())
}

/// The sheets of the compiler's fixture, as printed.
fn printed_sheets() -> Vec<PrintedCard> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../mtg-cards/tests/fixtures/un34_sheets.jsonl");
    let file = std::fs::File::open(path).unwrap();
    mtg_import::scryfall::cards_jsonl(std::io::BufReader::new(file))
        .map(|record| {
            let record = record.unwrap();
            PrintedCard {
                layout: record.layout.keyword().to_owned(),
                oracle_id: record.oracle_id.to_string(),
                name: record.name.clone(),
                keywords: record.keywords.clone(),
                faces: record
                    .faces()
                    .map(|face| PrintedFace {
                        name: face.name.clone(),
                        type_line: face.type_line.clone(),
                        oracle_text: face.oracle_text.clone(),
                        ..PrintedFace::default()
                    })
                    .collect(),
            }
        })
        .collect()
}

/// The sheet with its compiled stickers.
fn sheet(name: &str) -> Arc<CardDefinition> {
    let printed = printed_sheets()
        .into_iter()
        .find(|sheet| sheet.name == name)
        .unwrap();
    let card = compile(&printed).card.unwrap();
    assert_eq!(card.impl_status, ImplStatus::Full, "{name}");
    let mut definition = CardDefinition::new(name);
    for ability in &card.faces[0].abilities {
        definition = definition.with_rules(ability.clone());
    }
    Arc::new(definition)
}

fn bear_of(driver: &Driver, seat: usize) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().owner() == pid(seat))
        .unwrap()
}

/// What the sticker does to the Bear.
#[derive(Debug, Clone, Copy)]
enum Change {
    Named(&'static str),
    Keywords(&'static [Keyword]),
    Stats(i32, i32),
}

#[rstest]
fn a_compiled_sticker_changes_the_permanent_for_its_tickets(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        ("Ancestral Hot Dog Minotaur", "\u{201c}Hot Dog\u{201d}", 0, Change::Named("Hot Dog Grizzly Bears")),
        ("Ancestral Hot Dog Minotaur", "Flying", 3, Change::Keywords(&[Keyword::Flying])),
        ("Ancestral Hot Dog Minotaur", "8/6", 5, Change::Stats(8, 6)),
        ("Narrow-Minded Baloney Fireworks", "Vigilance, reach", 3, Change::Keywords(&[Keyword::Vigilance, Keyword::Reach])),
        ("Contortionist Otter Storm", "Deathtouch, lifelink", 4, Change::Keywords(&[Keyword::Deathtouch, Keyword::Lifelink])),
        ("Cool Fluffy Loxodon", "4/2", 2, Change::Stats(4, 2))
    )]
    case: (&str, &str, u32, Change),
) -> Result<(), ScriptError> {
    let (name, label, tickets, change) = case;
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [sheet(name)])
        .counters(0, ticket(), HELD)
        .hand(0, [un34_stickerer("Stickerer")]);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, [ready(mtg_testkit::cards::bear())]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = bear_of(&driver, 0);
    driver.cast("Stickerer")?;
    while !driver.state().stack().is_empty() {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseOption(options) = &decision.kind {
            let wanted = format!("{name}: {label}");
            let at = options
                .options
                .iter()
                .position(|option| {
                    matches!(option, ChoiceOption::Label(text) if text.starts_with(&wanted))
                })
                .unwrap_or_else(|| panic!("{wanted} among {:?}", options.options));
            driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    let state = driver.state();
    assert_eq!(state.stickers_on(bear).len(), 1);
    assert_eq!(
        state.player(pid(0)).unwrap().counters_of(&ticket()),
        HELD - tickets
    );
    match change {
        Change::Named(full) => assert_eq!(state.name_of(bear).unwrap(), full),
        Change::Keywords(keywords) => {
            for keyword in keywords {
                assert!(
                    mtg_engine::keywords::has_keyword(state, bear, keyword),
                    "{keyword:?}"
                );
            }
        }
        Change::Stats(power, toughness) => {
            let chars = mtg_engine::characteristics::characteristics(state, bear).unwrap();
            assert_eq!(
                (chars.power, chars.toughness),
                (Some(power), Some(toughness))
            );
        }
    }
    for seat in 1..seats {
        assert_eq!(driver.state().stickers_on(bear_of(&driver, seat)), []);
    }
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::StickerPut { .. }))
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[test]
fn every_whole_sheet_is_verified_by_what_the_engine_runs() {
    let caps = mtg_engine::caps();
    let mut whole = 0;
    for printed in printed_sheets() {
        let card = compile(&printed).card.unwrap();
        if card.impl_status != ImplStatus::Full {
            continue;
        }
        whole += 1;
        let record = support(Some(&card), &printed, &caps);
        assert_eq!(
            record.support,
            Support::Verified,
            "{}: {:?}",
            printed.name,
            record.clauses
        );
    }
    assert_eq!(whole, 32);
}
