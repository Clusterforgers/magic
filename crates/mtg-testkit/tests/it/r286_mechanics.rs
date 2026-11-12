//! Goal: the corpus cards of pack r28-6 play as printed at tables of two, three and four seats: a Guest creature has its
//! keyword only while its controller controls a stickered permanent; damage to a creature that turns into counters
//! removed is prevented whole and removes that many +1/+1 counters, only while the condition of the shield holds; damage
//! to a player that turns into a mill is prevented and mills twice that many cards; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Sticker};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{bear, cost, un34_sheet, un34_stickerer};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

/// Passes, picking the first target and option offered, until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(_) => {
                driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// A free instant with the one spell ability given as JSON.
fn spell(name: &str, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

/// A free instant that deals `amount` damage to target creature.
fn zap(amount: i32) -> Arc<CardDefinition> {
    spell(
        "Zap",
        &format!(
            r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
        ),
    )
}

fn plus_ones(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne))
}

#[rstest]
#[case::trample("Big Winner", Keyword::Trample)]
#[case::flying("Croakid Amphibonaut", Keyword::Flying)]
#[case::lifelink("Sanguine Sipper", Keyword::Lifelink)]
#[case::menace("Scared Stiff", Keyword::Menace)]
#[case::vigilance("Grabby Tabby", Keyword::Vigilance)]
fn a_guest_has_its_keyword_once_its_controller_controls_a_stickered_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[case] guest: &str,
    #[case] keyword: Keyword,
) -> Result<(), ScriptError> {
    let table = || {
        let mut scenario = Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .stickers(
                0,
                [un34_sheet(
                    "Fun Sheet",
                    vec![Sticker::Name {
                        words: "Wicked".to_owned(),
                        tickets: 1,
                    }],
                )],
            )
            .counters(0, CounterKind::Other("ticket".to_owned()), 1)
            .hand(0, [un34_stickerer("Stickerer")])
            .battlefield(0, [ready(card(guest).unwrap())]);
        for seat in 1..seats {
            scenario = scenario.battlefield(seat, [ready(card(guest).unwrap())]);
        }
        scenario
    };
    let mut driver = Driver::from_scenario(table())?;
    let guests: Vec<_> = driver.state().zone(Zone::Battlefield).to_vec();
    assert!(
        guests
            .iter()
            .all(|id| !has_keyword(driver.state(), *id, &keyword))
    );
    driver.cast("Stickerer")?;
    settle(&mut driver)?;
    for id in guests {
        let mine = driver.state().object(id).unwrap().controller() == pid(0);
        assert_eq!(has_keyword(driver.state(), id, &keyword), mine);
    }
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
#[case::hydra("Protean Hydra", 3, 2, 1)]
#[case::hydra_dies("Protean Hydra", 2, 3, 0)]
#[case::polukranos("Polukranos, Unchained", 6, 2, 4)]
#[case::conjurant("Ugin's Conjurant", 3, 1, 2)]
fn damage_to_the_creature_removes_that_many_counters_instead(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] counters: u32,
    #[case] amount: i32,
    #[case] left: u32,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [with_counters(
                    ready(card(name).unwrap()),
                    CounterKind::PlusOnePlusOne,
                    counters,
                )],
            )
            .hand(0, [zap(amount)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_cast("Zap")?;
    settle(&mut driver)?;
    if left == 0 {
        // A 0/0 with no counters left is put into its owner's graveyard.
        assert!(driver.id_of(name).is_err() || plus_ones(&driver, name) == 0);
    } else {
        let id = driver.id_of(name)?;
        assert_eq!(plus_ones(&driver, name), left);
        assert_eq!(
            driver
                .state()
                .object(id)
                .unwrap()
                .permanent()
                .unwrap()
                .damage(),
            0
        );
    }
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
fn the_shield_of_a_counter_holder_needs_a_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // A Polukranos with no counters is 0/0, so an anthem keeps it alive: the Zap of 2 marks damage as usual.
    let ability: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":5,"toughness":5}}]}}"#,
    )
    .unwrap();
    let anthem = Arc::new(
        CardDefinition::new("Anthem")
            .with_type(CardType::Enchantment)
            .with_rules(ability),
    );
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(anthem.clone()),
                    ready(card("Polukranos, Unchained").unwrap()),
                ],
            )
            .hand(0, [zap(2)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_cast("Zap")?;
    settle(&mut driver)?;
    let id = driver.id_of("Polukranos, Unchained")?;
    assert_eq!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        2
    );
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
fn damage_to_the_angels_controller_mills_twice_that_many(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] amount: i32,
) -> Result<(), ScriptError> {
    let burn = spell(
        "Burn",
        &format!(
            r#"{{"Spell":{{"targets":[{{"what":"Player"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
        ),
    );
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(card("Angel of Suffering").unwrap())])
            .library(0, (0..10).map(|_| bear()))
            .hand(0, [burn.clone()])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_cast("Burn")?;
    // The first legal player is the caster.
    settle(&mut driver)?;
    let me = driver.state().player(pid(0)).unwrap();
    assert_eq!(me.life(), 20);
    let milled = driver.state().zone(Zone::Graveyard(pid(0))).len();
    // The burn spell is in the graveyard too.
    assert_eq!(milled, 1 + 2 * usize::try_from(amount).unwrap());
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

/// Whether the attack of the named creatures of seat 0, each at seat 1, is a legal declaration.
fn attack_is_legal(driver: &Driver, names: &[&str]) -> bool {
    let DecisionKind::DeclareAttackers(options) = &driver.decision().unwrap().kind else {
        panic!("expected attackers");
    };
    let to = mtg_engine::combat::AttackTarget::Player(pid(1));
    let attack: Vec<_> = names
        .iter()
        .map(|name| (driver.id_of(name).unwrap(), to))
        .collect();
    options.accepts(&attack)
}

fn colored(name: &str, power: i32, color: mtg_core::color::Color) -> Arc<CardDefinition> {
    let card = (*mtg_testkit::cards::free_creature(name, power, 2)).clone();
    Arc::new(card.with_color_indicator(mtg_core::color::ColorSet::EMPTY.with(color)))
}

#[rstest]
#[case::okk_alone("Okk", &["Okk"], false)]
#[case::okk_with_smaller("Okk", &["Okk", "Small"], false)]
#[case::okk_with_greater("Okk", &["Okk", "Big"], true)]
#[case::conscripts_with_one("Orcish Conscripts", &["Orcish Conscripts", "Big"], false)]
#[case::conscripts_with_two("Orcish Conscripts", &["Orcish Conscripts", "Big", "Small"], true)]
#[case::puma_with_white("Scarred Puma", &["Scarred Puma", "Small"], false)]
#[case::puma_with_green("Scarred Puma", &["Scarred Puma", "Big"], true)]
#[case::master_alone("Master of Cruelties", &["Master of Cruelties"], true)]
#[case::master_with_another("Master of Cruelties", &["Master of Cruelties", "Big"], false)]
#[case::others_without_the_master("Master of Cruelties", &["Big", "Small"], true)]
fn a_creature_attacks_only_beside_the_others_its_rule_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] bound: &str,
    #[case] attack: &[&str],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    use mtg_core::color::Color;
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(card(bound).unwrap()),
                ready(colored("Big", 5, Color::Green)),
                ready(colored("Small", 1, Color::White)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(attack_is_legal(&driver, attack), legal);
    // The automatic declaration is always one the check accepts.
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("expected attackers");
    };
    let automatic = options.legal_attack();
    assert!(options.accepts(&automatic));
    let before = driver.engine().state_hash();
    let to = mtg_engine::combat::AttackTarget::Player(pid(1));
    let ids: Vec<_> = attack
        .iter()
        .map(|name| (driver.id_of(name).unwrap(), to))
        .collect();
    let answered = driver.answer(Answer::DeclareAttackers(ids));
    assert_eq!(answered.is_ok(), legal);
    if !legal {
        assert_eq!(driver.engine().state_hash(), before);
    }
    Ok(())
}

#[rstest]
#[case::okk_alone("Okk", &["Okk"], false)]
#[case::okk_with_greater("Okk", &["Okk", "Big"], true)]
#[case::conscripts_with_one("Orcish Conscripts", &["Orcish Conscripts", "Big"], false)]
#[case::conscripts_with_two("Orcish Conscripts", &["Orcish Conscripts", "Big", "Small"], true)]
fn a_creature_blocks_only_beside_the_others_its_rule_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] bound: &str,
    #[case] blockers: &[&str],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    use mtg_core::color::Color;
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(colored("Raider", 3, Color::Red))])
        .battlefield(
            1,
            [
                ready(card(bound).unwrap()),
                ready(colored("Big", 5, Color::Green)),
                ready(colored("Small", 1, Color::White)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider", 1)])?;
    while !matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
        driver.pass()?;
    }
    let raider = driver.id_of("Raider")?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("expected blockers");
    };
    let blocks: Vec<_> = blockers
        .iter()
        .map(|name| (driver.id_of(name).unwrap(), raider))
        .collect();
    assert_eq!(options.accepts(&blocks), legal);
    assert!(options.accepts(&options.legal_blocks()));
    Ok(())
}

#[rstest]
fn port_razer_attacks_each_player_once_a_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Port Razer").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Port Razer", 1)])?;
    // Its damage untaps it and adds a combat phase: in that one it may attack only the players it has not attacked.
    for _ in 0..400 {
        if driver.state().turn().number() > 2 {
            // At two seats the only opponent was attacked already: the second combat asks nothing.
            assert_eq!(seats, 2);
            return Ok(());
        }
        if let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind {
            let defenders: Vec<_> = options
                .attackers
                .iter()
                .flat_map(|option| option.defenders.clone())
                .collect();
            let expected: Vec<_> = (2..seats)
                .map(|seat| mtg_engine::combat::AttackTarget::Player(pid(seat)))
                .collect();
            assert_eq!(defenders, expected);
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("no second combat".to_owned()))
}

#[rstest]
fn the_wall_of_shadows_takes_no_damage_from_the_creature_it_blocks(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Wall of Shadows").unwrap())])
        .battlefield(
            1,
            [ready(mtg_testkit::cards::free_creature("Raider", 3, 3))],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider", 0)])?;
    while !matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
        driver.pass()?;
    }
    driver.block(&[("Wall of Shadows", "Raider")])?;
    driver.idle_until(2, Step::EndCombat)?;
    let wall = driver.id_of("Wall of Shadows")?;
    assert_eq!(
        driver
            .state()
            .object(wall)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        0
    );
    Ok(())
}

#[rstest]
#[case::cast_from_hand(true, 0)]
#[case::returned_from_the_graveyard(false, 3)]
fn epochrasite_enters_with_counters_unless_cast_from_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] cast: bool,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let raise = spell(
        "Raise",
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#,
    );
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..4).map(|_| ready(mtg_testkit::cards::forest())));
    scenario = if cast {
        scenario.hand(0, [card("Epochrasite").unwrap()])
    } else {
        scenario
            .graveyard(0, [card("Epochrasite").unwrap()])
            .hand(0, [raise])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(if cast { "Epochrasite" } else { "Raise" })?;
    settle(&mut driver)?;
    assert_eq!(plus_ones(&driver, "Epochrasite"), counters);
    Ok(())
}

#[rstest]
fn dire_wolves_band_while_their_controller_has_a_plains(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] plains: bool,
) -> Result<(), ScriptError> {
    let mut permanents = vec![ready(card("Dire Wolves").unwrap())];
    if plains {
        permanents.push(ready(mtg_testkit::cards::plains()));
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, permanents);
    let driver = Driver::from_scenario(scenario)?;
    let wolves = driver.id_of("Dire Wolves")?;
    assert_eq!(
        has_keyword(driver.state(), wolves, &Keyword::Banding),
        plains
    );
    Ok(())
}
