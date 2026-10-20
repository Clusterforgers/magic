//! Goal: tapping and untapping effects change exactly the permanents they name that are not in that state yet, at tables of two, three and four seats: a selector over the board reaches every seat it names, a permanent already in the state is passed over without an event, the events of one effect come together in one step, a tapped creature cannot be declared as an attacker or offered as a blocker and an untapped one can, and an untapped land pays again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{name_of, pid, spell};
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::NothingToDo;
use mtg_engine::turn::Step;
use mtg_engine::{Event, zones::Zone};
use mtg_testkit::cards::{forest, free_creature, instant};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

/// Which seats a row of a table is about.
#[derive(Debug, Clone, Copy)]
enum Who {
    Nobody,
    Me,
    Others,
    Everyone,
}

impl Who {
    fn covers(self, seat: usize) -> bool {
        match self {
            Who::Nobody => false,
            Who::Me => seat == 0,
            Who::Others => seat != 0,
            Who::Everyone => true,
        }
    }
}

/// `seats` seats; each controls an untapped `Bear n` and a tapped `Sentry n`, and seat 0 holds `spell`.
fn board(seats: usize, spell: std::sync::Arc<mtg_cards::CardDefinition>) -> Scenario {
    (0..seats).fold(Scenario::new(seats).hand(0, [spell]), |scenario, seat| {
        scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Bear {seat}"), 2, 2)),
                tapped(free_creature(&format!("Sentry {seat}"), 1, 3)),
            ],
        )
    })
}

fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

/// The names of the permanents that are tapped now, sorted.
fn tapped_now(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .and_then(|o| o.permanent())
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .map(|id| name_of(state, *id))
        .collect();
    names.sort();
    names
}

/// What the tables expect to be tapped: a `Bear` of each seat `bears` covers, a `Sentry` of each seat `sentries` covers.
fn expected(seats: usize, bears: Who, sentries: Who) -> Vec<String> {
    let mut names: Vec<String> = (0..seats)
        .flat_map(|seat| {
            [
                bears.covers(seat).then(|| format!("Bear {seat}")),
                sentries.covers(seat).then(|| format!("Sentry {seat}")),
            ]
        })
        .flatten()
        .collect();
    names.sort();
    names
}

const OF_MINE: &str = r#"{"ControlledBy":"You"}"#;
const OF_THEIRS: &str = r#"{"ControlledBy":"Opponents"}"#;

#[rstest]
#[case::their_creatures(&format!(r#"[{{"Tap":{{"what":{{"Each":[{{"Type":"Creature"}},{OF_THEIRS}]}}}}}}]"#), Who::Others, Who::Everyone)]
#[case::every_creature(
    r#"[{"Tap":{"what":{"Each":[{"Type":"Creature"}]}}}]"#,
    Who::Everyone,
    Who::Everyone
)]
#[case::my_permanents(&format!(r#"[{{"Tap":{{"what":{{"Each":[{OF_MINE}]}}}}}}]"#), Who::Me, Who::Everyone)]
#[case::the_tapped_ones_already(
    r#"[{"Tap":{"what":{"Each":["Tapped"]}}}]"#,
    Who::Nobody,
    Who::Everyone
)]
#[case::a_name_nobody_has(
    r#"[{"Tap":{"what":{"Each":[{"Named":"Nobody"}]}}}]"#,
    Who::Nobody,
    Who::Everyone
)]
#[case::my_sentry_untapped(&format!(r#"[{{"Untap":{{"what":{{"Each":[{OF_MINE},{{"Named":"Sentry 0"}}]}}}}}}]"#), Who::Nobody, Who::Others)]
#[case::their_creatures_untapped(&format!(r#"[{{"Untap":{{"what":{{"Each":[{{"Type":"Creature"}},{OF_THEIRS}]}}}}}}]"#), Who::Nobody, Who::Me)]
#[case::every_creature_untapped(
    r#"[{"Untap":{"what":{"Each":[{"Type":"Creature"}]}}}]"#,
    Who::Nobody,
    Who::Nobody
)]
#[case::the_untapped_ones_already(
    r#"[{"Untap":{"what":{"Each":["Untapped"]}}}]"#,
    Who::Nobody,
    Who::Everyone
)]
#[case::tapped_and_untapped_again(
    r#"[{"Tap":{"what":{"Each":[{"Type":"Creature"}]}}},{"Untap":{"what":{"Each":[{"Named":"Bear 0"}]}}}]"#,
    Who::Others,
    Who::Everyone
)]
fn an_effect_changes_the_permanents_it_names_that_are_not_in_that_state_yet(
    #[case] effects: &str,
    #[case] bears: Who,
    #[case] sentries: Who,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let card = spell("Wave", CardType::Instant, "[]", effects);
    let mut driver = Driver::from_scenario(board(seats, card))?;
    driver.cast("Wave")?;
    resolve_all(&mut driver)?;
    assert_eq!(tapped_now(&driver), expected(seats, bears, sentries));
    Ok(())
}

/// The events of the first resolution in the log that come before its first step is done.
fn first_step(driver: &Driver) -> Vec<Event> {
    driver
        .engine()
        .events()
        .iter()
        .skip_while(|event| !matches!(event, Event::ResolutionBegan { .. }))
        .take_while(|event| !matches!(event, Event::StepDone { .. }))
        .cloned()
        .collect()
}

#[rstest]
#[case::tapping(r#"[{"Tap":{"what":{"Each":[{"Type":"Creature"}]}}}]"#, true)]
#[case::untapping(r#"[{"Untap":{"what":{"Each":[{"Type":"Creature"}]}}}]"#, false)]
fn one_event_is_logged_for_each_permanent_changed_and_none_for_the_rest(
    #[case] effects: &str,
    #[case] taps: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let card = spell("Wave", CardType::Instant, "[]", effects);
    let mut driver = Driver::from_scenario(board(seats, card))?;
    driver.cast("Wave")?;
    resolve_all(&mut driver)?;
    let all = driver.engine().events();
    let count = |wanted: fn(&Event) -> bool| all.iter().filter(|event| wanted(event)).count();
    let (tapped, untapped) = (
        count(|e| matches!(e, Event::PermanentTapped { .. })),
        count(|e| matches!(e, Event::PermanentUntapped { .. })),
    );
    // Every Bear is untapped and every Sentry tapped: only the ones the effect turns over are logged.
    assert_eq!(
        (tapped, untapped),
        if taps { (seats, 0) } else { (0, seats) }
    );
    // They come together, in one step, before the step is done.
    let step = first_step(&driver);
    let in_step = step
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::PermanentTapped { .. } | Event::PermanentUntapped { .. }
            )
        })
        .count();
    assert_eq!(in_step, seats);
    Ok(())
}

fn frost() -> std::sync::Arc<mtg_cards::CardDefinition> {
    spell(
        "Frost",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Tap":{"what":{"Target":0}}}]"#,
    )
}

fn thaw() -> std::sync::Arc<mtg_cards::CardDefinition> {
    spell(
        "Thaw",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Untap":{"what":{"Target":0}}}]"#,
    )
}

fn until_has_priority(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("priority for seat {seat}")))
}

fn nothing_to_do(driver: &Driver, seat: usize, kind: NothingToDo) -> bool {
    driver
        .engine()
        .events()
        .iter()
        .any(|e| matches!(e, Event::NothingToDo { player, kind: k } if *player == pid(seat) && *k == kind))
}

#[rstest]
fn a_tapped_creature_is_not_declared_as_an_attacker(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] frozen: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::BeginCombat)
            .battlefield(1, [ready(free_creature("Raider", 2, 2))])
            .hand(0, [frost()]),
    )?;
    if frozen {
        until_has_priority(&mut driver, 0)?;
        let raider = driver.target_named("Raider")?;
        driver.cast_at("Frost", &[raider])?;
        resolve_all(&mut driver)?;
    }
    // On to the declaration of attackers.
    for _ in 0..16 {
        let waiting = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !waiting || driver.state().turn().step() != Step::BeginCombat {
            break;
        }
        driver.pass()?;
    }
    let asked = matches!(driver.decision()?.kind, DecisionKind::DeclareAttackers(_));
    assert_eq!(asked, !frozen);
    assert_eq!(nothing_to_do(&driver, 1, NothingToDo::Attack), frozen);
    Ok(())
}

/// What a combat table does before blockers are declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trick {
    /// The attacker's controller taps the Wall.
    Frost,
    /// The defender untaps the Wall.
    Thaw,
    Nothing,
}

#[rstest]
#[case::an_untapped_wall_blocks(false, Trick::Nothing, true)]
#[case::a_tapped_wall_does_not(true, Trick::Nothing, false)]
#[case::a_wall_tapped_after_the_attack_does_not(false, Trick::Frost, false)]
#[case::a_wall_untapped_after_the_attack_does(true, Trick::Thaw, true)]
fn a_tapped_creature_is_not_offered_as_a_blocker_and_an_untapped_one_is(
    #[case] wall_tapped: bool,
    #[case] trick: Trick,
    #[case] blocks: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let wall = free_creature("Wall", 0, 4);
    let wall = if wall_tapped {
        tapped(wall)
    } else {
        ready(wall)
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("Raider", 2, 2))])
            .battlefield(1, [wall])
            .hand(0, [frost()])
            .hand(1, [thaw()]),
    )?;
    driver.attack(&[("Raider", 1)])?;
    match trick {
        Trick::Frost => {
            let target = driver.target_named("Wall")?;
            driver.cast_at("Frost", &[target])?;
            resolve_all(&mut driver)?;
        }
        Trick::Thaw => {
            until_has_priority(&mut driver, 1)?;
            let target = driver.target_named("Wall")?;
            driver.cast_at("Thaw", &[target])?;
            resolve_all(&mut driver)?;
        }
        Trick::Nothing => {}
    }
    for _ in 0..16 {
        let waiting = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !waiting || driver.state().turn().step() != Step::DeclareAttackers {
            break;
        }
        driver.pass()?;
    }
    let offered: Vec<String> = match &driver.decision()?.kind {
        DecisionKind::DeclareBlockers(options) => options
            .blockers
            .iter()
            .map(|blocker| name_of(driver.state(), blocker.object))
            .collect(),
        _ => Vec::new(),
    };
    assert_eq!(
        offered,
        if blocks {
            vec!["Wall".to_owned()]
        } else {
            Vec::new()
        }
    );
    assert_eq!(nothing_to_do(&driver, 1, NothingToDo::Block), !blocks);
    Ok(())
}

#[rstest]
fn an_untapped_land_pays_again(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let thaw_land = spell(
        "Thaw Land",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Land"}]}}]"#,
        r#"[{"Untap":{"what":{"Target":0}}}]"#,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [tapped(forest())])
            .hand(0, [thaw_land, instant("Zap", 0, &[Color::Green])]),
    )?;
    let castable = |driver: &Driver| -> Result<Vec<String>, ScriptError> {
        let DecisionKind::Priority(options) = &driver.decision()?.kind else {
            panic!("seat 0 has priority");
        };
        let mut names: Vec<String> = options
            .castable
            .iter()
            .map(|id| name_of(driver.state(), *id))
            .collect();
        names.sort();
        Ok(names)
    };
    assert_eq!(castable(&driver)?, ["Thaw Land"], "the Forest is tapped");
    let forest = driver.target_named("Forest")?;
    driver.cast_at("Thaw Land", &[forest])?;
    resolve_all(&mut driver)?;
    assert_eq!(tapped_now(&driver), Vec::<String>::new());
    assert_eq!(castable(&driver)?, ["Zap"], "the Forest pays again");
    Ok(())
}

#[test]
fn the_keywords_of_a_tapped_creature_are_unchanged() -> Result<(), ScriptError> {
    // A tap neither adds nor removes anything but the tapped state: vigilance and haste stay.
    let guard = mtg_testkit::cards::keyworded("Guard", 2, 2, &[Keyword::Vigilance, Keyword::Haste]);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(guard)])
            .hand(0, [frost()]),
    )?;
    let guard = driver.target_named("Guard")?;
    driver.cast_at("Frost", &[guard])?;
    resolve_all(&mut driver)?;
    assert_eq!(tapped_now(&driver), ["Guard"]);
    let id = driver.id_of("Guard")?;
    assert_eq!(
        mtg_engine::keywords::keywords_of(driver.state(), id),
        [&Keyword::Vigilance, &Keyword::Haste]
    );
    Ok(())
}
