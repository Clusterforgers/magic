//! Goal: blocks an effect changes after blockers are declared (CR 506.4, 509.1h), at two to four seats: Sorrow's Path
//! has two blockers trade the attackers they block only when each could block the other's (a flier stops it), General
//! Jarkeld has two attackers trade the creatures blocking exactly one of them, Aetherplasm returns itself and puts a
//! creature card from its controller's hand onto the battlefield blocking the attacker (each "may" declined keeps what
//! was), and every log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A permanent: a 4/4 Flier, a 1/4 Spider with reach, a named corpus card or a plain creature of that name, 3/3 when it attacks and 1/4 when it blocks.
fn permanent(name: &str, attacking: bool) -> PresetPermanent {
    match name {
        "Flier" => return ready(keyworded(name, 4, 4, &[Keyword::Flying])),
        "Spider" => return ready(keyworded(name, 1, 4, &[Keyword::Reach])),
        _ => {}
    }
    match card(name) {
        Ok(found) => ready(found),
        Err(_) if attacking => ready(free_creature(name, 3, 3)),
        Err(_) => ready(free_creature(name, 1, 4)),
    }
}

/// Seat 0 controls `ours` and attacks seat 1 with `attackers`; seat 1 controls `theirs`, holds `hand` and declares
/// `blocks`.
fn blocked(
    seats: usize,
    (ours, attackers): (&[&str], &[&str]),
    (theirs, hand): (&[&str], &[&str]),
    blocks: &[(&str, &str)],
) -> Result<Driver, ScriptError> {
    let mine: Vec<PresetPermanent> = ours
        .iter()
        .map(|name| permanent(name, false))
        .chain(attackers.iter().map(|name| permanent(name, true)))
        .collect();
    let defending: Vec<PresetPermanent> =
        theirs.iter().map(|name| permanent(name, false)).collect();
    let held: Vec<_> = hand.iter().map(|name| card(name).unwrap()).collect();
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(1, defending)
        .hand(1, held);
    let mut driver = Driver::from_scenario(scenario)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    driver.block(blocks)?;
    Ok(driver)
}

/// The names of the creatures blocking the attacker named, sorted.
fn blockers(driver: &Driver, name: &str) -> Vec<String> {
    let id = driver.id_of(name).unwrap();
    let attack = driver
        .state()
        .combat()
        .attackers()
        .iter()
        .find(|attack| attack.object() == id)
        .unwrap();
    let mut named: Vec<String> = attack
        .blockers()
        .iter()
        .map(|blocker| driver.state().object(*blocker).unwrap().card().name.clone())
        .collect();
    named.sort();
    named
}

/// Lets the stack resolve, answering yes or no questions in turn with `said` and a card choice with `chosen`.
fn resolve(driver: &mut Driver, said: &[bool], chosen: Option<&str>) -> Result<(), ScriptError> {
    let mut said = said.iter();
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::YesNo(_) => {
                let yes = *said.next().expect("a question was not expected");
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let picked = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| {
                        chosen == Some(driver.state().object(*id).unwrap().card().name.as_str())
                    })
                    .take(1)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                assert!(said.next().is_none(), "a question was not asked");
                return Ok(());
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The log replays to the same state.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Activates the first ability of `source` with `targets` and lets the stack resolve.
fn activate(driver: &mut Driver, source: &str, targets: &[&str]) -> Result<(), ScriptError> {
    driver.begin_ability(source, 0)?;
    let chosen = targets
        .iter()
        .map(|name| driver.target_named(name))
        .collect::<Result<Vec<_>, _>>()?;
    driver.choose_targets(&chosen)?;
    resolve(driver, &[], None)
}

// (attackers, blocks, after: blockers of the first attacker, of the second)
#[rstest]
#[case::trade(&["Ape", "Bat"], &[("Wall", "Ape"), ("Guard", "Bat")], (&["Guard"][..], &["Wall"][..]))]
#[case::a_flier_stops_it(&["Ape", "Flier"], &[("Wall", "Ape"), ("Spider", "Flier")], (&["Wall"][..], &["Spider"][..]))]
#[case::reach_trades_with_a_ground_blocker(&["Ape", "Bat"], &[("Spider", "Ape"), ("Wall", "Bat")], (&["Wall"][..], &["Spider"][..]))]
fn r115_sorrows_path_trades_two_blocks(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[&str],
    #[case] blocks: &[(&str, &str)],
    #[case] after: (&[&str], &[&str]),
) -> Result<(), ScriptError> {
    let theirs: Vec<&str> = blocks.iter().map(|(blocker, _)| *blocker).collect();
    let mut driver = blocked(
        seats,
        (&["Sorrow's Path"], attackers),
        (&theirs, &[]),
        blocks,
    )?;
    activate(&mut driver, "Sorrow's Path", &[blocks[0].0, blocks[1].0])?;
    assert_eq!(blockers(&driver, attackers[0]), after.0);
    assert_eq!(blockers(&driver, attackers[1]), after.1);
    replays(&driver);
    Ok(())
}

// (attackers, blocks, after: blockers of the first attacker, of the second)
#[rstest]
#[case::one_each(&["Ape", "Bat"], &[("Wall", "Ape"), ("Guard", "Bat")], (&["Guard"][..], &["Wall"][..]))]
#[case::two_against_one(&["Ape", "Bat"], &[("Wall", "Ape"), ("Guard", "Bat"), ("Ox", "Bat")], (&["Guard", "Ox"][..], &["Wall"][..]))]
#[case::a_flier_stops_it(&["Ape", "Flier"], &[("Wall", "Ape"), ("Spider", "Flier")], (&["Wall"][..], &["Spider"][..]))]
fn r115_general_jarkeld_trades_two_attackers_blockers(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[&str],
    #[case] blocks: &[(&str, &str)],
    #[case] after: (&[&str], &[&str]),
) -> Result<(), ScriptError> {
    let theirs: Vec<&str> = blocks.iter().map(|(blocker, _)| *blocker).collect();
    let mut driver = blocked(
        seats,
        (&["General Jarkeld"], attackers),
        (&theirs, &[]),
        blocks,
    )?;
    activate(&mut driver, "General Jarkeld", attackers)?;
    assert_eq!(blockers(&driver, attackers[0]), after.0);
    assert_eq!(blockers(&driver, attackers[1]), after.1);
    replays(&driver);
    Ok(())
}

// (answers, card put, blockers of the Ape after, Aetherplasm in hand)
#[rstest]
#[case::put_another(&[true, true], Some("Grizzly Bears"), &["Grizzly Bears"][..], true)]
#[case::put_itself_back(&[true, true], Some("Aetherplasm"), &["Aetherplasm"][..], false)]
#[case::put_nothing(&[true, false], None, &[][..], true)]
#[case::stays(&[false], None, &["Aetherplasm"][..], false)]
fn r115_aetherplasm_puts_a_creature_in_blocking(
    #[values(2, 3, 4)] seats: usize,
    #[case] said: &[bool],
    #[case] put: Option<&str>,
    #[case] after: &[&str],
    #[case] in_hand: bool,
) -> Result<(), ScriptError> {
    let mut driver = blocked(
        seats,
        (&[], &["Ape"]),
        (&["Aetherplasm"], &["Grizzly Bears"]),
        &[("Aetherplasm", "Ape")],
    )?;
    resolve(&mut driver, said, put)?;
    assert_eq!(blockers(&driver, "Ape"), after);
    let attack = driver.state().combat().attackers()[0].clone();
    assert!(attack.blocked());
    let hand = names(driver.state(), Zone::Hand(pid(1)));
    assert_eq!(
        hand.contains(&"Aetherplasm".to_owned()),
        in_hand,
        "{hand:?}"
    );
    replays(&driver);
    Ok(())
}
