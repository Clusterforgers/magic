//! Goal: prevention and regeneration shields play through the whole stack as the rules say, at tables of two, three and four seats: a spell or an activated ability makes a shield for its target and for the rest of the turn (Mending Hands, Samite Healer); a shield prevents up to its amount in all and is used up (several burns across one shield, several shields across one burn); when its player is dealt the damage of several attackers in one combat and it cannot stop all of it, that player picks which damage it stops first (lifelink follows); it protects what it names and nothing else; Fog prevents the combat damage of every attacker and blocker and not the damage of a spell; every shield ends with the turn; a regeneration shield (Death Ward, Drudge Skeletons) takes one destruction each, by a destroy effect or by lethal damage, leaving the permanent tapped and without damage, and a regenerating attacker or blocker leaves combat; a destruction that can't be regenerated (Wrath of God) buries the creature whatever shields it has; and damage that can't be prevented passes every shield and leaves it whole.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::combat::to_blockers;
use common::names;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Selector;
use mtg_engine::choose::{ChoiceOption, OptionChoice, ReplacementChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::Duration;
use mtg_engine::events::ZoneCause;
use mtg_engine::object::PermanentState;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{bear, burn, forest, free_creature, keyworded, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::prevent_cards::smite;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes priority until the stack is empty, answering each question of the replacement step with the first effect.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::ChooseReplacement(_) => {
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(0)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Activates the named permanent's first ability that is not a mana ability at these targets and pays for it.
fn activate(driver: &mut Driver, name: &str, targets: &[Target]) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..4 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(targets)?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => break,
        }
    }
    settle(driver)
}

/// The shield spell or ability of seat 0 resolves; then seat 1, the next to hold priority, casts `spell` at `at`.
fn cast_by_foe(driver: &mut Driver, spell: &str, at: Target) -> Result<(), ScriptError> {
    driver.pass()?;
    driver.cast_at(spell, &[at])?;
    settle(driver)
}

fn library(count: usize) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    (0..count).map(|_| forest()).collect()
}

/// A table of `seats` where every seat has cards to draw.
fn table(seats: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, library(10))
    })
}

fn lands(
    make: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
    count: usize,
) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(make())).collect()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(common::pid(seat)).unwrap().life()
}

/// What the first damage shield in force prevents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    /// There is no shield.
    None,
    /// It prevents all the damage it covers.
    All,
    /// It prevents this much more.
    Left(u32),
}

fn shield_left(driver: &Driver) -> Held {
    driver
        .state()
        .rule_effects()
        .iter()
        .find_map(|effect| match &effect.rule {
            Rule::PreventDamage(shield) => Some(shield.left.map_or(Held::All, Held::Left)),
            _ => None,
        })
        .unwrap_or(Held::None)
}

fn regeneration_shields(driver: &Driver) -> usize {
    driver
        .state()
        .rule_effects()
        .iter()
        .filter(|effect| matches!(effect.rule, Rule::Regenerate(_)))
        .count()
}

fn count(driver: &Driver, wanted: impl Fn(&Event) -> bool) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|e| wanted(e))
        .count()
}

fn regenerations(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Regenerated { .. }))
}

fn damage_marked(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, PermanentState::damage)
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(PermanentState::tapped)
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

// Prevention spells

/// Seat 0 holds Mending Hands; seat 1 holds one burn for each number, named `Spark0`, `Spark1` and so on.
fn burn_table(seats: usize, burns: &[i32]) -> Scenario {
    let sparks = burns
        .iter()
        .enumerate()
        .map(|(at, amount)| burn(&format!("Spark{at}"), *amount))
        .collect::<Vec<_>>();
    table(seats)
        .hand_named(0, ["Mending Hands"])
        .battlefield(0, lands(plains, 1))
        .hand(1, sparks)
        .battlefield(1, lands(mountain, burns.len()))
}

#[rstest]
#[case::none(&[], 20, Some(4))]
#[case::less_than_the_shield(&[2], 20, Some(2))]
#[case::exactly_the_shield(&[4], 20, None)]
#[case::more_than_the_shield(&[7], 17, None)]
#[case::two_inside_it(&[2, 2], 20, None)]
#[case::two_across_it(&[3, 3], 18, None)]
#[case::three_across_it(&[1, 1, 3], 19, None)]
#[case::the_remainder_is_remembered(&[1, 1], 20, Some(2))]
#[case::a_burn_after_it_is_used_up(&[4, 3], 17, None)]
fn mending_hands_prevents_four_damage_in_all_and_is_used_up(
    #[values(2, 3, 4)] seats: usize,
    #[case] burns: &[i32],
    #[case] life_left: i32,
    #[case] shield: Option<u32>,
) -> Result<(), ScriptError> {
    let setup = burn_table(seats, burns).setup().unwrap();
    let mut driver = Driver::from_scenario(burn_table(seats, burns))?;
    let me = driver.target_player(0);
    driver.cast_at("Mending Hands", &[me])?;
    settle(&mut driver)?;
    assert_eq!(shield_left(&driver), Held::Left(4));
    for at in 0..burns.len() {
        cast_by_foe(&mut driver, &format!("Spark{at}"), me)?;
    }
    assert_eq!(life(&driver, 0), life_left);
    assert_eq!(shield_left(&driver), shield.map_or(Held::None, Held::Left));
    // A shield that took damage and has some left says how much (`ShieldUsed`); one that took its last ends.
    let (mut left, mut remembered) = (4, 0);
    for amount in burns {
        let taken = (*amount).min(left);
        left -= taken;
        remembered += usize::from(taken > 0 && left > 0);
    }
    assert_eq!(
        count(&driver, |event| matches!(event, Event::ShieldUsed { .. })),
        remembered
    );
    assert_eq!(
        count(&driver, |event| matches!(event, Event::RuleExpired { .. })),
        usize::from(left == 0)
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::the_shielded_creature_survives("Ogre", "Ogre", 3, true)]
#[case::a_burn_beyond_the_shield_kills("Ogre", "Ogre", 9, false)]
#[case::another_creature_is_not_covered("Ogre", "Imp", 3, false)]
#[case::the_player_is_not_covered("Ogre", "player0", 3, true)]
fn a_shield_protects_what_it_names_and_nothing_else(
    #[values(2, 3, 4)] seats: usize,
    #[case] shielded: &str,
    #[case] hit: &str,
    #[case] amount: i32,
    #[case] hit_survives: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        burn_table(seats, &[amount]).battlefield(
            0,
            [
                ready(free_creature("Ogre", 3, 3)),
                ready(free_creature("Imp", 3, 3)),
                ready(plains()),
            ],
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let protected = driver.target_named(shielded)?;
    driver.cast_at("Mending Hands", &[protected])?;
    settle(&mut driver)?;
    let aim = if hit == "player0" {
        driver.target_player(0)
    } else {
        driver.target_named(hit)?
    };
    cast_by_foe(&mut driver, "Spark0", aim)?;
    if hit == "player0" {
        assert_eq!(life(&driver, 0), 20 - amount);
    } else {
        let alive = names(driver.state(), Zone::Battlefield).contains(&hit.to_owned());
        assert_eq!(alive, hit_survives);
    }
    Ok(())
}

#[rstest]
#[case::no_healer(0, 3, 17)]
#[case::one_healer(1, 3, 18)]
#[case::two_healers(2, 3, 19)]
#[case::three_healers(3, 3, 20)]
#[case::more_healers_than_damage(3, 1, 20)]
fn each_samite_healer_prevents_one_damage_and_taps_to_do_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] healers: usize,
    #[case] amount: i32,
    #[case] life_left: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        table(seats)
            .battlefield(
                0,
                (0..healers).map(|_| ready(card("Samite Healer").unwrap())),
            )
            .hand(1, [burn("Spark0", amount)])
            .battlefield(1, lands(mountain, 1))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let me = driver.target_player(0);
    for _ in 0..healers {
        activate(&mut driver, "Samite Healer", &[me])?;
    }
    // Each healer is tapped for its shield, and a tapped one offers no second.
    assert_eq!(
        count(&driver, |event| matches!(
            event,
            Event::PermanentTapped { .. }
        )),
        healers
    );
    assert_eq!(
        driver.offered_abilities("Samite Healer")?,
        Vec::<usize>::new()
    );
    if healers > 0 {
        assert_eq!(shield_left(&driver), Held::Left(1));
    }
    cast_by_foe(&mut driver, "Spark0", me)?;
    assert_eq!(life(&driver, 0), life_left);
    replays(&driver, &setup);
    Ok(())
}

// Fog

/// Seat 0 attacks every other seat with a 3/3 `A<seat>`; seat 1 holds a 2/2 `Guard`, Fog and a burn.
fn fog_table(seats: usize) -> Scenario {
    let attackers: Vec<PresetPermanent> = (1..seats)
        .map(|seat| ready(free_creature(&format!("A{seat}"), 3, 3)))
        .collect();
    table(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, attackers)
        .battlefield(
            1,
            [
                ready(free_creature("Guard", 2, 2)),
                ready(forest()),
                ready(mountain()),
            ],
        )
        .hand(1, vec![card("Fog").unwrap(), burn("Spark0", 2)])
}

#[rstest]
#[case::fog_unblocked(true, false, 20, true, 0)]
#[case::fog_blocked(true, true, 20, true, 0)]
#[case::no_fog_unblocked(false, false, 17, true, 0)]
#[case::no_fog_blocked(false, true, 20, false, 2)]
fn fog_prevents_the_combat_damage_of_every_creature_and_not_the_damage_of_a_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] fogged: bool,
    #[case] blocked: bool,
    #[case] guarded_life: i32,
    #[case] guard_lives: bool,
    #[case] attacker_damage: u32,
) -> Result<(), ScriptError> {
    let setup = fog_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(fog_table(seats))?;
    let attacks: Vec<(String, usize)> = (1..seats).map(|seat| (format!("A{seat}"), seat)).collect();
    let attacks: Vec<(&str, usize)> = attacks
        .iter()
        .map(|(name, seat)| (name.as_str(), *seat))
        .collect();
    driver.attack(&attacks)?;
    // The defending seat answers with Fog and a burn at the attacking player, as the attack is declared.
    driver.pass()?;
    if fogged {
        driver.cast("Fog")?;
        settle(&mut driver)?;
        driver.pass()?;
    }
    driver.cast_at("Spark0", &[driver.target_player(0)])?;
    settle(&mut driver)?;
    to_blockers(&mut driver, seats - 1)?;
    if blocked {
        driver.block(&[("Guard", "A1")])?;
    }
    driver.idle_until(2, Step::EndCombat)?;
    // The spell's damage is not combat damage, so Fog leaves it alone.
    assert_eq!(life(&driver, 0), 18);
    let others = if fogged { 20 } else { 17 };
    for seat in 1..seats {
        let wanted = if seat == 1 { guarded_life } else { others };
        // A blocked attacker deals its damage to the blocker, so the player behind it keeps their life.
        assert_eq!(life(&driver, seat), wanted, "seat {seat}");
    }
    assert_eq!(
        names(driver.state(), Zone::Battlefield).contains(&"Guard".to_owned()),
        guard_lives
    );
    assert_eq!(damage_marked(&driver, "A1"), attacker_damage);
    replays(&driver, &setup);
    Ok(())
}

// The order a shield takes the damage of one combat in

/// Seat 0 attacks seat 1 with a lifelinking `Linker` and a `Grunt`; seat 1 holds Mending Hands.
fn pick_table(seats: usize, linker: i32, grunt: i32) -> Scenario {
    table(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(keyworded("Linker", linker, 3, &[Keyword::Lifelink])),
                ready(free_creature("Grunt", grunt, 2)),
            ],
        )
        .hand_named(1, ["Mending Hands"])
        .battlefield(1, lands(plains, 1))
}

/// Answers what the table asks passively until a seat is asked to pick an option, and returns the question.
fn until_the_pick(driver: &mut Driver) -> Result<Option<(usize, Vec<String>)>, ScriptError> {
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(None);
        };
        if driver.state().turn().step() == Step::EndCombat {
            return Ok(None);
        }
        if let DecisionKind::ChooseOption(options) = decision.kind {
            let labels = options
                .options
                .iter()
                .map(|option| match option {
                    ChoiceOption::Label(label) => label.clone(),
                    other => panic!("a label was expected, got {other:?}"),
                })
                .collect();
            return Ok(Some((decision.player.index(), labels)));
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the combat damage".into()))
}

#[rstest]
#[case::the_lifelinker_first(3, 2, Some("Linker"), 19, 20)]
#[case::the_other_first(3, 2, Some("Grunt"), 19, 21)]
#[case::a_large_one_first(4, 1, Some("Linker"), 19, 20)]
#[case::a_small_one_first(4, 1, Some("Grunt"), 19, 21)]
#[case::enough_for_both(2, 2, None, 20, 20)]
#[case::exactly_enough(3, 1, None, 20, 20)]
fn mending_hands_takes_the_combat_damage_in_the_order_its_player_picks(
    #[values(2, 3, 4)] seats: usize,
    #[case] linker: i32,
    #[case] grunt: i32,
    #[case] pick: Option<&str>,
    #[case] defender_life: i32,
    #[case] attacker_life: i32,
) -> Result<(), ScriptError> {
    let setup = pick_table(seats, linker, grunt).setup().unwrap();
    let mut driver = Driver::from_scenario(pick_table(seats, linker, grunt))?;
    driver.attack(&[("Linker", 1), ("Grunt", 1)])?;
    driver.pass()?;
    let me = driver.target_player(1);
    driver.cast_at("Mending Hands", &[me])?;
    settle(&mut driver)?;
    to_blockers(&mut driver, 1)?;
    let asked = until_the_pick(&mut driver)?;
    match (pick, asked) {
        (None, None) => {}
        (Some(first), Some((who, offered))) => {
            // Seat 1 is hurt, so seat 1 orders the damage; the question names each event once.
            assert_eq!(who, 1);
            let mut sorted = offered.clone();
            sorted.sort();
            assert_eq!(
                sorted,
                [
                    format!("{grunt} damage from Grunt to player 1"),
                    format!("{linker} damage from Linker to player 1"),
                ]
            );
            let index = offered
                .iter()
                .position(|label| label.contains(first))
                .unwrap();
            driver.answer(Answer::ChooseOption(OptionChoice(index)))?;
        }
        (wanted, other) => panic!("expected the pick {wanted:?}, the table asked {other:?}"),
    }
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(life(&driver, 1), defender_life);
    assert_eq!(life(&driver, 0), attacker_life);
    replays(&driver, &setup);
    Ok(())
}

// The end of the turn

/// What a shield made in turn 1 is tested against in turn 2, when the other seat is the active player.
#[derive(Clone, Copy, Debug)]
enum Shield {
    Fog,
    MendingHands,
    DeathWard,
}

#[rstest]
#[case::fog(Shield::Fog)]
#[case::mending_hands(Shield::MendingHands)]
#[case::death_ward(Shield::DeathWard)]
fn a_shield_ends_with_the_turn_it_was_made_in(
    #[values(2, 3, 4)] seats: usize,
    #[case] shield: Shield,
) -> Result<(), ScriptError> {
    let name = match shield {
        Shield::Fog => "Fog",
        Shield::MendingHands => "Mending Hands",
        Shield::DeathWard => "Death Ward",
    };
    let scenario = || {
        table(seats)
            .hand_named(0, [name])
            .battlefield(0, [ready(plains()), ready(forest()), ready(bear())])
            .battlefield(
                1,
                [
                    ready(free_creature("Raider", 3, 3)),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .hand(1, vec![burn("Spark0", 3), smite("Smite").unwrap()])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let target = match shield {
        Shield::DeathWard => driver.target_named("Grizzly Bears")?,
        _ => driver.target_player(0),
    };
    match shield {
        Shield::Fog => driver.cast(name)?,
        _ => driver.cast_at(name, &[target])?,
    };
    settle(&mut driver)?;
    assert!(shield_left(&driver) != Held::None || regeneration_shields(&driver) == 1);
    driver.idle_until(2, Step::Main1)?;
    // The shield expired in the cleanup step of turn 1, with the effects that last until end of turn.
    assert_eq!(shield_left(&driver), Held::None);
    assert_eq!(regeneration_shields(&driver), 0);
    assert!(count(&driver, |event| matches!(event, Event::RuleExpired { .. })) >= 1);
    match shield {
        Shield::Fog => {
            driver.idle_until(2, Step::DeclareAttackers)?;
            driver.attack(&[("Raider", 0)])?;
            driver.idle_until(2, Step::EndCombat)?;
            assert_eq!(life(&driver, 0), 17);
        }
        Shield::MendingHands => {
            driver.cast_at("Spark0", &[target])?;
            settle(&mut driver)?;
            assert_eq!(life(&driver, 0), 17);
        }
        Shield::DeathWard => {
            driver.cast_at("Smite", &[target])?;
            settle(&mut driver)?;
            assert_eq!(regenerations(&driver), 0);
            assert!(
                !names(driver.state(), Zone::Battlefield).contains(&"Grizzly Bears".to_owned())
            );
        }
    }
    Ok(())
}

// Regeneration

/// Where the regeneration shields of seat 0's creature come from.
#[derive(Clone, Copy, Debug)]
enum Source {
    /// Death Ward cast at a Grizzly Bears.
    DeathWard,
    /// The `{B}` ability of Drudge Skeletons, activated once for each shield.
    Drudge,
}

/// What seat 1 destroys it with.
#[derive(Clone, Copy, Debug)]
enum Destroyer {
    /// A spell that destroys target creature.
    Effect,
    /// A burn of lethal damage.
    Damage,
}

#[rstest]
fn each_regeneration_shield_takes_one_destruction_and_the_next_one_kills(
    #[values(2, 3, 4)] seats: usize,
    #[values(Source::DeathWard, Source::Drudge)] source: Source,
    #[values(Destroyer::Effect, Destroyer::Damage)] destroyer: Destroyer,
    #[values(0, 1, 2)] shields: usize,
) -> Result<(), ScriptError> {
    let (subject, hand, battlefield) = match source {
        Source::DeathWard => (
            "Grizzly Bears",
            vec![card("Death Ward").unwrap(); shields],
            [ready(bear()), ready(plains()), ready(plains())],
        ),
        Source::Drudge => (
            "Drudge Skeletons",
            Vec::new(),
            [
                ready(card("Drudge Skeletons").unwrap()),
                ready(swamp()),
                ready(swamp()),
            ],
        ),
    };
    let foes: Vec<_> = (0..=shields)
        .map(|at| match destroyer {
            Destroyer::Effect => smite(&format!("Foe{at}")).unwrap(),
            Destroyer::Damage => burn(&format!("Foe{at}"), 3),
        })
        .collect();
    let scenario = || {
        table(seats)
            .hand(0, hand.clone())
            .battlefield(0, battlefield.clone())
            .hand(1, foes.clone())
            .battlefield(1, lands(mountain, shields + 1))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    for _ in 0..shields {
        match source {
            Source::DeathWard => {
                let at = driver.target_named(subject)?;
                driver.cast_at("Death Ward", &[at])?;
                settle(&mut driver)?;
            }
            Source::Drudge => activate(&mut driver, subject, &[])?,
        }
    }
    assert_eq!(regeneration_shields(&driver), shields);
    for at in 0..=shields {
        let aim = driver.target_named(subject)?;
        cast_by_foe(&mut driver, &format!("Foe{at}"), aim)?;
        if at < shields {
            // A shield is used up and the creature is tapped, without damage, and still there.
            assert!(names(driver.state(), Zone::Battlefield).contains(&subject.to_owned()));
            assert!(is_tapped(&driver, subject), "regeneration taps it");
            assert_eq!(damage_marked(&driver, subject), 0);
            assert_eq!(regenerations(&driver), at + 1);
            assert_eq!(regeneration_shields(&driver), shields - at - 1);
        }
    }
    assert!(!names(driver.state(), Zone::Battlefield).contains(&subject.to_owned()));
    assert!(names(driver.state(), Zone::Graveyard(common::pid(0))).contains(&subject.to_owned()));
    assert_eq!(regenerations(&driver), shields);
    // A destruction is a move to the graveyard with its cause, or lethal damage that kills: once, at the end.
    let deaths = count(&driver, |event| {
        matches!(
            event,
            Event::CreatureDied { .. }
                | Event::ZoneChanged {
                    cause: ZoneCause::Destroyed,
                    from: Zone::Battlefield,
                    ..
                }
        )
    });
    assert_eq!(deaths, 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::attacking(true)]
#[case::blocking(false)]
fn a_regenerating_attacker_or_blocker_is_tapped_and_leaves_combat(
    #[values(2, 3, 4)] seats: usize,
    #[case] attacking: bool,
) -> Result<(), ScriptError> {
    let skeletons = || ready(card("Drudge Skeletons").unwrap());
    let scenario = || {
        let base = table(seats).at(2, 0, Step::DeclareAttackers);
        if attacking {
            base.battlefield(0, [skeletons(), ready(swamp())])
                .battlefield(1, [ready(free_creature("Guard", 3, 3))])
        } else {
            base.battlefield(0, [ready(free_creature("Raider", 3, 3))])
                .battlefield(1, [skeletons(), ready(swamp())])
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let (attacker, blocker) = if attacking {
        ("Drudge Skeletons", "Guard")
    } else {
        ("Raider", "Drudge Skeletons")
    };
    driver.attack(&[(attacker, 1)])?;
    to_blockers(&mut driver, seats - 1)?;
    driver.block(&[(blocker, attacker)])?;
    // The regeneration is paid for after the blocks; the active player holds priority first.
    if !attacking {
        driver.pass()?;
    }
    activate(&mut driver, "Drudge Skeletons", &[])?;
    assert_eq!(regeneration_shields(&driver), 1);
    driver.idle_until(2, Step::EndCombat)?;
    let events = driver.engine().events();
    assert_eq!(regenerations(&driver), 1);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::CreatureDied { .. }))
    );
    let skeletons = driver.id_of("Drudge Skeletons")?;
    assert!(is_tapped(&driver, "Drudge Skeletons"));
    assert_eq!(damage_marked(&driver, "Drudge Skeletons"), 0);
    // Whichever side it was on, it deals its damage first and then leaves combat (CR 701.19c) and stays out of it.
    assert_eq!(
        damage_marked(&driver, if attacking { "Guard" } else { "Raider" }),
        1
    );
    assert!(
        driver
            .state()
            .combat()
            .attackers()
            .iter()
            .all(|a| a.object() != skeletons && !a.blockers().contains(&skeletons))
    );
    replays(&driver, &setup);
    Ok(())
}

// Wrath of God

#[rstest]
fn a_destruction_that_cant_be_regenerated_buries_the_creature_whatever_shields_it_has(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        (1..seats).fold(
            table(seats)
                .hand_named(0, ["Wrath of God", "Death Ward"])
                .battlefield(
                    0,
                    [
                        ready(bear()),
                        ready(card("Drudge Skeletons").unwrap()),
                        ready(swamp()),
                    ]
                    .into_iter()
                    .chain(lands(plains, 5)),
                ),
            |scenario, seat| {
                scenario.battlefield(seat, [ready(free_creature(&format!("Imp{seat}"), 2, 2))])
            },
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    activate(&mut driver, "Drudge Skeletons", &[])?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Death Ward", &[bears])?;
    settle(&mut driver)?;
    assert_eq!(regeneration_shields(&driver), 2);
    // Another Death Ward on the same creature changes nothing about a burial.
    let before = driver.engine().events().len();
    driver.cast("Wrath of God")?;
    settle(&mut driver)?;
    assert!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .all(|name| name == "Plains" || name == "Swamp"),
        "only the lands are left"
    );
    let played = &driver.engine().events()[before..];
    assert!(
        !played
            .iter()
            .any(|event| matches!(event, Event::Regenerated { .. }))
    );
    let buried = played
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    cause: ZoneCause::Buried,
                    from: Zone::Battlefield,
                    ..
                }
            )
        })
        .count();
    assert_eq!(buried, seats + 1, "every creature, shielded or not");
    assert_eq!(
        regeneration_shields(&driver),
        0,
        "the shields went with their creatures"
    );
    replays(&driver, &setup);
    Ok(())
}

// Damage that can't be prevented

#[rstest]
#[case::all_damage(None, 17, Some(4))]
#[case::only_combat_damage(Some(true), 20, Some(1))]
#[case::only_other_damage(Some(false), 17, Some(4))]
fn damage_that_cant_be_prevented_passes_a_shield_and_leaves_it_whole(
    #[values(2, 3, 4)] seats: usize,
    #[case] combat: Option<bool>,
    #[case] life_left: i32,
    #[case] shield: Option<u32>,
) -> Result<(), ScriptError> {
    let scenario = || {
        burn_table(seats, &[3]).battlefield(0, [ready(plains()), ready(free_creature("Hex", 1, 1))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let me = driver.target_player(0);
    driver.cast_at("Mending Hands", &[me])?;
    settle(&mut driver)?;
    let hex = driver.id_of("Hex")?;
    driver.add_rule_from(
        hex,
        Rule::DamageCantBePrevented {
            source: Selector::default(),
            combat,
        },
        Duration::UntilEndOfTurn,
    )?;
    cast_by_foe(&mut driver, "Spark0", me)?;
    assert_eq!(life(&driver, 0), life_left);
    assert_eq!(shield_left(&driver), shield.map_or(Held::None, Held::Left));
    let used = count(&driver, |event| matches!(event, Event::ShieldUsed { .. }));
    assert_eq!(used, usize::from(combat == Some(true)));
    Ok(())
}

#[rstest]
#[case::all_damage(None, 17)]
#[case::only_combat_damage(Some(true), 17)]
#[case::only_other_damage(Some(false), 20)]
fn combat_damage_that_cant_be_prevented_passes_fog(
    #[values(2, 3, 4)] seats: usize,
    #[case] combat: Option<bool>,
    #[case] life_left: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        table(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A1", 3, 3))])
            .battlefield(1, [ready(forest())])
            .hand(1, vec![card("Fog").unwrap()])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let hex = driver.id_of("A1")?;
    driver.add_rule_from(
        hex,
        Rule::DamageCantBePrevented {
            source: Selector::default(),
            combat,
        },
        Duration::UntilEndOfTurn,
    )?;
    driver.attack(&[("A1", 1)])?;
    driver.pass()?;
    driver.cast("Fog")?;
    settle(&mut driver)?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(life(&driver, 1), life_left);
    Ok(())
}
