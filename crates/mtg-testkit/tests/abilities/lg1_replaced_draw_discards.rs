//! Goal: a draw that two replacements change, Breathstealer's Crypt ("draw it, reveal it, discard a creature card unless
//! 3 life is paid") and Zur's Weirding ("reveal it instead; another player may pay 2 life to put it into the graveyard"),
//! in either order and with either payment, at 2 to 4 seats: a card Zur's Weirding puts into the graveyard never reaches
//! the hand and is not discarded (CR 616.1, 701.9a), a card drawn is discarded only as the Crypt says, and the log replays
//! to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{ReplacementChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const CRYPT: &str = "Breathstealer's Crypt";
const WEIRDING: &str = "Zur's Weirding";

/// An artifact sacrificed at no mana: "Draw a card."
fn study() -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Draw":{}}]}}"#)
            .unwrap();
    Arc::new(
        CardDefinition::new("Study")
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

/// Draws with Study, applying `first` first; seat 0 answers the Crypt's 3 life with `crypt_pays`, the others the
/// Weirding's 2 life with `weirding_pays`.
fn draw(
    driver: &mut Driver,
    first: &str,
    crypt_pays: bool,
    weirding_pays: bool,
) -> Result<(), ScriptError> {
    driver.begin_ability("Study", 0)?;
    for _ in 0..80 {
        let state = driver.state();
        let busy = !state.stack().is_empty() || !state.pending_triggers().is_empty();
        match &driver.decision()?.kind {
            DecisionKind::ChooseReplacement(options) => {
                let at = options
                    .candidates
                    .iter()
                    .position(|c| name_of(driver.state(), c.source) == first)
                    .unwrap();
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(at)))?;
            }
            DecisionKind::YesNo(options) => {
                let player = driver.decision()?.player;
                let yes = !options.only_no
                    && if player == pid(0) {
                        crypt_pays
                    } else {
                        weirding_pays
                    };
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the draw".into()))
}

#[rstest]
fn lg1_a_card_kept_from_the_hand_is_not_discarded(
    #[values(2, 3, 4)] seats: usize,
    #[values(CRYPT, WEIRDING)] first: &str,
    #[values(false, true)] crypt_pays: bool,
    #[values(false, true)] weirding_pays: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card(CRYPT).unwrap()),
                ready(card(WEIRDING).unwrap()),
                ready(study()),
            ],
        )
        .library(0, [free_creature("Drawn", 2, 2), plains()]);
    let mut driver = Driver::from_scenario(scenario)?;
    draw(&mut driver, first, crypt_pays, weirding_pays)?;
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    let kept = !weirding_pays && crypt_pays;
    assert_eq!(
        hand,
        if kept {
            vec!["Drawn".to_owned()]
        } else {
            Vec::new()
        }
    );
    assert_eq!(
        graveyard.contains(&"Drawn".to_owned()),
        !kept,
        "{graveyard:?}"
    );
    assert_eq!(
        names(driver.state(), Zone::Library(pid(0))),
        vec!["Plains".to_owned()]
    );
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!(life(1), if weirding_pays { 18 } else { 20 });
    if !weirding_pays {
        assert_eq!(life(0), if crypt_pays { 17 } else { 20 });
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
