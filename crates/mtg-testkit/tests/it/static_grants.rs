//! Goal: the statics that give a group a bonus, a keyword or a base size (a Merfolk lord with islandwalk, a Zombie lord's swampwalk for every other Zombie, a subtype taken out of a base size, creature tokens during their controller's turn) reach exactly the objects their words name at two to four seats, end when the source leaves, stack with another lord in the order of the layers, and replay to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::keywords::keywords_of;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::{GameSetup, characteristics, replay};
use mtg_testkit::cards::{bear, burn, cost, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SPARK: &str = "Spark";

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// Whether the permanent has a landwalk keyword.
fn walks(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .any(|held| held.name() == "Landwalk")
}

/// A creature of one subtype with nothing else.
fn kind(name: &str, subtype: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(power, toughness),
    )
}

/// Seat 0 casts a burn spell of `damage` at the start of its first main phase; each seat has a library to draw from.
fn table(seats: usize, lists: Vec<Vec<PresetPermanent>>, damage: i32) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for (seat, mut list) in lists.into_iter().enumerate() {
        if seat == 0 {
            list.push(ready(mountain()));
        }
        scenario = scenario
            .battlefield(seat, list)
            .library(seat, vec![bear(), bear(), bear(), bear()]);
    }
    scenario
        .hand(0, vec![burn(SPARK, damage)])
        .at(1, 0, Step::Main1)
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

/// Seat 0 burns the named permanent with the spell in its hand.
fn burn_it(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let at = driver.target_named(name)?;
    driver.cast_at(SPARK, &[at])?;
    settle(driver)
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The other seats each control one creature of a subtype of their own.
fn foes(seats: usize, subtype: &str, power: i32, toughness: i32) -> Vec<Vec<PresetPermanent>> {
    (1..seats)
        .map(|seat| {
            vec![ready(kind(
                &format!("Foe {subtype} {seat}"),
                subtype,
                power,
                toughness,
            ))]
        })
        .collect()
}

#[rstest]
fn a_merfolk_lord_changes_the_other_merfolk_you_control_until_it_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut lists = vec![vec![
        ready(card("Master of the Pearl Trident").unwrap()),
        ready(card("Merfolk Mistbinder").unwrap()),
    ]];
    lists.extend(foes(seats, "Merfolk", 1, 1));
    let game = || table(seats, lists.clone(), 3);
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    // Each lord gives the other +1/+1; only the Trident gives islandwalk; their Merfolk are out of reach.
    assert_eq!(stats(&driver, "Merfolk Mistbinder"), (3, 3));
    assert!(walks(&driver, "Merfolk Mistbinder"));
    assert_eq!(stats(&driver, "Master of the Pearl Trident"), (3, 3));
    assert!(!walks(&driver, "Master of the Pearl Trident"));
    for seat in 1..seats {
        let foe = format!("Foe Merfolk {seat}");
        assert_eq!(stats(&driver, &foe), (1, 1), "{foe}");
        assert!(!walks(&driver, &foe), "{foe}");
    }
    burn_it(&mut driver, "Master of the Pearl Trident")?;
    assert!(driver.id_of("Master of the Pearl Trident").is_err());
    assert_eq!(stats(&driver, "Merfolk Mistbinder"), (2, 2));
    assert!(!walks(&driver, "Merfolk Mistbinder"));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_zombie_lord_gives_swampwalk_to_every_other_zombie_whoever_controls_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut lists = vec![vec![
        ready(card("Zombie Master").unwrap()),
        ready(kind("Walker", "Zombie", 1, 1)),
        ready(bear()),
    ]];
    lists.extend(foes(seats, "Zombie", 1, 1));
    let game = || table(seats, lists.clone(), 3);
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    assert!(!walks(&driver, "Zombie Master"), "the lord is not another");
    assert!(!walks(&driver, "Grizzly Bears"));
    assert!(walks(&driver, "Walker"));
    for seat in 1..seats {
        assert!(walks(&driver, &format!("Foe Zombie {seat}")), "seat {seat}");
    }
    burn_it(&mut driver, "Zombie Master")?;
    assert!(!walks(&driver, "Walker"));
    for seat in 1..seats {
        assert!(
            !walks(&driver, &format!("Foe Zombie {seat}")),
            "seat {seat}"
        );
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::alone(false, (3, 3))]
#[case::under_a_lord_of_merfolk(true, (4, 4))]
fn a_base_size_sets_every_creature_but_the_subtype_before_the_bonuses_apply(
    #[case] lord: bool,
    #[case] merfolk: (i32, i32),
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Harmonious Archon").unwrap()), ready(bear())];
    if lord {
        mine.push(ready(card("Master of the Pearl Trident").unwrap()));
        mine.push(ready(card("Merfolk Mistbinder").unwrap()));
    }
    let mut lists = vec![mine];
    lists.extend(foes(seats, "Elf", 1, 1));
    let game = || table(seats, lists.clone(), 5);
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    assert_eq!(
        stats(&driver, "Harmonious Archon"),
        (4, 5),
        "an Archon keeps its size"
    );
    assert_eq!(stats(&driver, "Grizzly Bears"), (3, 3));
    for seat in 1..seats {
        assert_eq!(stats(&driver, &format!("Foe Elf {seat}")), (3, 3));
    }
    if lord {
        // The base size comes first, then each Merfolk lord adds +1/+1.
        assert_eq!(stats(&driver, "Merfolk Mistbinder"), merfolk);
        assert_eq!(stats(&driver, "Master of the Pearl Trident"), merfolk);
    }
    burn_it(&mut driver, "Harmonious Archon")?;
    assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2));
    for seat in 1..seats {
        assert_eq!(stats(&driver, &format!("Foe Elf {seat}")), (1, 1));
    }
    replays(&driver, &setup);
    Ok(())
}

/// A sorcery that creates a 1/1 Spirit token.
fn spirit_maker() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1}}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Spirit Maker")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

#[rstest]
fn creature_tokens_grow_during_their_controllers_turn_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let mut list = vec![ready(bear())];
        if seat == 0 {
            list.push(ready(card("Prava of the Steel Legion").unwrap()));
        }
        scenario = scenario
            .battlefield(seat, list)
            .library(seat, vec![bear(), bear(), bear(), bear()]);
    }
    let game = || {
        scenario
            .clone()
            .hand(0, vec![spirit_maker()])
            .at(1, 0, Step::Main1)
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    driver.cast("Spirit Maker")?;
    settle(&mut driver)?;
    // On its controller's turn the token is 2/5; the bears and the Cat are not tokens.
    assert_eq!(stats(&driver, "Spirit"), (2, 5));
    assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2));
    // On the next turn (another seat's) the bonus is gone.
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(stats(&driver, "Spirit"), (1, 1));
    // And back on its controller's turn, one round of the table later.
    let round = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(round, Step::Main1)?;
    assert_eq!(stats(&driver, "Spirit"), (2, 5));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::the_permission_is_yours_and_used(0, true)]
#[case::the_permission_is_yours_and_not_used(0, false)]
#[case::the_permission_is_theirs(1, true)]
fn a_permission_to_play_forests_from_the_graveyard_is_for_its_controller_and_for_forests_only(
    #[case] holder: usize,
    #[case] play: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let game = || {
        let mut scenario = Scenario::new(seats)
            .battlefield_named(holder, ["Titania, Nature's Force"])
            .graveyard(0, [card("Forest").unwrap(), mountain()])
            .at(1, 0, Step::Main1);
        for seat in 0..seats {
            scenario = scenario.library(seat, vec![bear(), bear(), bear(), bear()]);
        }
        scenario
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    // A Mountain is never offered, whoever holds the permission.
    assert!(driver.play_land("Mountain").is_err());
    if holder != 0 {
        assert!(
            driver.play_land("Forest").is_err(),
            "their permission is not mine"
        );
    } else if play {
        driver.play_land("Forest")?;
        settle(&mut driver)?;
        assert!(
            driver.id_of("Forest").is_ok(),
            "the Forest is on the battlefield"
        );
        // Titania's trigger: a Forest entering under its controller makes a 5/3 Elemental.
        assert_eq!(stats(&driver, "Elemental"), (5, 3));
        assert!(
            driver.play_land("Mountain").is_err(),
            "the land play is spent"
        );
    } else {
        assert!(driver.id_of("Elemental").is_err());
    }
    replays(&driver, &setup);
    Ok(())
}
