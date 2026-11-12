//! Goal: Invasion of Theros // Ephara, Ever-Sheltering plays whole: the Siege enters with four defense and the protector its controller chooses, searches for an Aura, God or Demigod card, can be attacked by every player but the protector, loses defense to damage, is exiled and cast transformed when defeated (or stays exiled), and Ephara draws for the other enchantments that enter and guards herself with three others; every log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameSetup;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::dk1f_cards::{assert_replays, first_main, has, with_lands};

const CARD: &str = "Invasion of Theros // Ephara, Ever-Sheltering";
const INVASION: &str = "Invasion of Theros";
const EPHARA: &str = "Ephara, Ever-Sheltering";
const LIBRARY: [&str; 4] = [
    "Grizzly Bears",
    "All That Glitters",
    "Heliod, God of the Sun",
    "Hercules, Olympian Hero",
];

/// What the table answers while the Siege and its trigger come in.
struct Picks {
    /// The seat chosen as the protector.
    protector: usize,
    /// The card found by the search.
    found: &'static str,
    /// Whether the Siege's controller casts the card once it is defeated.
    cast: bool,
}

const DEFAULT: Picks = Picks {
    protector: 1,
    found: "All That Glitters",
    cast: true,
};

fn name_of(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// Answers the questions of the table until the stack is empty again and priority is asked.
fn resolve(driver: &mut Driver, picks: &Picks) -> Result<(), ScriptError> {
    run_to(driver, picks, None)
}

/// [`resolve`], and on from there to the first priority with an empty stack in `step`.
fn run_to(driver: &mut Driver, picks: &Picks, step: Option<Step>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        let there = step.is_none_or(|step| driver.state().turn().step() == step);
        if there
            && driver.state().stack().is_empty()
            && matches!(decision.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options.options.iter().position(
                    |option| matches!(option, ChoiceOption::Player(seat) if seat.index() == picks.protector),
                );
                Answer::ChooseOption(OptionChoice(at.unwrap_or(0)))
            }
            DecisionKind::ChooseCards(options) => Answer::ChooseCards(CardsChoice(
                options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| name_of(driver, *id) == picks.found)
                    .take(1)
                    .collect(),
            )),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(picks.cast)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    panic!("the stack never emptied");
}

fn defense(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::defense())
        })
}

fn protector(driver: &Driver, id: ObjectId) -> Option<usize> {
    match driver.state().object(id)?.permanent()?.chosen()? {
        Chosen::Player(seat) => Some(seat.index()),
        _ => None,
    }
}

/// Where the cards called `name` are, last first.
fn zones_of(driver: &Driver, name: &str) -> Vec<Zone> {
    driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::zone)
        .collect()
}

/// A table where seat 0 holds the card and three Plains, owns `LIBRARY` and every seat has the `creatures` ready.
fn table(seats: usize, creatures: &[&str]) -> Scenario {
    let mut mine = with_lands(3, []);
    mine.extend(creatures.iter().map(|name| ready(card(name).unwrap())));
    let mut scenario = first_main(seats)
        .hand_named(0, [CARD])
        .library_named(0, LIBRARY)
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            creatures.iter().map(|name| ready(card(name).unwrap())),
        );
    }
    scenario
}

#[rstest]
fn the_siege_enters_with_four_defense_and_the_protector_its_controller_chose(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] chosen: usize,
) -> Result<(), ScriptError> {
    if chosen >= seats {
        return Ok(());
    }
    let setup = table(seats, &[]).setup()?;
    let mut driver = Driver::from_scenario(table(seats, &[]))?;
    driver.cast(INVASION)?;
    resolve(
        &mut driver,
        &Picks {
            protector: chosen,
            ..DEFAULT
        },
    )?;
    let siege = driver.id_of(INVASION)?;
    assert_eq!(defense(&driver, siege), 4);
    assert_eq!(protector(&driver, siege), Some(chosen));
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_search_puts_an_aura_a_god_or_a_demigod_into_the_hand_and_nothing_else(
    #[values(
        "All That Glitters",
        "Heliod, God of the Sun",
        "Hercules, Olympian Hero"
    )]
    found: &'static str,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, &[]).setup()?;
    let mut driver = Driver::from_scenario(table(seats, &[]))?;
    driver.cast(INVASION)?;
    let mut offered: Vec<String> = Vec::new();
    for _ in 0..30 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                offered = options
                    .from
                    .iter()
                    .map(|id| name_of(&driver, *id))
                    .collect();
                break;
            }
            DecisionKind::ChooseOption(_) => {
                driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    offered.sort();
    assert_eq!(
        offered,
        [
            "All That Glitters",
            "Heliod, God of the Sun",
            "Hercules, Olympian Hero"
        ],
        "an Aura, a God and a Demigod, and not the Bears"
    );
    resolve(&mut driver, &Picks { found, ..DEFAULT })?;
    let hand: Vec<String> = driver
        .state()
        .zone(Zone::Hand(PlayerId(0)))
        .iter()
        .map(|id| name_of(&driver, *id))
        .collect();
    assert_eq!(hand, [found], "the found card and nothing more");
    assert_eq!(
        driver.state().zone(Zone::Library(PlayerId(0))).len(),
        LIBRARY.len() - 1
    );
    assert_replays(&driver, &setup)
}

#[rstest]
fn every_player_but_the_protector_may_attack_the_siege(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] attacker: usize,
    #[values(1, 2, 3)] chosen: usize,
) -> Result<(), ScriptError> {
    if chosen >= seats || attacker >= seats {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(table(seats, &["Grizzly Bears"]))?;
    driver.cast(INVASION)?;
    resolve(
        &mut driver,
        &Picks {
            protector: chosen,
            ..DEFAULT
        },
    )?;
    let siege = driver.id_of(INVASION)?;
    let turn = u32::try_from(attacker).unwrap() + 1;
    if attacker == 0 {
        driver.pass_until(Step::DeclareAttackers)?;
    } else {
        driver.idle_until(turn, Step::DeclareAttackers)?;
    }
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("attackers are declared");
    };
    let can_attack = options
        .attackers
        .iter()
        .any(|a| a.defenders.contains(&AttackTarget::Planeswalker(siege)));
    assert_eq!(
        can_attack,
        attacker != chosen,
        "seat {attacker}, protector {chosen}"
    );
    Ok(())
}

/// The Siege in play, seat 0 attacking it with `attacker`, an optional block by the protector and the resolution.
fn attack_the_siege(
    seats: usize,
    attacker: &str,
    blocker: Option<&str>,
    picks: &Picks,
) -> Result<(Driver, GameSetup), ScriptError> {
    let build = || {
        let mut scenario = table(seats, &[attacker]);
        if let Some(blocker) = blocker {
            scenario = scenario.battlefield(picks.protector, [ready(card(blocker).unwrap())]);
        }
        scenario
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast(INVASION)?;
    resolve(&mut driver, picks)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack_planeswalkers(&[(attacker, INVASION)])?;
    if let Some(blocker) = blocker {
        driver.pass_until(Step::DeclareBlockers)?;
        driver.block(&[(blocker, attacker)])?;
    }
    run_to(&mut driver, picks, Some(Step::EndCombat))?;
    Ok((driver, setup))
}

#[rstest]
#[case::two_damage(("Grizzly Bears", false), 2)]
#[case::a_block_stops_it(("Grizzly Bears", true), 4)]
#[case::four_damage_defeat_it(("Serra Angel", false), 0)]
fn damage_takes_defense_counters_off_the_siege_and_a_block_stops_it(
    #[values(2, 3)] seats: usize,
    #[case] attack: (&str, bool),
    #[case] left: u32,
) -> Result<(), ScriptError> {
    let blocker = attack.1.then_some("Grizzly Bears");
    let (driver, setup) = attack_the_siege(seats, attack.0, blocker, &DEFAULT)?;
    if left == 0 {
        // It is Ephara now, cast by seat 0 for nothing.
        assert_eq!(zones_of(&driver, EPHARA), [Zone::Battlefield]);
        assert!(driver.id_of(INVASION).is_err(), "the Siege is gone");
    } else {
        assert_eq!(defense(&driver, driver.id_of(INVASION)?), left);
    }
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_defeated_siege_is_exiled_and_cast_transformed_by_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] chosen: usize,
) -> Result<(), ScriptError> {
    if chosen >= seats {
        return Ok(());
    }
    let picks = Picks {
        protector: chosen,
        ..DEFAULT
    };
    let (driver, setup) = attack_the_siege(seats, "Serra Angel", None, &picks)?;
    let id = driver.id_of(EPHARA)?;
    let ephara = driver.state().object(id).unwrap();
    assert_eq!(ephara.zone(), Zone::Battlefield);
    assert_eq!(ephara.controller().index(), 0);
    assert_eq!(ephara.face(), Some(1), "the back face is up");
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_siege_whose_cast_is_declined_stays_in_exile(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let picks = Picks {
        cast: false,
        ..DEFAULT
    };
    let (driver, setup) = attack_the_siege(seats, "Serra Angel", None, &picks)?;
    assert_eq!(zones_of(&driver, INVASION), [Zone::Exile]);
    assert_eq!(zones_of(&driver, EPHARA).len(), 0, "the card was not cast");
    assert_replays(&driver, &setup)
}

const OTHERS: [&str; 4] = [
    "Anointed Procession",
    "Boon Reflection",
    "Assemble the Entmoot",
    "Armistice",
];

#[rstest]
fn ephara_is_lifelink_and_indestructible_with_three_other_enchantments_and_draws_for_each_that_enters(
    #[values(0, 1, 2, 3)] others: usize,
) -> Result<(), ScriptError> {
    let build = || {
        let in_play = OTHERS
            .iter()
            .skip(1)
            .take(others)
            .map(|name| ready(card(name).unwrap()));
        let mut mine = with_lands(8, ["Serra Angel"]);
        mine.extend(in_play);
        first_main(2)
            .hand_named(0, [CARD, "Anointed Procession"])
            .library_named(0, LIBRARY)
            .battlefield(0, mine)
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    // The Siege falls to the Angel and Ephara comes in; she is no other enchantment to herself.
    driver.cast(INVASION)?;
    resolve(&mut driver, &DEFAULT)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack_planeswalkers(&[("Serra Angel", INVASION)])?;
    run_to(&mut driver, &DEFAULT, Some(Step::EndCombat))?;
    for keyword in [Keyword::Lifelink, Keyword::Indestructible] {
        assert_eq!(
            has(&driver, EPHARA, &keyword),
            others >= 3,
            "{others} others, {keyword:?}"
        );
    }
    // The next enchantment to enter under her controller draws a card and counts toward the three.
    driver.pass_until(Step::Main2)?;
    let hand = driver.state().zone(Zone::Hand(PlayerId(0))).len();
    driver.cast("Anointed Procession")?;
    resolve(&mut driver, &DEFAULT)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(PlayerId(0))).len(),
        hand,
        "the enchantment left the hand and a card was drawn for it"
    );
    for keyword in [Keyword::Lifelink, Keyword::Indestructible] {
        assert_eq!(
            has(&driver, EPHARA, &keyword),
            others + 1 >= 3,
            "{others} + 1, {keyword:?}"
        );
    }
    assert_replays(&driver, &setup)
}

/// A battle that is no Siege: the given defense and nothing printed.
fn plain_battle(defense: u32) -> std::sync::Arc<mtg_cards::definition::CardDefinition> {
    std::sync::Arc::new(
        mtg_cards::definition::CardDefinition::new("Test Battle")
            .with_type(mtg_core::card::CardType::Battle)
            .with_defense(defense),
    )
}

#[rstest]
#[case::bolt_defeats_a_battle_of_three(3, 0)]
#[case::a_battle_with_defense_left_stays(5, 2)]
fn a_battle_with_no_defense_goes_to_its_owners_graveyard_and_one_with_some_stays(
    #[case] printed: u32,
    #[case] left: u32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        first_main(seats)
            .hand_named(0, ["Lightning Bolt"])
            .battlefield(
                0,
                [
                    ready(mtg_testkit::cards::mountain()),
                    ready(plain_battle(printed)),
                ],
            )
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let target = driver.target_named("Test Battle")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    resolve(&mut driver, &DEFAULT)?;
    if left == 0 {
        assert_eq!(
            zones_of(&driver, "Test Battle"),
            [Zone::Graveyard(PlayerId(0))]
        );
    } else {
        assert_eq!(defense(&driver, driver.id_of("Test Battle")?), left);
    }
    assert_replays(&driver, &setup)
}
