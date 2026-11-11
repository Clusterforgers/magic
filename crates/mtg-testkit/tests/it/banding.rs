//! Goal: banding plays out through the engine at tables of two to four seats: a band is declared with the attack and blocked as a group, the attacking player divides the damage of a creature blocking a band, the defending player divides the damage of an attacker a banding creature blocks, "bands with other" bands only creatures of its quality, a band the rules do not allow is refused, and every game replays from its log.

#![allow(clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::combat::{survivors, to_blockers};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::Subtype;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DamageOptions, DecisionKind, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, EngineError, GameSetup, replay};
use mtg_testkit::cards::keyworded;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature: name, power, toughness, subtype and whether it has banding.
type Unit = (&'static str, i32, i32, &'static str, &'static str);

fn card(unit: Unit) -> Arc<CardDefinition> {
    let (name, power, toughness, subtype, keyword) = unit;
    let keywords = match keyword {
        "banding" => vec![Keyword::Banding],
        "bands-with-dinosaurs" => vec![Keyword::BandsWithOther(Selector(vec![Pred::Subtype(
            Subtype("Dinosaur".to_owned()),
        )]))],
        _ => Vec::new(),
    };
    let base = keyworded(name, power, toughness, &keywords);
    Arc::new((*base).clone().with_subtype(subtype))
}

/// `seats` players; player 0 is about to attack with `attackers` and the last player holds `blockers`.
fn table(seats: usize, attackers: &[Unit], blockers: &[Unit]) -> GameSetup {
    Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, attackers.iter().map(|unit| ready(card(*unit))))
        .battlefield(seats - 1, blockers.iter().map(|unit| ready(card(*unit))))
        .setup()
        .expect("the table is valid")
}

fn start(setup: &GameSetup) -> Driver {
    Driver::new(Engine::new(setup.clone()).expect("the game starts")).expect("a driver")
}

/// The damage decisions of a combat, each with the player who was asked.
type Asked = Vec<(PlayerId, DamageOptions)>;

/// Plays combat out to its end: each damage decision is answered with `divide` and recorded, every other with a pass.
fn finish(
    driver: &mut Driver,
    divide: impl Fn(&DamageOptions) -> Vec<(ObjectId, u32)>,
) -> Result<Asked, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..80 {
        if driver.state().turn().step() == Step::EndCombat && driver.engine().decision().is_some() {
            return Ok(asked);
        }
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::AssignDamage(options) => {
                asked.push((decision.player, options.clone()));
                Answer::AssignDamage(divide(options))
            }
            _ => Answer::Priority(PriorityAction::Pass),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the end of combat".into()))
}

fn id(driver: &Driver, name: &str) -> ObjectId {
    driver.id_of(name).expect("on the battlefield")
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .expect("a seat")
        .life()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).expect("the log replays");
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

const KNIGHT: Unit = ("Knight", 2, 2, "Knight", "banding");
const BEAR: Unit = ("Bear", 3, 3, "Bear", "");
const OGRE: Unit = ("Ogre", 1, 1, "Ogre", "");
const WALL: Unit = ("Wall", 2, 4, "Wall", "");

#[rstest]
fn a_band_is_blocked_as_a_group_and_its_attacker_divides_the_blockers_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] to_the_knight: u32,
) {
    let setup = table(seats, &[KNIGHT, BEAR], &[WALL]);
    let mut driver = start(&setup);
    let foe = seats - 1;
    driver
        .attack_in_bands(&[("Knight", foe), ("Bear", foe)], &[&["Knight", "Bear"]])
        .expect("the band attacks");
    to_blockers(&mut driver, 1).expect("blockers");
    driver
        .block(&[("Wall", "Bear")])
        .expect("the Wall blocks the Bear");
    let (knight, bear, wall) = (
        id(&driver, "Knight"),
        id(&driver, "Bear"),
        id(&driver, "Wall"),
    );
    let asked = finish(&mut driver, |options| {
        if options.attacker == wall {
            vec![(knight, to_the_knight), (bear, 2 - to_the_knight)]
        } else {
            options
                .blockers
                .iter()
                .map(|b| (*b, options.power))
                .collect()
        }
    })
    .expect("combat ends");
    // Both members were blocked by the Wall, so neither reached the player; the Wall took 2 + 3 and died.
    assert_eq!(life(&driver, foe), 20);
    let walls: Vec<_> = asked.iter().filter(|(_, o)| o.attacker == wall).collect();
    assert_eq!(walls.len(), 1, "{asked:?}");
    assert_eq!(
        walls[0].0,
        PlayerId::from_index(0),
        "the attacking player divides"
    );
    let mut blocked = walls[0].1.blockers.clone();
    blocked.sort();
    let mut band = vec![knight, bear];
    band.sort();
    assert_eq!(blocked, band);
    let expected: &[&str] = if to_the_knight == 2 {
        &["Bear"]
    } else {
        &["Bear", "Knight"]
    };
    assert_eq!(survivors(&driver), expected);
    replays(&driver, &setup);
}

#[rstest]
#[case::plain_attackers_are_blocked_alone(&[OGRE, BEAR], false, 3)]
#[case::a_band_is_blocked_together(&[KNIGHT, OGRE], true, 0)]
fn only_a_band_is_blocked_as_a_group(
    #[case] attackers: &[Unit],
    #[case] banded: bool,
    #[case] lost: i32,
) {
    let setup = table(2, attackers, &[WALL]);
    let mut driver = start(&setup);
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|unit| (unit.0, 1)).collect();
    let band = [attackers[0].0, attackers[1].0];
    let one = [&band[..]];
    let bands: &[&[&str]] = if banded { &one } else { &[] };
    driver.attack_in_bands(&attacks, bands).expect("the attack");
    to_blockers(&mut driver, 1).expect("blockers");
    driver
        .block(&[("Wall", attackers[0].0)])
        .expect("the block");
    finish(&mut driver, |options| {
        let mut left = options.power;
        options
            .blockers
            .iter()
            .map(|b| {
                let share = left;
                left = 0;
                (*b, share)
            })
            .collect()
    })
    .expect("combat ends");
    assert_eq!(life(&driver, 1), 20 - lost);
    replays(&driver, &setup);
}

#[rstest]
#[case::banding_blocker_lets_the_defender_choose(("Banner", 2, 2, "Wall", "banding"), PlayerId(1))]
#[case::plain_blockers_leave_it_to_the_attacker(("Banner", 2, 2, "Wall", ""), PlayerId(0))]
fn the_defending_player_divides_an_attackers_damage_when_a_banding_creature_blocks(
    #[case] banner: Unit,
    #[case] divider: PlayerId,
) {
    let giant: Unit = ("Giant", 6, 9, "Giant", "");
    let plain: Unit = ("Plain", 2, 2, "Wall", "");
    let setup = table(2, &[giant], &[banner, plain]);
    let mut driver = start(&setup);
    driver.attack(&[("Giant", 1)]).expect("the attack");
    to_blockers(&mut driver, 1).expect("blockers");
    driver
        .block(&[("Banner", "Giant"), ("Plain", "Giant")])
        .expect("a double block");
    let banner_id = id(&driver, "Banner");
    let plain_id = id(&driver, "Plain");
    // All 6 damage on the Banner, which the rules let whoever divides do (CR 510.1c, 702.22j).
    let asked = finish(&mut driver, |_| vec![(banner_id, 6), (plain_id, 0)]).expect("combat ends");
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].0, divider);
    assert_eq!(survivors(&driver), ["Giant", "Plain"]);
    replays(&driver, &setup);
}

#[rstest]
#[case::two_dinosaurs(("Raptor", 2, 2, "Dinosaur", "bands-with-dinosaurs"), ("Brontos", 3, 3, "Dinosaur", ""), true)]
#[case::a_dinosaur_and_a_bear(("Raptor", 2, 2, "Dinosaur", "bands-with-dinosaurs"), ("Bear", 3, 3, "Bear", ""), false)]
#[case::banding_and_one_other(KNIGHT, BEAR, true)]
#[case::two_without_banding(OGRE, BEAR, false)]
fn a_band_the_rules_do_not_allow_is_refused(
    #[case] first: Unit,
    #[case] second: Unit,
    #[case] allowed: bool,
) {
    let mut driver = start(&table(2, &[first, second], &[WALL]));
    let result = driver.attack_in_bands(&[(first.0, 1), (second.0, 1)], &[&[first.0, second.0]]);
    match (allowed, result) {
        (true, Ok(_)) => {}
        (
            false,
            Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::NotOffered("band")))),
        ) => {}
        (_, other) => panic!("allowed {allowed}: {other:?}"),
    }
}
