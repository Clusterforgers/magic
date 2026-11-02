//! Goal: the Soldier-token fixture the web grouping tests read is what the engine projects for tokens made on different turns and given the same size and keywords by effects registered in different orders, while attacking and after combat.
//!
//! Regenerate with `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol stack_fixture::`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_protocol::StateView;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

const FIXTURE: &str = "web/src/protocol/fixtures/soldier-stacks.json";
const DESCRIPTION: &str = "Four Soldier tokens of Hero of Bladehold from the real engine, two made on turn 2 and two on \
    turn 4, each pair made 6/6 with vigilance and lifelink by effects registered in a different order. `attacking` is turn 4 with the second pair attacking, `idle` is turn 6 before combat. Regenerate: MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol \
    stack_fixture::";

fn soldiers(driver: &Driver) -> Vec<ObjectId> {
    let mut found: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
        .collect();
    found.sort();
    found
}

fn resolve_everything(driver: &mut Driver) -> Result<(), ScriptError> {
    loop {
        let ordering = matches!(
            driver.decision()?.kind,
            mtg_engine::decision::DecisionKind::OrderTriggers(_)
        );
        if ordering {
            driver.order_triggers(&[0, 1])?;
        } else if driver.state().stack().is_empty() {
            return Ok(());
        } else {
            driver.pass()?;
        }
    }
}

fn attack_with_hero(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.attack(&[("Hero of Bladehold", 1)])?;
    resolve_everything(driver)
}

fn make_big(driver: &mut Driver, ids: &[ObjectId], reversed: bool) -> Result<(), ScriptError> {
    let mut effects = vec![
        Modification::SetPowerToughness {
            power: 6,
            toughness: 6,
        },
        Modification::AddKeyword(Keyword::Vigilance),
        Modification::AddKeyword(Keyword::Lifelink),
    ];
    if reversed {
        effects.reverse();
    }
    let selector = Selector::objects(driver.state(), ids.iter().copied());
    for effect in effects {
        driver.add_effect(selector.clone(), effect, Duration::Forever)?;
    }
    Ok(())
}

fn stage(driver: &Driver) -> String {
    serde_json::to_string(&StateView::for_player(driver.state(), PlayerId(0))).unwrap()
}

fn render() -> String {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .life(1, 500)
            .battlefield(0, [ready(card("Hero of Bladehold").unwrap())]),
    )
    .unwrap();
    attack_with_hero(&mut driver).unwrap();
    let first = soldiers(&driver);
    make_big(&mut driver, &first, false).unwrap();
    driver.idle_until(4, Step::DeclareAttackers).unwrap();
    attack_with_hero(&mut driver).unwrap();
    let all = soldiers(&driver);
    let second: Vec<ObjectId> = all
        .iter()
        .copied()
        .filter(|id| !first.contains(id))
        .collect();
    make_big(&mut driver, &second, true).unwrap();
    let attacking = stage(&driver);
    driver.pass_until(Step::Main2).unwrap();
    driver.idle_until(6, Step::Upkeep).unwrap();
    let idle = stage(&driver);
    format!(
        "{{\n  \"description\": {},\n  \"tokens\": {},\n  \"attacking\": {attacking},\n  \"idle\": {idle}\n}}\n",
        serde_json::to_string(DESCRIPTION).unwrap(),
        serde_json::to_string(&all).unwrap(),
    )
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FIXTURE)
}

#[test]
fn the_committed_fixture_is_what_the_engine_projects() {
    let fresh = render();
    let path = fixture_path();
    if std::env::var_os("MTG_UPDATE_BINDINGS").is_some() {
        fs::write(&path, &fresh).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == fresh,
        "the stack fixture drifted; run MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol stack_fixture::"
    );
}
