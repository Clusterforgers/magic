//! Goal: one pick of each kind for each player ("chooses a creature and a land they control, then sacrifices the rest") and
//! each player taking back the permanents they own both work at tables of two, three and four seats, with the log replaying
//! to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free sorcery with these effects.
fn sorcery(name: &str, effects: &str) -> Arc<CardDefinition> {
    let json = format!(r#"{{"Spell":{{"effects":{effects}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

/// Plays on until the stack is empty, taking the first cards offered.
fn play_out(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseCards(options) => {
                let picked = options.from[..options.min as usize].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The creatures (or the lands) on the battlefield that this player controls.
fn controlled(driver: &Driver, seat: usize, creature: bool) -> usize {
    driver
        .state()
        .objects()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && characteristics::controller_of(driver.state(), object.id())
                    .is_some_and(|player| player.index() == seat)
                && (object.card().name != "Forest") == creature
        })
        .count()
}

/// What the translator writes for "each player chooses a creature and a land they control, then sacrifices the rest".
const KEEP_ONE_OF_EACH: &str = r#"[
    {"Choose":{"as":"c1","from":{"Each":[{"ControlledBy":"Chooser"},{"Type":"Creature"}]},"who":"EachPlayer"}},
    {"Choose":{"as":"c2","from":{"Each":[{"ControlledBy":"Chooser"},{"Type":"Land"},{"Not":{"Is":{"Bound":"c1"}}}]},"who":"EachPlayer"}},
    {"Bind":{"as":"picks","what":{"Each":[{"AnyOf":[{"Is":{"Bound":"c1"}},{"Is":{"Bound":"c2"}}]},
        {"AnyOf":[{"In":"Battlefield"},{"In":"Hand"},{"In":"Library"},{"In":"Graveyard"},{"In":"Exile"},{"In":"Stack"}]}]}}},
    {"Sacrifice":{"count":{"Count":[{"Not":{"Is":{"Bound":"picks"}}}]},"filter":[{"Not":{"Is":{"Bound":"picks"}}}],"who":"EachPlayer"}}]"#;

#[rstest]
fn each_player_keeps_one_creature_and_one_land(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).hand(0, [sorcery("Keep", KEEP_ONE_OF_EACH)]);
    for seat in 0..seats {
        scenario = scenario.battlefield(
            seat,
            [
                free_creature("Bear", 2, 2),
                free_creature("Elf", 1, 1),
                forest(),
                forest(),
            ]
            .into_iter()
            .map(ready),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Keep")?;
    play_out(&mut driver)?;
    for seat in 0..seats {
        assert_eq!(
            controlled(&driver, seat, true),
            1,
            "creatures of seat {seat}"
        );
        assert_eq!(controlled(&driver, seat, false), 1, "lands of seat {seat}");
    }
    replays(&driver);
    Ok(())
}

/// Every creature comes under the caster's control for good.
const STEAL_ALL: &str = r#"[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},
    "mods":[{"SetController":"You"}],"duration":"Forever"}}]"#;

/// What the translator writes for "each player gains control of all creatures they own".
const RETURN_TO_OWNERS: &str = r#"[{"RepeatEach":{"over":"EachPlayer","as":"owner","effects":[
    {"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"OwnedBy":{"Bound":"owner"}}]},
    "mods":[{"SetController":{"Bound":"owner"}}],"duration":"Forever"}}]}}]"#;

#[rstest]
fn each_player_takes_back_the_creatures_they_own(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).hand(
        0,
        [
            sorcery("Steal", STEAL_ALL),
            sorcery("Return", RETURN_TO_OWNERS),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bear", 2, 2))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Steal")?;
    play_out(&mut driver)?;
    assert_eq!(controlled(&driver, 0, true), seats);
    driver.cast("Return")?;
    play_out(&mut driver)?;
    for seat in 0..seats {
        assert_eq!(
            controlled(&driver, seat, true),
            1,
            "creatures of seat {seat}"
        );
    }
    replays(&driver);
    Ok(())
}
