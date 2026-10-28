//! Goal: "can't attack you or planeswalkers you control" (the Vow cycle, Sandwurm Convergence, Blazing Archon, Reverence) and attack taxes paid in life play at 2 to 4 seats as the rules say: the restriction takes away the player who controls its source, their planeswalkers or both from the defenders of the creatures it names, while every other player stays open and a creature with no defender left is not offered; a declaration at a protected defender is refused; a creature that must attack is not required to with nothing to attack; a life tax is paid as the attack is declared (the life of each taxed attacker added up, beside a mana tax) and an attack that costs more life than its player has is refused; and every log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::modify::Attacked;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::Duration;
use mtg_engine::rules::{AttackTax, Rule};
use mtg_engine::turn::Step;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, plains, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// A permanent of the corpus card called `name`.
fn corpus(name: &str) -> PresetPermanent {
    ready(card(name).unwrap())
}

/// Seat 0 is about to declare attackers with `mine` (the creature "A" first, then the rest) and `mana` Plains; seat 1
/// holds `held` and the planeswalker "Chandra"; every seat holds `life`.
fn scenario(
    seats: usize,
    mine: Vec<PresetPermanent>,
    held: Vec<PresetPermanent>,
    mana: usize,
    life: i32,
) -> Scenario {
    let mut ours = mine;
    ours.extend(lands(mana, plains));
    let mut theirs = held;
    theirs.push(ready(walker("Chandra", 3)));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, ours)
        .battlefield(1, theirs);
    for seat in 0..seats {
        scenario = scenario.life(seat, life);
    }
    scenario
}

/// The defenders the creature called `name` is offered, or `None` when it is not offered.
fn defenders(driver: &Driver, name: &str) -> Option<Vec<AttackTarget>> {
    let DecisionKind::DeclareAttackers(options) = &driver.decision().ok()?.kind else {
        return None;
    };
    options
        .attackers
        .iter()
        .find(|option| {
            driver
                .state()
                .object(option.object)
                .is_some_and(|object| object.card().name == name)
        })
        .map(|option| option.defenders.clone())
}

fn chandra(driver: &Driver) -> ObjectId {
    driver.id_of("Chandra").unwrap()
}

/// What seat 0 may attack when it may attack everyone but `closed` (seat 1, its planeswalker or both).
fn open(driver: &Driver, seats: usize, player: bool, walker: bool) -> Vec<AttackTarget> {
    let mut open: Vec<AttackTarget> = (1..seats)
        .filter(|seat| *seat != 1 || player)
        .map(|seat| AttackTarget::Player(pid(seat)))
        .collect();
    if walker {
        open.push(AttackTarget::Planeswalker(chandra(driver)));
    }
    open
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Seat 1's Aura on seat 0's creature "A", and seat 0's plain creature "B".
fn vowed(vow: &str) -> (Vec<PresetPermanent>, Vec<PresetPermanent>) {
    let mine = vec![
        ready(free_creature("A", 2, 2)),
        ready(free_creature("B", 1, 1)),
    ];
    (mine, vec![attached_to(corpus(vow), 0, 0)])
}

#[rstest]
#[case::vow_of_lightning("Vow of Lightning")]
#[case::vow_of_duty("Vow of Duty")]
#[case::vow_of_flight("Vow of Flight")]
#[case::vow_of_malice("Vow of Malice")]
#[case::vow_of_torment("Vow of Torment")]
#[case::vow_of_wildness("Vow of Wildness")]
fn a_vowed_creature_attacks_only_the_other_players(
    #[values(2, 3, 4)] seats: usize,
    #[case] vow: &str,
) -> Result<(), ScriptError> {
    let (mine, held) = vowed(vow);
    let scenario = scenario(seats, mine, held, 0, 20);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let others = open(&driver, seats, false, false);
    assert_eq!(
        defenders(&driver, "A"),
        (!others.is_empty()).then_some(others)
    );
    assert_eq!(
        defenders(&driver, "B"),
        Some(open(&driver, seats, true, true))
    );
    assert!(driver.attack(&[("A", 1)]).is_err());
    let walker = chandra(&driver);
    let a = driver.id_of("A")?;
    assert!(
        driver
            .answer(Answer::DeclareAttackers(vec![(
                a,
                AttackTarget::Planeswalker(walker)
            )]))
            .is_err()
    );
    if seats > 2 {
        driver.attack(&[("A", 2), ("B", 1)])?;
    } else {
        driver.attack(&[("B", 1)])?;
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::sandwurm_keeps_flyers_from_both("Sandwurm Convergence", true, true, false)]
#[case::blazing_archon_keeps_all_from_the_player("Blazing Archon", false, true, true)]
#[case::reverence_keeps_the_small_from_the_player("Reverence", false, true, true)]
fn a_static_restriction_closes_what_it_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] source: &str,
    #[case] flying: bool,
    #[case] walker_open: bool,
    #[case] player_closed_to_big: bool,
) -> Result<(), ScriptError> {
    // "A" is a 2/2 (with flying for Sandwurm), "Big" a 3/3 with no keyword.
    let a = if flying {
        ready(mtg_testkit::cards::keyworded(
            "A",
            2,
            2,
            &[mtg_cards::keyword::Keyword::Flying],
        ))
    } else {
        ready(free_creature("A", 2, 2))
    };
    let mine = vec![a, ready(free_creature("Big", 3, 3))];
    let scenario = scenario(seats, mine, vec![corpus(source)], 0, 20);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let walker_kept = !walker_open || flying;
    assert_eq!(
        defenders(&driver, "A"),
        Some(open(&driver, seats, false, !walker_kept)).filter(|open| !open.is_empty())
    );
    let big_reaches_player = !(player_closed_to_big && source == "Blazing Archon");
    assert_eq!(
        defenders(&driver, "Big"),
        Some(open(&driver, seats, big_reaches_player, true))
    );
    let walker = chandra(&driver);
    let big = driver.id_of("Big")?;
    driver.answer(Answer::DeclareAttackers(vec![(
        big,
        AttackTarget::Planeswalker(walker),
    )]))?;
    replays(&driver, &setup);
    Ok(())
}

/// A rule that taxes the attacks on seat 1 and its planeswalkers `life` life and `mana` each.
fn life_tax(life: u32, mana: &str) -> Rule {
    Rule::AttackTax(AttackTax {
        players: vec![pid(1)],
        attacked: Attacked::Both,
        creatures: mtg_cards::refs::Selector::default(),
        mana: mtg_core::mana::ManaCost::parse(mana).unwrap(),
        life,
    })
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

fn paid(events: &[Event]) -> u32 {
    events
        .iter()
        .filter_map(|event| match event {
            Event::LifePaid { amount, .. } => Some(*amount),
            _ => None,
        })
        .sum()
}

#[rstest]
#[case::one_attacker(2, 1, 20, Some(2))]
#[case::two_attackers(2, 2, 20, Some(4))]
#[case::three_life_each(3, 2, 20, Some(6))]
#[case::down_to_nothing(2, 2, 4, Some(4))]
#[case::one_life_short(2, 2, 3, None)]
fn a_life_tax_is_paid_as_the_attack_is_declared(
    #[values(2, 3, 4)] seats: usize,
    #[case] each: u32,
    #[case] count: usize,
    #[case] start: i32,
    #[case] total: Option<u32>,
) -> Result<(), ScriptError> {
    let names = ["A", "B"];
    let mine = names[..count]
        .iter()
        .map(|name| ready(free_creature(name, 1, 1)))
        .collect();
    let scenario = scenario(seats, mine, Vec::new(), 0, start);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.add_rule(life_tax(each, ""), Duration::UntilEndOfTurn)?;
    let attacks: Vec<(&str, usize)> = names[..count].iter().map(|name| (*name, 1)).collect();
    match total {
        Some(total) => {
            let events = driver.attack(&attacks)?;
            assert_eq!(paid(&events), total);
            assert_eq!(life(&driver, 0), start - i32::try_from(total).unwrap());
        }
        None => {
            assert!(driver.attack(&attacks).is_err());
            assert_eq!(life(&driver, 0), start);
            // Another player is not protected and costs nothing.
            if seats > 2 {
                let events = driver.attack(&[("A", 2)])?;
                assert_eq!(paid(&events), 0);
            }
        }
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_life_tax_beside_a_mana_tax_is_paid_with_the_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = vec![ready(free_creature("A", 1, 1))];
    let scenario = scenario(seats, mine, Vec::new(), 2, 20);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.add_rule(life_tax(2, "{2}"), Duration::UntilEndOfTurn)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("not declaring attackers");
    };
    assert_eq!(
        options.attackers[0].life.len(),
        2,
        "the player and Chandra cost life"
    );
    driver.attack(&[("A", 1)])?;
    driver.expect_decision(0, "pay-mana")?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(paid(&events), 2);
    assert_eq!(life(&driver, 0), 18);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_creature_that_must_attack_is_not_required_to_with_nothing_to_attack() -> Result<(), ScriptError>
{
    // Two seats: the vowed creature can attack nobody, so no declaration is asked for though it must attack.
    let (mut mine, held) = vowed("Vow of Lightning");
    mine.truncate(1);
    let scenario = scenario(2, mine, held, 0, 20);
    let mut driver = Driver::from_scenario(scenario)?;
    let a = driver.id_of("A")?;
    let named = driver.state().object(a).unwrap().reference();
    driver.add_rule(Rule::MustAttack(vec![named]), Duration::UntilEndOfTurn)?;
    assert_eq!(defenders(&driver, "A"), None);
    driver.expect_decision(0, "priority")?;
    Ok(())
}
