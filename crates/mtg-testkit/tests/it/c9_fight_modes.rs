//! Goal: the compiled readings of Dromoka's Command, Epic Fight and Savage Stomp play at tables of two, three and four seats: "Choose two" and "Choose one or both" run the chosen modes in printed order with a fight among their own targets, a shield from a target spell prevents only that spell's damage (CR 615, 611.2c), doubling counts the power and toughness as it resolves (CR 701.10d), "costs {2} less to cast if it targets a Dinosaur you control" spends less mana only for such a target, and every log replays to the live state.

use std::error::Error;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::choose::{ModeChoice, Target};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, keyworded, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::fight::{Fate, MINE, THEIRS, fate, life, replays, settle, table};

type C9FightResult = Result<(), Box<dyn Error>>;

/// A vanilla creature of this size.
fn c9_fight_body(name: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    keyworded(name, power, toughness, &[])
}

/// The table of `fight::table` with `spell` in seat 0's hand (and two Bolts), `mine` for seat 0 and `theirs` for the last
/// seat.
fn c9_fight_game(
    seats: usize,
    spell: &str,
    mine: (i32, i32),
    theirs: (i32, i32),
) -> Result<Scenario, Box<dyn Error>> {
    Ok(table(
        seats,
        seats - 1,
        vec![card(spell)?, burn("Bolt", 3), burn("Bolt", 3)],
        c9_fight_body(MINE, mine.0, mine.1),
        c9_fight_body(THEIRS, theirs.0, theirs.1),
    ))
}

/// Casts `spell` with these modes and, for each target specification of the modes in order, these targets.
fn c9_fight_cast(
    driver: &mut Driver,
    spell: &str,
    modes: Vec<usize>,
    targets: &[Vec<Target>],
) -> C9FightResult {
    driver.begin_cast(spell)?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes)))?;
    for chosen in targets {
        driver.choose_targets(chosen)?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

/// Dromoka's Command, the counter and the fight modes: the counter lands first, then the creatures fight.
#[rstest]
#[case::the_counter_on_mine_trades(2, MINE, Fate::Dead, Fate::Dead)]
#[case::the_counter_on_theirs_wins(3, THEIRS, Fate::Dead, Fate::Marked(2))]
#[case::four_seats(4, MINE, Fate::Dead, Fate::Dead)]
fn c9_fight_dromokas_command_puts_the_counter_before_the_fight(
    #[case] seats: usize,
    #[case] counter_on: &str,
    #[case] mine_fate: Fate,
    #[case] theirs_fate: Fate,
) -> C9FightResult {
    let game = || c9_fight_game(seats, "Dromoka's Command", (2, 2), (3, 3));
    let setup = game()?.setup()?;
    let mut driver = Driver::from_scenario(game()?)?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let targets = vec![
        vec![driver.target_named(counter_on)?],
        vec![driver.target_named(MINE)?],
        vec![driver.target_named(THEIRS)?],
    ];
    c9_fight_cast(&mut driver, "Dromoka's Command", vec![2, 3], &targets)?;
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine), mine_fate, "mine");
    assert_eq!(fate(&driver, theirs), theirs_fate, "theirs");
    replays(&driver, &setup);
    Ok(())
}

/// Dromoka's Command answers a Bolt aimed at a player: that Bolt deals nothing, a second Bolt cast later does.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn c9_fight_a_shield_from_a_target_spell_prevents_only_its_damage(
    #[case] seats: usize,
) -> C9FightResult {
    let game = || -> Result<Scenario, Box<dyn Error>> {
        c9_fight_game(seats, "Dromoka's Command", (2, 2), (3, 3))
    };
    let setup = game()?.setup()?;
    let mut driver = Driver::from_scenario(game()?)?;
    let foe = seats - 1;
    let before = life(&driver, foe);
    let at_foe = driver.target_player(foe);
    driver.cast_at("Bolt", &[at_foe])?;
    let bolt = *driver
        .state()
        .stack()
        .first()
        .ok_or("no Bolt on the stack")?;
    let spell = Target::Object(driver.state().object(bolt).ok_or("no Bolt")?.reference());
    let targets = vec![vec![spell], vec![driver.target_named(MINE)?]];
    c9_fight_cast(&mut driver, "Dromoka's Command", vec![0, 2], &targets)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, foe), before, "the first Bolt is prevented");
    driver.cast_at("Bolt", &[at_foe])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, foe), before - 3, "the second Bolt is not");
    replays(&driver, &setup);
    Ok(())
}

/// Epic Fight with one mode or both: a doubled creature fights with its doubled power and toughness.
#[rstest]
#[case::both_modes(2, vec![0, 1], Fate::Marked(4), Fate::Dead, (4, 6))]
#[case::only_the_fight(3, vec![1], Fate::Dead, Fate::Marked(2), (2, 3))]
#[case::only_the_doubling(4, vec![0], Fate::Marked(0), Fate::Marked(0), (4, 6))]
fn c9_fight_epic_fight_doubles_before_it_fights(
    #[case] seats: usize,
    #[case] modes: Vec<usize>,
    #[case] mine_fate: Fate,
    #[case] theirs_fate: Fate,
    #[case] size: (i32, i32),
) -> C9FightResult {
    let game = || c9_fight_game(seats, "Epic Fight", (2, 3), (4, 4));
    let setup = game()?.setup()?;
    let mut driver = Driver::from_scenario(game()?)?;
    let (mine, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    let (me, them) = (driver.target_named(MINE)?, driver.target_named(THEIRS)?);
    let mut targets = Vec::new();
    if modes.contains(&0) {
        targets.push(vec![me]);
    }
    if modes.contains(&1) {
        targets.extend([vec![me], vec![them]]);
    }
    c9_fight_cast(&mut driver, "Epic Fight", modes, &targets)?;
    settle(&mut driver)?;
    assert_eq!(fate(&driver, mine), mine_fate, "mine");
    assert_eq!(fate(&driver, theirs), theirs_fate, "theirs");
    if mine_fate != Fate::Dead {
        let now = characteristics(driver.state(), mine).ok_or("no creature")?;
        assert_eq!((now.power, now.toughness), (Some(size.0), Some(size.1)));
    }
    replays(&driver, &setup);
    Ok(())
}

/// Savage Stomp costs {G} with a Dinosaur of seat 0 as its first target and {2}{G} with any other creature.
#[rstest]
#[case::a_dinosaur(2, true, 1)]
#[case::not_a_dinosaur(3, false, 3)]
#[case::a_dinosaur_at_four(4, true, 1)]
fn c9_fight_savage_stomp_costs_less_for_a_dinosaur(
    #[case] seats: usize,
    #[case] dinosaur: bool,
    #[case] spent: usize,
) -> C9FightResult {
    let mine = CardDefinition::new(MINE)
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    let mine = Arc::new(if dinosaur {
        mine.with_subtype("Dinosaur")
    } else {
        mine
    });
    let game = || -> Result<Scenario, Box<dyn Error>> {
        let mut scenario = Scenario::new(seats).hand(0, vec![card("Savage Stomp")?]);
        let mut mine_side = lands(3, forest);
        mine_side.extend(lands(1, plains));
        mine_side.push(ready(Arc::clone(&mine)));
        scenario = scenario.battlefield(0, mine_side);
        for seat in 0..seats {
            scenario = scenario.library(seat, (0..8).map(|_| mtg_testkit::cards::bear()));
        }
        Ok(scenario.battlefield(seats - 1, [ready(c9_fight_body(THEIRS, 2, 2))]))
    };
    let setup = game()?.setup()?;
    let mut driver = Driver::from_scenario(game()?)?;
    let (mine_id, theirs) = (driver.id_of(MINE)?, driver.id_of(THEIRS)?);
    driver.begin_cast("Savage Stomp")?;
    driver.choose_targets(&[driver.target_named(MINE)?])?;
    driver.choose_targets(&[driver.target_named(THEIRS)?])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.permanent().is_some_and(|p| p.tapped()))
        .count();
    assert_eq!(tapped, spent, "lands tapped");
    settle(&mut driver)?;
    assert_eq!(
        fate(&driver, mine_id),
        Fate::Marked(2),
        "mine, 4/4 after the counter"
    );
    assert_eq!(fate(&driver, theirs), Fate::Dead, "theirs");
    replays(&driver, &setup);
    Ok(())
}
