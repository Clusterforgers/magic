//! Goal: Greymond asks for two different abilities from among first strike, vigilance, and lifelink as it enters, at
//! two to four seats; the Humans its controller controls have exactly the chosen two (not the third, not other players'
//! Humans or non-Humans), get +2/+2 only with four or more Humans, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const GREYMOND: &str = "Greymond, Avacyn's Stalwart";

/// A free 1/1 Human named `name`.
fn r6001_human(name: &str) -> Arc<CardDefinition> {
    let base = free_creature(name, 1, 1);
    Arc::new((*base).clone().with_subtype("Human"))
}

const ABILITIES: [(&str, Keyword); 3] = [
    ("first strike", Keyword::FirstStrike),
    ("vigilance", Keyword::Vigilance),
    ("lifelink", Keyword::Lifelink),
];

#[rstest]
#[case::first_strike_and_vigilance(2, [0, 1], 1)]
#[case::lifelink_then_first_strike(3, [2, 0], 3)]
#[case::vigilance_and_lifelink(4, [1, 2], 2)]
#[case::four_humans_at_two(2, [2, 1], 3)]
fn r6001_greymonds_humans_have_the_two_chosen_abilities(
    #[case] seats: usize,
    #[case] picks: [usize; 2],
    #[case] humans: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut board: Vec<_> = (0..4).map(|_| ready(plains())).collect();
        board.extend((0..humans).map(|n| ready(r6001_human(&format!("Mine {n}")))));
        board.push(ready(free_creature("Beast", 1, 1)));
        let mut scenario = Scenario::new(seats)
            .at(1, 0, mtg_engine::turn::Step::Main1)
            .hand(0, [card(GREYMOND).unwrap()])
            .battlefield(0, board);
        for seat in 1..seats {
            scenario = scenario.battlefield(seat, [ready(r6001_human(&format!("Theirs {seat}")))]);
        }
        scenario
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast(GREYMOND)?;
    let mut offered = Vec::new();
    let mut taken: Vec<&str> = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let want = ABILITIES[picks[taken.len()]].0;
                offered.push(options.options.len());
                let at = options
                    .options
                    .iter()
                    .position(|o| *o == ChoiceOption::Label(want.to_owned()))
                    .expect("the ability is offered");
                taken.push(want);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            _ => driver.answer_passively()?,
        }
    }
    assert_eq!(offered, [3, 2], "two different abilities are asked for");
    let id = driver.id_of(GREYMOND)?;
    let held = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(
        held,
        Some(Chosen::Words(
            taken.iter().map(|w| (*w).to_owned()).collect()
        ))
    );
    let bonus = if humans + 1 >= 4 { 2 } else { 0 };
    for (at, (_, keyword)) in ABILITIES.iter().enumerate() {
        let chosen = picks.contains(&at);
        for n in 0..humans {
            let mine = driver.id_of(&format!("Mine {n}"))?;
            assert_eq!(
                has_keyword(driver.state(), mine, keyword),
                chosen,
                "Mine {n}: {keyword:?}"
            );
            let power = characteristics(driver.state(), mine)
                .unwrap()
                .power
                .unwrap();
            assert_eq!(power, 1 + bonus);
        }
        assert_eq!(
            has_keyword(driver.state(), id, keyword),
            chosen,
            "Greymond: {keyword:?}"
        );
        let beast = driver.id_of("Beast")?;
        assert!(!has_keyword(driver.state(), beast, keyword));
        for seat in 1..seats {
            let theirs = driver.id_of(&format!("Theirs {seat}"))?;
            assert!(!has_keyword(driver.state(), theirs, keyword));
            let power = characteristics(driver.state(), theirs)
                .unwrap()
                .power
                .unwrap();
            assert_eq!(power, 1);
        }
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
