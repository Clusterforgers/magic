//! Goal: a permanent that enters the battlefield changes as its replacement effects say, at tables of two, three and four seats, whichever way it comes in (a land played, a permanent spell that resolves, a card an effect puts onto the battlefield): a shock land asks its controller whether to pay 2 life and enters untapped when they do and tapped when they decline or cannot pay, with the life paid and not lost and nothing applied before the answer; a land that enters tapped does, and one that enters tapped unless its controller controls two basic lands enters untapped with two basic lands and tapped with fewer or with nonbasic lands; a permanent that chooses a color or a creature type as it enters asks its controller (the five colors, the creature types of the creature cards in the game), remembers the choice on the permanent and its ability reads it, so only the creatures its controller controls of that color or type get the bonus, those that come in later too; a creature that enters with counters has them put by an event of its own that a counter doubler or adder changes in the order its controller chooses; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{name_of, names, pid};
use mtg_cards::value::CounterKind;
use mtg_core::card::Subtype;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{ChoiceOption, OptionChoice, ReplacementChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::{forest, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::replace_cards::raise_dead;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// How a table answers the questions asked while something enters.
#[derive(Default)]
struct Policy<'a> {
    /// The effect to apply first, by the name of its card, when two or more apply.
    order: &'a [&'a str],
    /// The color chosen.
    color: Option<Color>,
    /// The creature types chosen, one for each question.
    types: Vec<&'a str>,
}

impl Policy<'_> {
    fn answer(&mut self, driver: &Driver, decision: &Decision) -> Option<Answer> {
        match &decision.kind {
            DecisionKind::ChooseReplacement(options) => {
                let at = self
                    .order
                    .iter()
                    .find_map(|want| {
                        options
                            .candidates
                            .iter()
                            .position(|c| name_of(driver.state(), c.source) == *want)
                    })
                    .unwrap_or(0);
                Some(Answer::ChooseReplacement(ReplacementChoice(at)))
            }
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| match option {
                        ChoiceOption::Color(color) => Some(*color) == self.color,
                        ChoiceOption::Label(label) => {
                            self.types.first().is_some_and(|want| label == want)
                        }
                        ChoiceOption::Player(_) => false,
                    })
                    .expect("the choice the table makes is on offer");
                if matches!(options.options[at], ChoiceOption::Label(_)) {
                    self.types.remove(0);
                }
                Some(Answer::ChooseOption(OptionChoice(at)))
            }
            _ => None,
        }
    }
}

/// Plays on until the stack is empty at a priority decision, answering the questions of the step by `policy` and
/// passing for everything else.
fn play_out(driver: &mut Driver, policy: &mut Policy<'_>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        if let Some(answer) = policy.answer(driver, &decision) {
            driver.answer(answer)?;
            continue;
        }
        match decision.kind {
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

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn chosen_by(driver: &Driver, name: &str) -> Option<Chosen> {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned()
}

fn counters_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

fn permanents(names: &[&str]) -> Vec<mtg_engine::PresetPermanent> {
    names
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect()
}

/// The first card in a graveyard as a target.
fn in_graveyard(driver: &Driver, seat: usize) -> Target {
    let id = driver.state().zone(Zone::Graveyard(pid(seat)))[0];
    Target::Object(driver.state().object(id).unwrap().reference())
}

// A shock land

#[rstest]
#[case::pays(Some(true), 18, false)]
#[case::declines(Some(false), 20, true)]
#[case::cannot_pay(None, 1, true)]
fn a_shock_land_asks_whether_to_pay_two_life_and_enters_untapped_only_when_they_do(
    #[values(2, 3, 4)] seats: usize,
    #[case] pays: Option<bool>,
    #[case] life_after: i32,
    #[case] tapped_after: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let scenario = Scenario::new(seats).hand(0, [card("Steam Vents").unwrap()]);
        if pays.is_none() {
            scenario.life(0, 1)
        } else {
            scenario
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    driver.play_land("Steam Vents")?;
    if let Some(pays) = pays {
        // The play waits for the answer: nothing is logged, applied or paid yet.
        let decision = driver.decision()?.clone();
        assert_eq!(decision.player, pid(0));
        let DecisionKind::YesNo(question) = &decision.kind else {
            panic!("a question was expected, got {:?}", decision.kind);
        };
        assert_eq!(question.text, "Pay 2 life?");
        assert!(driver.engine().replacing());
        assert_eq!(driver.engine().events().len(), begun);
        assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Steam Vents"]);
        assert_eq!(life(&driver, 0), 20);
        driver.answer(Answer::YesNo(YesNoChoice(pays)))?;
    } else {
        // Too little life to pay: nothing to ask.
        assert!(!driver.engine().replacing());
    }
    assert_eq!(names(driver.state(), Zone::Battlefield), ["Steam Vents"]);
    assert_eq!(is_tapped(&driver, "Steam Vents"), tapped_after);
    assert_eq!(life(&driver, 0), life_after);
    // The life is paid before the land comes in, and it is paid, not lost.
    let log = &driver.engine().events()[begun..];
    let paid = log
        .iter()
        .position(|event| matches!(event, Event::LifePaid { amount: 2, .. }));
    let played = log
        .iter()
        .position(|event| matches!(event, Event::LandPlayed { .. }));
    assert_eq!(paid.is_some(), pays == Some(true));
    if let (Some(paid), Some(played)) = (paid, played) {
        assert!(paid < played);
    }
    assert!(
        !log.iter()
            .any(|event| matches!(event, Event::LifeLost { .. }))
    );
    replays(&driver, &setup);
    Ok(())
}

// A land that enters tapped

#[rstest]
#[case::always_tapped("Idyllic Beachfront", &[], true)]
#[case::no_basic_land("Sodden Verdure", &[], true)]
#[case::one_basic_land("Sodden Verdure", &["Forest"], true)]
#[case::two_basic_lands("Sodden Verdure", &["Forest", "Island"], false)]
#[case::three_basic_lands("Sodden Verdure", &["Forest", "Island", "Forest"], false)]
#[case::nonbasic_lands_do_not_count("Sodden Verdure", &["Idyllic Beachfront", "Idyllic Beachfront"], true)]
#[case::one_of_each("Sodden Verdure", &["Forest", "Idyllic Beachfront"], true)]
fn a_land_that_enters_tapped_does_and_one_that_does_unless_it_controls_two_basic_lands_counts_them(
    #[values(2, 3, 4)] seats: usize,
    #[case] land: &str,
    #[case] others: &[&str],
    #[case] tapped_after: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let others = others.iter().map(|name| match *name {
            "Forest" => ready(forest()),
            "Island" => ready(island()),
            other => ready(card(other).unwrap()),
        });
        Scenario::new(seats)
            .hand(0, [card(land).unwrap()])
            .battlefield(0, others)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    driver.play_land(land)?;
    // Nothing to ask, whatever the land does.
    assert!(!driver.engine().replacing());
    assert_eq!(is_tapped(&driver, land), tapped_after);
    let entered_tapped = driver.engine().events()[begun..]
        .iter()
        .filter(|event| matches!(event, Event::Entered { tapped: true, .. }))
        .count();
    assert_eq!(entered_tapped, usize::from(tapped_after));
    replays(&driver, &setup);
    Ok(())
}

// A choice made as the permanent enters

const COLORS: [Color; 5] = Color::ALL;

#[rstest]
fn a_permanent_that_chooses_a_color_as_it_enters_remembers_it_and_its_ability_reads_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(Color::White, Color::Blue, Color::Black, Color::Red, Color::Green)] chosen: Color,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(
                0,
                [
                    card("Hall of Triumph").unwrap(),
                    card("Llanowar Elves").unwrap(),
                ],
            )
            .battlefield(
                0,
                [
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                ]
                .into_iter()
                .chain(permanents(&[
                    "Llanowar Elves",
                    "Serra Angel",
                    "Grizzly Bears",
                ])),
            )
            .battlefield(1, permanents(&["Grizzly Bears"]))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Hall of Triumph")?;
    // The question comes while the Hall enters: the controller picks one of the five colors.
    let mut asked = false;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseOption(options) = &decision.kind {
            assert_eq!(decision.player, pid(0));
            assert_eq!(options.text, "Choose a color.");
            assert_eq!(options.options, COLORS.map(ChoiceOption::Color).to_vec());
            assert!(
                driver.state().stack().len() == 1,
                "the Hall has not entered yet"
            );
            asked = true;
            break;
        }
        driver.pass()?;
    }
    assert!(asked, "the controller of the Hall chooses a color");
    let mut policy = Policy {
        color: Some(chosen),
        ..Policy::default()
    };
    play_out(&mut driver, &mut policy)?;
    assert_eq!(
        chosen_by(&driver, "Hall of Triumph"),
        Some(Chosen::Color(chosen))
    );
    // A creature that comes in later is read the same way.
    driver.cast("Llanowar Elves")?;
    play_out(&mut driver, &mut policy)?;
    let state = driver.state();
    let mut seen = 0;
    for &id in state.zone(Zone::Battlefield) {
        let object = state.object(id).unwrap();
        let (power, toughness) = match object.card().name.as_str() {
            "Llanowar Elves" => (1, 1),
            "Serra Angel" => (4, 4),
            "Grizzly Bears" => (2, 2),
            _ => continue,
        };
        let bonus =
            i32::from(object.controller() == pid(0) && object.card().colors().contains(chosen));
        let chars = characteristics(state, id).unwrap();
        assert_eq!(
            (chars.power, chars.toughness),
            (Some(power + bonus), Some(toughness + bonus)),
            "{} of seat {} with {chosen:?} chosen",
            object.card().name,
            object.controller().index()
        );
        seen += 1;
    }
    assert_eq!(seen, 5);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::elf(&["Elf"], &["Llanowar Elves"])]
#[case::druid(&["Druid"], &["Llanowar Elves"])]
#[case::angel(&["Angel"], &["Serra Angel"])]
#[case::bear(&["Bear"], &["Grizzly Bears"])]
#[case::two_rallies_each_read_their_own(&["Elf", "Angel"], &["Llanowar Elves", "Serra Angel"])]
#[case::two_rallies_one_type_twice(&["Bear", "Bear"], &["Grizzly Bears"])]
fn a_permanent_that_chooses_a_creature_type_as_it_enters_remembers_it_and_its_ability_reads_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] types: &[&str],
    #[case] pumped: &[&str],
) -> Result<(), ScriptError> {
    let rallies = types.len();
    let scenario = || {
        let ranks = std::iter::repeat_n(card("Rally the Ranks").unwrap(), rallies);
        Scenario::new(seats)
            .hand(0, ranks)
            .battlefield(
                0,
                std::iter::repeat_with(|| ready(plains()))
                    .take(2 * rallies)
                    .chain(permanents(&[
                        "Llanowar Elves",
                        "Serra Angel",
                        "Grizzly Bears",
                    ])),
            )
            .battlefield(1, permanents(&["Llanowar Elves", "Grizzly Bears"]))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let mut policy = Policy {
        types: types.to_vec(),
        ..Policy::default()
    };
    for _ in 0..rallies {
        driver.cast("Rally the Ranks")?;
        // What can be chosen: the creature types of the creature cards in the game, sorted and each once.
        let mut asked = false;
        for _ in 0..20 {
            let decision = driver.decision()?.clone();
            if let DecisionKind::ChooseOption(options) = &decision.kind {
                assert_eq!(decision.player, pid(0));
                assert_eq!(options.text, "Choose a creature type.");
                let labels: Vec<&str> = options
                    .options
                    .iter()
                    .map(|option| match option {
                        ChoiceOption::Label(label) => label.as_str(),
                        other => panic!("a creature type was expected, got {other:?}"),
                    })
                    .collect();
                for want in ["Angel", "Bear", "Druid", "Elf"] {
                    assert!(labels.contains(&want), "{want} in {labels:?}");
                }
                assert!(
                    labels.windows(2).all(|pair| pair[0] < pair[1]),
                    "{labels:?}"
                );
                assert!(
                    !driver.state().stack().is_empty(),
                    "the Rally has not entered yet"
                );
                asked = true;
                break;
            }
            driver.pass()?;
        }
        assert!(asked, "the controller of the Rally chooses a creature type");
        play_out(&mut driver, &mut policy)?;
    }
    let state = driver.state();
    let ranks: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == "Rally the Ranks")
        .collect();
    assert_eq!(ranks.len(), rallies);
    let remembered: Vec<Chosen> = ranks
        .iter()
        .map(|id| {
            state
                .object(*id)
                .unwrap()
                .permanent()
                .unwrap()
                .chosen()
                .cloned()
                .unwrap()
        })
        .collect();
    assert_eq!(
        remembered,
        types
            .iter()
            .map(|name| Chosen::CreatureType(Subtype((*name).to_owned())))
            .collect::<Vec<_>>()
    );
    // Each creature of seat 0 gets +1/+1 for every Rally whose type it has; those of other seats get nothing.
    let mut bonused = std::collections::BTreeSet::new();
    for &id in state.zone(Zone::Battlefield) {
        let object = state.object(id).unwrap();
        let (power, toughness, kinds): (i32, i32, &[&str]) = match object.card().name.as_str() {
            "Llanowar Elves" => (1, 1, &["Elf", "Druid"]),
            "Serra Angel" => (4, 4, &["Angel"]),
            "Grizzly Bears" => (2, 2, &["Bear"]),
            _ => continue,
        };
        let bonus = if object.controller() == pid(0) {
            i32::try_from(types.iter().filter(|name| kinds.contains(name)).count()).unwrap()
        } else {
            0
        };
        let chars = characteristics(state, id).unwrap();
        assert_eq!(
            (chars.power, chars.toughness),
            (Some(power + bonus), Some(toughness + bonus)),
            "{} of seat {}",
            object.card().name,
            object.controller().index()
        );
        if bonus > 0 {
            bonused.insert(object.card().name.clone());
        }
    }
    assert_eq!(bonused.into_iter().collect::<Vec<_>>(), {
        let mut want = pumped.to_vec();
        want.sort_unstable();
        want
    });
    replays(&driver, &setup);
    Ok(())
}

// A creature that enters with counters

#[rstest]
#[case::cast(false, &[], &[], 3)]
#[case::cast_adder(false, &["Hardened Scales"], &[], 4)]
#[case::cast_doubler(false, &["Doubling Season"], &[], 6)]
#[case::cast_adder_first(false, &["Hardened Scales", "Doubling Season"], &["Hardened Scales"], 8)]
#[case::cast_doubler_first(false, &["Hardened Scales", "Doubling Season"], &["Doubling Season"], 7)]
#[case::put_onto_the_battlefield(true, &[], &[], 3)]
#[case::put_adder(true, &["Hardened Scales"], &[], 4)]
#[case::put_doubler(true, &["Doubling Season"], &[], 6)]
#[case::put_adder_first(true, &["Hardened Scales", "Doubling Season"], &["Hardened Scales"], 8)]
#[case::put_doubler_first(true, &["Hardened Scales", "Doubling Season"], &["Doubling Season"], 7)]
fn a_creature_that_enters_with_counters_has_them_put_by_an_event_other_effects_change(
    #[values(2, 3, 4)] seats: usize,
    #[case] raised: bool,
    #[case] effects: &[&str],
    #[case] order: &[&str],
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let table = Scenario::new(seats);
        if raised {
            table
                .hand(0, [raise_dead("Raise Dead", false).unwrap()])
                .graveyard(0, [card("Faithful Watchdog").unwrap()])
                .battlefield(0, permanents(effects).into_iter().chain([ready(swamp())]))
        } else {
            table
                .hand(0, [card("Faithful Watchdog").unwrap()])
                .battlefield(
                    0,
                    permanents(effects)
                        .into_iter()
                        .chain([ready(forest()), ready(plains())]),
                )
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    if raised {
        let target = in_graveyard(&driver, 0);
        driver.cast_at("Raise Dead", &[target])?;
    } else {
        driver.cast("Faithful Watchdog")?;
    }
    play_out(
        &mut driver,
        &mut Policy {
            order,
            ..Policy::default()
        },
    )?;
    assert_eq!(counters_on(&driver, "Faithful Watchdog"), counters);
    // The counters are the permanent's own event, after the one that brought it in, in the same batch.
    let log = &driver.engine().events()[begun..];
    let id = driver.id_of("Faithful Watchdog")?;
    let came = log
        .iter()
        .position(|event| {
            matches!(event, Event::SpellResolved { object } | Event::ZoneChanged { object, to: Zone::Battlefield, .. } if *object == id)
        })
        .expect("the creature came in");
    let put = log
        .iter()
        .position(|event| matches!(event, Event::CountersChanged(changed) if matches!(changed.on, Target::Object(named) if named.id == id)))
        .expect("the counters were put");
    assert!(came < put);
    let chars = characteristics(driver.state(), id).unwrap();
    let size = i32::try_from(counters).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(size), Some(size)));
    replays(&driver, &setup);
    Ok(())
}

// A card put onto the battlefield by an effect

#[rstest]
#[case::pays(true, 18, false)]
#[case::declines(false, 20, true)]
fn a_shock_land_put_onto_the_battlefield_by_an_effect_asks_the_same_question(
    #[values(2, 3, 4)] seats: usize,
    #[case] pays: bool,
    #[case] life_after: i32,
    #[case] tapped_after: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [raise_dead("Raise Dead", false).unwrap()])
            .graveyard(0, [card("Steam Vents").unwrap()])
            .battlefield(0, [ready(swamp())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = in_graveyard(&driver, 0);
    driver.cast_at("Raise Dead", &[target])?;
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            break;
        }
        driver.pass()?;
    }
    assert!(driver.engine().replacing());
    driver.answer(Answer::YesNo(YesNoChoice(pays)))?;
    play_out(&mut driver, &mut Policy::default())?;
    assert_eq!(is_tapped(&driver, "Steam Vents"), tapped_after);
    assert_eq!(life(&driver, 0), life_after);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::untapped_effect_tapped_land(false)]
#[case::tapped_effect_tapped_land(true)]
fn a_land_that_enters_tapped_by_an_effect_does_so_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] effect_taps: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [raise_dead("Raise Dead", effect_taps).unwrap()])
            .graveyard(0, [card("Idyllic Beachfront").unwrap()])
            .battlefield(0, [ready(swamp())])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    let target = in_graveyard(&driver, 0);
    driver.cast_at("Raise Dead", &[target])?;
    play_out(&mut driver, &mut Policy::default())?;
    assert!(is_tapped(&driver, "Idyllic Beachfront"));
    // The tapped entry is one event, whether the effect or the land asked for it.
    let entered = driver.engine().events()[begun..]
        .iter()
        .filter(|event| matches!(event, Event::Entered { tapped: true, .. }))
        .count();
    assert_eq!(entered, 1);
    Ok(())
}
