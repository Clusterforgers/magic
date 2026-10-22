//! Goal: a permanent that enters as a copy has the copiable values of the object it copies from the moment it enters, at tables of two, three and four seats: Clone asks its controller which creature to copy (none is a choice, and with no creature nobody is asked), takes the name, types, power, toughness and abilities of that creature and none of its counters or damage, can copy a copy, keeps the exceptions of its copy effect (Phantasmal Image is an Illusion that is sacrificed when it becomes a target), applies the enters replacement of the copied card (three counters), and is a second legend when it copies one; a token that is a copy (Cackling Counterpart) has the same values, none of the counters, and a copy of a token (populate) is a token of the same kind, chosen among several, none when there is no token; the original is never changed; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::ObjectId;
use mtg_engine::ability_reader::abilities_of;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, characteristics, replay};
use mtg_testkit::cards::{burn, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, damaged, ready, with_counters};
use rstest::rstest;

const HERO: &str = "Hero of Bladehold";

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn lands(count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(island())).collect()
}

/// The permanents of the battlefield named `name`.
fn named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect()
}

fn counters_on(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

fn stats(driver: &Driver, id: ObjectId) -> (Option<i32>, Option<i32>) {
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power, chars.toughness)
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Casts the Clone variant `spell` with the lands `taps` and passes until its controller is asked what to copy; `None`
/// when it resolves without a question.
fn cast_until_asked(
    driver: &mut Driver,
    spell: &str,
    taps: &[&str],
) -> Result<Option<Vec<ObjectId>>, ScriptError> {
    driver.cast_tapping(spell, taps)?;
    for _ in 0..50 {
        match driver.engine().decision().map(|d| d.kind.clone()) {
            Some(DecisionKind::ChooseCards(options)) => {
                assert_eq!(driver.decision()?.player, pid(0));
                assert_eq!((options.min, options.max), (0, 1));
                assert!(driver.engine().replacing());
                return Ok(Some(options.from));
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(None);
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the copy".into()))
}

fn answer_cards(driver: &mut Driver, picks: &[ObjectId]) -> Result<(), ScriptError> {
    driver.answer(Answer::ChooseCards(CardsChoice(picks.to_vec())))?;
    Ok(())
}

/// Seat 0 holds Clone with four Islands; seat 1 controls `original`.
fn clone_game(seats: usize, original: mtg_engine::PresetPermanent) -> Scenario {
    Scenario::new(seats)
        .hand(0, [card("Clone").unwrap()])
        .battlefield(0, lands(4))
        .battlefield(1, [original])
}

const CLONE_TAPS: [&str; 4] = ["Island", "Island", "Island", "Island"];

#[rstest]
#[case::plain(0, 0)]
#[case::with_counters_and_damage(2, 1)]
fn clone_copies_the_values_of_the_creature_and_none_of_its_counters_or_damage(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] damage: u32,
) -> Result<(), ScriptError> {
    let original = || {
        let hero = damaged(card(HERO).unwrap(), damage);
        if counters > 0 {
            with_counters(hero, CounterKind::PlusOnePlusOne, counters)
        } else {
            hero
        }
    };
    let setup = clone_game(seats, original()).setup().unwrap();
    let mut driver = Driver::from_scenario(clone_game(seats, original()))?;
    let hero = named(&driver, HERO)[0];
    let before = (stats(&driver, hero), counters_on(&driver, hero));
    let options = cast_until_asked(&mut driver, "Clone", &CLONE_TAPS)?.expect("a question");
    assert_eq!(options, [hero]);
    answer_cards(&mut driver, &[hero])?;
    settle(&mut driver)?;
    let heroes = named(&driver, HERO);
    assert_eq!(heroes.len(), 2);
    let copy = *heroes.iter().find(|id| **id != hero).unwrap();
    let object = driver.state().object(copy).unwrap();
    assert_eq!(object.permanent().unwrap().copy().unwrap().of().id, hero);
    assert_eq!(object.kind(), ObjectKind::Card);
    assert_eq!(stats(&driver, copy), (Some(3), Some(4)));
    assert!(characteristics(driver.state(), copy).unwrap().is_creature());
    assert_eq!(
        (counters_on(&driver, copy), object.permanent().unwrap().damage()),
        (0, 0)
    );
    // It has the abilities of the Hero, battle cry and the attack trigger.
    assert_eq!(
        abilities_of(driver.state(), copy).len(),
        abilities_of(driver.state(), hero).len()
    );
    assert!(!abilities_of(driver.state(), copy).is_empty());
    assert_eq!(object.owner(), pid(0));
    // The original is not touched.
    assert_eq!((stats(&driver, hero), counters_on(&driver, hero)), before);
    assert!(
        driver
            .state()
            .object(hero)
            .unwrap()
            .permanent()
            .unwrap()
            .copy()
            .is_none()
    );
    assert!(names(driver.state(), Zone::Hand(pid(0))).is_empty());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::declined(true)]
#[case::no_creature(false)]
fn clone_that_copies_nothing_enters_as_itself_and_dies_as_a_zero_zero(
    #[values(2, 3, 4)] seats: usize,
    #[case] creature_to_copy: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let scenario = Scenario::new(seats)
            .hand(0, [card("Clone").unwrap()])
            .battlefield(0, lands(4));
        if creature_to_copy {
            scenario.battlefield(1, [ready(card(HERO).unwrap())])
        } else {
            scenario
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let asked = cast_until_asked(&mut driver, "Clone", &CLONE_TAPS)?;
    // Nobody is asked when there is nothing to copy; the battlefield is public.
    assert_eq!(asked.is_some(), creature_to_copy);
    if asked.is_some() {
        answer_cards(&mut driver, &[])?;
    }
    settle(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Clone"]);
    assert_eq!(named(&driver, "Clone").len(), 0);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn an_answer_that_names_a_permanent_not_offered_is_refused_and_changes_nothing()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(clone_game(2, ready(card(HERO).unwrap())))?;
    cast_until_asked(&mut driver, "Clone", &CLONE_TAPS)?.expect("a question");
    let before = (driver.engine().events().len(), driver.state().state_hash());
    let island = driver.id_of("Island")?;
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![island])))
            .is_err()
    );
    let hero = named(&driver, HERO)[0];
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![hero, hero])))
            .is_err()
    );
    assert_eq!(
        before,
        (driver.engine().events().len(), driver.state().state_hash())
    );
    Ok(())
}

#[test]
fn the_enters_replacement_of_the_copied_card_applies_to_the_copy() -> Result<(), ScriptError> {
    let scenario = || {
        let mut permanents = lands(4);
        permanents.push(ready(mtg_testkit::cards::forest()));
        permanents.push(ready(plains()));
        Scenario::new(2)
            .hand(
                0,
                [
                    card("Faithful Watchdog").unwrap(),
                    card("Clone").unwrap(),
                ],
            )
            .battlefield(0, permanents)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast_tapping("Faithful Watchdog", &["Forest", "Plains"])?;
    settle(&mut driver)?;
    let dog = named(&driver, "Faithful Watchdog")[0];
    assert_eq!(counters_on(&driver, dog), 3);
    cast_until_asked(&mut driver, "Clone", &CLONE_TAPS)?.expect("a question");
    answer_cards(&mut driver, &[dog])?;
    settle(&mut driver)?;
    let copy = *named(&driver, "Faithful Watchdog")
        .iter()
        .find(|id| **id != dog)
        .unwrap();
    // The Watchdog prints three counters as it enters, and so does the copy, which got them from the copied values.
    assert_eq!(counters_on(&driver, copy), 3);
    assert_eq!(counters_on(&driver, dog), 3);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_copy_of_a_copy_has_the_values_of_the_first_original() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(
                0,
                [card("Clone").unwrap(), card("Mirror Image").unwrap()],
            )
            .battlefield(0, lands(8))
            .battlefield(1, [ready(card(HERO).unwrap())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let hero = named(&driver, HERO)[0];
    cast_until_asked(&mut driver, "Clone", &CLONE_TAPS)?.expect("a question");
    answer_cards(&mut driver, &[hero])?;
    settle(&mut driver)?;
    let first = *named(&driver, HERO).iter().find(|id| **id != hero).unwrap();
    // Mirror Image copies only what its controller controls: the first copy is offered, the Hero of the other seat is not.
    let taps = ["Island", "Island", "Island"];
    let options = cast_until_asked(&mut driver, "Mirror Image", &taps)?.expect("a question");
    assert_eq!(options, [first]);
    answer_cards(&mut driver, &[first])?;
    settle(&mut driver)?;
    let heroes = named(&driver, HERO);
    assert_eq!(heroes.len(), 3);
    let second = *heroes
        .iter()
        .find(|id| ![hero, first].contains(id))
        .unwrap();
    let copy = driver
        .state()
        .object(second)
        .unwrap()
        .permanent()
        .unwrap()
        .copy()
        .unwrap();
    assert_eq!(copy.of().id, first);
    assert_eq!(stats(&driver, second), (Some(3), Some(4)));
    assert_eq!(copy.definition().name, HERO);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn phantasmal_image_keeps_its_exceptions_and_is_sacrificed_when_it_becomes_a_target()
-> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Phantasmal Image").unwrap(), burn("Spark", 2)])
            .battlefield(
                0,
                [
                    ready(island()),
                    ready(island()),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .battlefield(1, [ready(card(HERO).unwrap())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let hero = named(&driver, HERO)[0];
    cast_until_asked(&mut driver, "Phantasmal Image", &["Island", "Island"])?
        .expect("a question");
    answer_cards(&mut driver, &[hero])?;
    settle(&mut driver)?;
    let image = *named(&driver, HERO).iter().find(|id| **id != hero).unwrap();
    let object = driver.state().object(image).unwrap();
    // An Illusion in addition to its other types, with the Hero's abilities and the one it was given.
    assert!(object.card().subtypes.contains(&Subtype("Illusion".into())));
    assert!(object.card().subtypes.contains(&Subtype("Knight".into())));
    assert!(
        characteristics(driver.state(), image)
            .unwrap()
            .types
            .contains(CardType::Creature)
    );
    assert_eq!(
        abilities_of(driver.state(), image).len(),
        abilities_of(driver.state(), hero).len() + 1
    );
    // The original has no Illusion subtype.
    assert!(
        !driver
            .state()
            .object(hero)
            .unwrap()
            .card()
            .subtypes
            .contains(&Subtype("Illusion".into()))
    );
    // Aiming anything at the Image sacrifices it.
    let aim = Target::Object(driver.state().object(image).unwrap().reference());
    driver.cast_at("Spark", &[aim])?;
    settle(&mut driver)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert!(graveyard.contains(&"Phantasmal Image".to_owned()), "{graveyard:?}");
    assert_eq!(named(&driver, HERO), [hero]);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_copy_of_a_legendary_creature_is_a_second_legend_and_the_legend_rule_keeps_one()
-> Result<(), ScriptError> {
    // The legend rule looks at the permanents of one controller, so the Beast is the Clone's controller's too.
    let scenario = || {
        let mut permanents = lands(4);
        permanents.push(ready(card("Questing Beast").unwrap()));
        Scenario::new(2)
            .hand(0, [card("Clone").unwrap()])
            .battlefield(0, permanents)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let beast = named(&driver, "Questing Beast")[0];
    cast_until_asked(&mut driver, "Clone", &CLONE_TAPS)?.expect("a question");
    answer_cards(&mut driver, &[beast])?;
    // Two legends with the name: the legend rule asks which to keep, and the other goes to its graveyard.
    let mut kept = None;
    for _ in 0..50 {
        if let Some(DecisionKind::ChooseCards(options)) =
            driver.engine().decision().map(|d| d.kind.clone())
        {
            assert_eq!(options.from.len(), 2);
            assert_eq!((options.min, options.max), (1, 1));
            assert!(!driver.engine().replacing());
            kept = Some(options.from[0]);
            break;
        }
        driver.answer_passively()?;
    }
    let kept = kept.expect("the legend rule asked");
    answer_cards(&mut driver, &[kept])?;
    settle(&mut driver)?;
    assert_eq!(named(&driver, "Questing Beast"), [kept]);
    replays(&driver, &setup);
    Ok(())
}

/// Seat 0 controls the Hero with a +1/+1 counter and six Islands and Plains each, holding the spells in `hand`.
fn counterpart_game(seats: usize, hand: &[&str]) -> Scenario {
    let mut lands: Vec<_> = (0..6).map(|_| ready(island())).collect();
    lands.extend((0..6).map(|_| ready(plains())));
    lands.push(with_counters(
        ready(card(HERO).unwrap()),
        CounterKind::PlusOnePlusOne,
        1,
    ));
    Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name).unwrap()))
        .battlefield(0, lands)
}

fn cast_counterpart(driver: &mut Driver) -> Result<(), ScriptError> {
    let target = driver.target_named(HERO)?;
    driver.begin_cast("Cackling Counterpart")?;
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&["Island", "Island", "Island"], 0)?;
    settle(driver)
}

#[rstest]
fn cackling_counterpart_makes_a_token_with_the_values_of_the_creature_and_none_of_its_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = counterpart_game(seats, &["Cackling Counterpart"])
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(counterpart_game(seats, &["Cackling Counterpart"]))?;
    let hero = named(&driver, HERO)[0];
    cast_counterpart(&mut driver)?;
    let heroes = named(&driver, HERO);
    assert_eq!(heroes.len(), 2);
    let token = *heroes.iter().find(|id| **id != hero).unwrap();
    let object = driver.state().object(token).unwrap();
    assert_eq!(object.kind(), ObjectKind::Token);
    assert_eq!(object.permanent().unwrap().copy().unwrap().of().id, hero);
    assert_eq!((object.owner(), object.controller()), (pid(0), pid(0)));
    assert!(object.permanent().unwrap().summoning_sick());
    assert_eq!(counters_on(&driver, token), 0);
    assert_eq!(stats(&driver, token), (Some(3), Some(4)));
    // The Hero keeps its counter and its 4/5.
    assert_eq!(
        (counters_on(&driver, hero), stats(&driver, hero)),
        (1, (Some(4), Some(5)))
    );
    assert_eq!(
        abilities_of(driver.state(), token).len(),
        abilities_of(driver.state(), hero).len()
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::no_token(0)]
#[case::one_token(1)]
#[case::several_tokens(2)]
fn populate_copies_a_creature_token_chosen_among_those_its_controller_controls(
    #[values(2, 3, 4)] seats: usize,
    #[case] tokens: usize,
) -> Result<(), ScriptError> {
    let hand: Vec<&str> = std::iter::repeat_n("Cackling Counterpart", tokens)
        .chain(["Rootborn Defenses"])
        .collect();
    let setup = counterpart_game(seats, &hand).setup().unwrap();
    let mut driver = Driver::from_scenario(counterpart_game(seats, &hand))?;
    let hero = named(&driver, HERO)[0];
    for _ in 0..tokens {
        cast_counterpart(&mut driver)?;
    }
    let mut made: Vec<ObjectId> = named(&driver, HERO)
        .into_iter()
        .filter(|id| *id != hero)
        .collect();
    made.sort();
    assert_eq!(made.len(), tokens);
    driver.cast_tapping("Rootborn Defenses", &["Plains", "Plains", "Plains"])?;
    if tokens > 1 {
        // Several tokens: the controller chooses one.
        for _ in 0..50 {
            if matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::ChooseCards(_))
            ) {
                break;
            }
            driver.answer_passively()?;
        }
        let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
            panic!("a choice")
        };
        let mut offered = options.from.clone();
        offered.sort();
        assert_eq!(offered, made);
        assert_eq!(driver.decision()?.player, pid(0));
        answer_cards(&mut driver, &[made[1]])?;
    }
    settle(&mut driver)?;
    let now: Vec<ObjectId> = named(&driver, HERO)
        .into_iter()
        .filter(|id| *id != hero && !made.contains(id))
        .collect();
    // One new token when there was a token to copy, none otherwise; it copies the one chosen (the last of several).
    assert_eq!(now.len(), usize::from(tokens > 0));
    if let Some(copy) = now.first() {
        let chosen = *made.last().unwrap();
        let of = driver
            .state()
            .object(*copy)
            .unwrap()
            .permanent()
            .unwrap()
            .copy()
            .unwrap()
            .of()
            .id;
        assert_eq!(of, chosen);
        assert_eq!(driver.state().object(*copy).unwrap().kind(), ObjectKind::Token);
    }
    // The rest of the spell still happens: the creatures gain indestructible.
    let indestructible = |id: ObjectId| {
        keywords_of(driver.state(), id)
            .iter()
            .any(|keyword| keyword.name() == "Indestructible")
    };
    assert!(indestructible(hero));
    replays(&driver, &setup);
    Ok(())
}
