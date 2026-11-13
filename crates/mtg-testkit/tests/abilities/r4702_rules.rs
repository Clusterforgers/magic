//! Goal: an emblem made after a creature type was chosen keeps that type in its abilities after the resolution (only
//! creatures of that type get the bonus, also ones that enter later), and a player whose opponents' spells "can't cause
//! you to discard cards or sacrifice permanents" keeps their cards and permanents against an opponent's spell while the
//! other players and that player's own spell still make them discard and sacrifice, at tables of two, three and four
//! seats, with the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::ChoiceOption;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn sorcery(name: &str, effects: &str) -> Arc<CardDefinition> {
    let spell = format!(r#"{{"Spell":{{"effects":{effects}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(&spell).unwrap()),
    )
}

fn typed(name: &str, subtype: &str) -> Arc<CardDefinition> {
    let card = creature(name, 0, &[], 1, 1);
    Arc::new(Arc::unwrap_or_clone(card).with_subtype(subtype))
}

/// Plays on until the stack is empty, choosing `pick` wherever a list of words is offered and passing otherwise.
fn play_out(driver: &mut Driver, pick: &str) -> Result<(), ScriptError> {
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
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| matches!(option, ChoiceOption::Label(word) if word == pick))
                    .expect("the word is offered");
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    mtg_engine::characteristics::characteristics(driver.state(), id)
        .unwrap()
        .power
        .unwrap()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

const EMBLEM: &str = r#"[{"ChooseValue":{"kind":"CreatureType","as":"type"}},{"CreateEmblem":{"abilities":[{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"ChosenAs":"type"}]},"mods":[{"ModifyPT":{"power":3,"toughness":3}}]}}]}}]"#;

#[rstest]
#[case::elves("Elf", 4, 1)]
#[case::goblins("Goblin", 1, 4)]
fn an_emblem_keeps_the_type_chosen_before_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: &str,
    #[case] elf: i32,
    #[case] goblin: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [sorcery("Decree", EMBLEM), typed("Late Elf", "Elf")])
            .battlefield(
                0,
                [typed("Elf", "Elf"), typed("Goblin", "Goblin")]
                    .into_iter()
                    .map(ready),
            )
            .battlefield(1, [ready(typed("Their Elf", "Elf"))]),
    )?;
    driver.cast("Decree")?;
    play_out(&mut driver, chosen)?;
    assert_eq!(
        (power(&driver, "Elf"), power(&driver, "Goblin")),
        (elf, goblin)
    );
    // An opponent's creature of the type is not "you control".
    assert_eq!(power(&driver, "Their Elf"), 1);
    driver.cast("Late Elf")?;
    play_out(&mut driver, chosen)?;
    assert_eq!(power(&driver, "Late Elf"), elf);
    replays(&driver);
    Ok(())
}

const EACH_DISCARDS: &str = r#"[{"Discard":{"who":"EachPlayer"}},{"Sacrifice":{"who":"EachPlayer","filter":[{"Type":"Creature"}]}}]"#;

fn ward() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Ward")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[]))
            .with_rules(
                serde_json::from_str(
                    r#"{"Rule":{"rule":{"OpponentsCantMakeDiscardOrSacrifice":"You"}}}"#,
                )
                .unwrap(),
            ),
    )
}

/// Seat 0 has the rule; `caster` casts a spell that makes each player discard a card and sacrifice a creature.
#[rstest]
#[case::an_opponents_spell_spares_them(1, false)]
#[case::their_own_spell_does_not(0, true)]
fn a_spared_player_keeps_cards_against_opponents_only(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] spared_loses: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, caster, Step::Main1);
    for seat in 0..seats {
        let mut hand = vec![forest()];
        if seat == caster {
            hand.push(sorcery("Edict", EACH_DISCARDS));
        }
        let mut board = vec![ready(creature("Bear", 0, &[Color::Green], 2, 2))];
        if seat == 0 {
            board.push(ready(ward()));
        }
        scenario = scenario.hand(seat, hand).battlefield(seat, board);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Edict")?;
    play_out(&mut driver, "")?;
    for seat in 0..seats {
        let hand = driver.state().zone(Zone::Hand(pid(seat))).len();
        let bears = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                let object = driver.state().object(**id).unwrap();
                object.owner() == pid(seat) && object.card().name == "Bear"
            })
            .count();
        let loses = seat != 0 || spared_loses;
        assert_eq!(
            (hand, bears),
            if loses { (0, 0) } else { (1, 1) },
            "seat {seat}"
        );
    }
    replays(&driver);
    Ok(())
}
