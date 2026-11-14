//! Goal: the name-sticker words play at tables of two to four seats: a count over name stickers with N or more (or N or fewer) letters sizes an Aura's bonus, a chosen letter counts the stickers that begin with it, the u's and o's of name stickers set how many creatures are tapped and how much damage is dealt; a sticker goes only on a creature its player owns that entered this turn; a power and toughness sticker put is offered only those stickers; and a creature told to block a stickered creature must block it; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Sticker};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, un34_sheet, un34_stickerer};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The corpus card, free to cast.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(cost(0, &[]));
    Arc::new(definition)
}

/// A sheet with a name sticker of the words, two power and toughness stickers, a flying sticker and an art sticker, all
/// free.
fn sheet(words: &str) -> Vec<Sticker> {
    vec![
        Sticker::Name {
            words: words.to_owned(),
            tickets: 0,
        },
        Sticker::PowerToughness {
            power: 4,
            toughness: 1,
            tickets: 0,
        },
        Sticker::PowerToughness {
            power: 2,
            toughness: 5,
            tickets: 0,
        },
        Sticker::Ability {
            ability: Box::new(Ability::Keyword(Keyword::Flying)),
            tickets: 0,
        },
        Sticker::Art {},
    ]
}

/// Seat 0 brings the sheet and has the cards in hand; every seat has a Bear.
fn table(seats: usize, words: &str, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [un34_sheet("Fun Sheet", sheet(words))])
        .hand(0, hand);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

/// What the script saw asked.
#[derive(Default)]
struct Seen {
    /// The options offered, each time.
    offered: Vec<Vec<String>>,
    /// The objects offered by each choice of cards.
    cards: Vec<Vec<ObjectId>>,
    /// The legal targets of each choice of targets.
    targets: Vec<Vec<Target>>,
}

/// Answers until the stack is empty: yes to every question, the option labelled `pick` (else the first containing it,
/// else the first), the first card offered, the first legal target of `prefer` (else the first legal one).
fn settle(driver: &mut Driver, pick: &str, prefer: &[Target]) -> Result<Seen, ScriptError> {
    settle_onto(driver, pick, prefer, None)
}

/// [`settle`], choosing `onto` when a choice of cards offers it.
fn settle_onto(
    driver: &mut Driver,
    pick: &str,
    prefer: &[Target],
    onto: Option<ObjectId>,
) -> Result<Seen, ScriptError> {
    let mut seen = Seen::default();
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(seen);
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseOption(options) => {
                let labels: Vec<String> = options
                    .options
                    .iter()
                    .map(|option| match option {
                        ChoiceOption::Label(label) => label.clone(),
                        other => format!("{other:?}"),
                    })
                    .collect();
                let at = labels
                    .iter()
                    .position(|label| label == pick)
                    .or_else(|| labels.iter().position(|label| label.contains(pick)))
                    .unwrap_or(0);
                seen.offered.push(labels);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseCards(options) => {
                seen.cards.push(options.from.clone());
                let taken = onto
                    .filter(|id| options.from.contains(id))
                    .or_else(|| options.from.first().copied())
                    .into_iter()
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(taken)))?;
            }
            DecisionKind::ChooseTargets(options) if !options.legal.is_empty() => {
                seen.targets.push(options.legal.clone());
                let first = prefer
                    .iter()
                    .find(|target| options.legal.contains(target))
                    .copied()
                    .unwrap_or(options.legal[0]);
                driver.choose_targets(&[first])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

/// The Bears seat `seat` owns on the battlefield, oldest first.
fn bears_of(driver: &Driver, seat: usize) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = driver.state().object(*id).unwrap();
            object.owner() == pid(seat) && object.card().name == "Grizzly Bears"
        })
        .collect()
}

fn as_target(driver: &Driver, id: ObjectId) -> Target {
    Target::Object(driver.state().object(id).unwrap().reference())
}

fn stats(driver: &Driver, id: ObjectId) -> (Option<i32>, Option<i32>) {
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    (chars.power, chars.toughness)
}

fn assert_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::nine_letters("Wonderful", true)]
#[case::eight_letters_with_a_space("Big Ducks", true)]
#[case::three_letters("Fun", false)]
fn a_long_name_sticker_makes_the_fight_survivable(
    #[values(2, 3, 4)] seats: usize,
    #[case] words: &str,
    #[case] long: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, words, vec![free("Fight the _____ Fight")]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let mine = bears_of(&driver, 0)[0];
    let theirs = bears_of(&driver, seats - 1)[0];
    let (mine_at, theirs_at) = (as_target(&driver, mine), as_target(&driver, theirs));
    driver.cast_at("Fight the _____ Fight", &[mine_at])?;
    let seen = settle(&mut driver, words, &[theirs_at])?;
    assert!(
        seen.targets.iter().any(|legal| !legal.contains(&mine_at)),
        "the fight is with a creature you don't control"
    );
    assert!(bears_of(&driver, seats - 1).is_empty(), "their Bear took 2");
    assert_eq!(
        bears_of(&driver, 0).len(),
        usize::from(long),
        "+0/+2 only for a sticker of eight or more letters"
    );
    if long {
        assert_eq!(stats(&driver, mine), (Some(2), Some(4)));
    }
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::four_os("Ouroboros", 4)]
#[case::either_case("Oboe", 2)]
#[case::none("Fun", 0)]
fn the_os_of_the_name_sticker_are_the_damage(
    #[values(2, 3, 4)] seats: usize,
    #[case] words: &str,
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let scenario = table(seats, words, vec![free("_____ Balls of Fire")]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let victim = driver.target_player(seats - 1);
    let life = driver.state().player(pid(seats - 1)).unwrap().life();
    driver.cast("_____ Balls of Fire")?;
    settle(&mut driver, words, &[victim])?;
    assert_eq!(
        driver.state().player(pid(seats - 1)).unwrap().life(),
        life - damage
    );
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::one_u("Ruckus", 1)]
#[case::no_u("Fun", 1)]
#[case::none("Grr", 0)]
fn the_us_of_the_name_sticker_are_the_creatures_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] words: &str,
    #[case] most: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, words, vec![free("Make a _____ Splash")]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let theirs = bears_of(&driver, seats - 1)[0];
    let theirs_at = as_target(&driver, theirs);
    driver.cast("Make a _____ Splash")?;
    settle(&mut driver, words, &[theirs_at])?;
    let tapped = driver
        .state()
        .object(theirs)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped);
    assert_eq!(tapped, most > 0, "up to X, X the u's on the stickers");
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::two_zs("Zap", "Zoom", "z", 2)]
#[case::one_of_two("Zap", "Quiet", "q", 1)]
#[case::other("Zap", "Zoom", "b", 0)]
fn a_chosen_letter_counts_the_name_stickers_that_begin_with_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] first: &str,
    #[case] second: &str,
    #[case] letter: &str,
    #[case] bonus: i32,
) -> Result<(), ScriptError> {
    let names = vec![
        Sticker::Name {
            words: first.to_owned(),
            tickets: 0,
        },
        Sticker::Name {
            words: second.to_owned(),
            tickets: 0,
        },
    ];
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [un34_sheet("Names", names)])
        .hand(0, [un34_stickerer("Stick"), un34_stickerer("Stick")])
        .battlefield(
            0,
            [
                ready(card("_____ _____ Rocketship").unwrap()),
                ready(bear()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let ship = driver.id_of("_____ _____ Rocketship")?;
    for words in [first, second] {
        driver.cast("Stick")?;
        settle_onto(&mut driver, words, &[], Some(ship))?;
    }
    assert_eq!(driver.state().stickers_on(ship).len(), 2);
    driver.begin_activation("_____ _____ Rocketship")?;
    settle(&mut driver, "", &[])?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("_____ _____ Rocketship", 1)])?;
    let seen = settle(&mut driver, letter, &[])?;
    assert!(
        seen.offered.iter().any(|labels| labels.len() == 26),
        "the letters are offered: {:?}",
        seen.offered
    );
    assert_eq!(stats(&driver, ship), (Some(2 + bonus), Some(3 + bonus)));
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn a_sticker_goes_only_on_a_creature_you_own_that_entered_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Fun", vec![free("Grizzly Bears")]).battlefield(
        0,
        [
            ready(bear()),
            ready(card("Park Bleater").unwrap()),
            ready(mtg_testkit::cards::plains()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let old = bears_of(&driver, 0)[0];
    let tickets = |driver: &Driver| {
        driver
            .state()
            .player(pid(0))
            .unwrap()
            .counters()
            .find(|(kind, _)| **kind == mtg_cards::value::CounterKind::Other("ticket".to_owned()))
            .map_or(0, |(_, count)| count)
    };
    assert_eq!(tickets(&driver), 0);
    driver.cast("Grizzly Bears")?;
    settle(&mut driver, "Fun", &[])?;
    assert_eq!(tickets(&driver), 1, "another creature you own entered");
    let new = *bears_of(&driver, 0).iter().find(|id| **id != old).unwrap();
    driver.begin_activation("Park Bleater")?;
    let seen = settle(&mut driver, "Fun", &[])?;
    // The one creature that may take it is taken unasked.
    assert!(
        seen.cards.iter().all(|offered| offered == &vec![new]),
        "{:?}",
        seen.cards
    );
    assert_eq!(driver.state().stickers_on(new).len(), 1);
    assert!(
        driver.state().stickers_on(old).is_empty(),
        "the Bear that was there before is no choice"
    );
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn a_power_and_toughness_sticker_is_the_only_kind_offered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Fun", vec![free("Lineprancers")]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let mine = bears_of(&driver, 0)[0];
    driver.cast("Lineprancers")?;
    let seen = settle(&mut driver, "4/1", &[])?;
    assert_eq!(seen.offered.len(), 1);
    assert_eq!(seen.offered[0].len(), 2, "{:?}", seen.offered);
    assert!(seen.offered[0].iter().all(|label| label.contains('/')));
    let on: usize = [mine, driver.id_of("Lineprancers")?]
        .iter()
        .map(|id| driver.state().stickers_on(*id).len())
        .sum();
    assert_eq!(on, 1);
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn the_target_must_block_the_stickered_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut lineprancers = (*card("Lineprancers").unwrap()).clone();
    lineprancers.rules.remove(0);
    let mut stickered = (*bear()).clone();
    stickered.name = "Stickered Bear".to_owned();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [un34_sheet("Fun Sheet", sheet("Fun"))])
        .battlefield(
            0,
            [
                ready(Arc::new(lineprancers)),
                ready(Arc::new(stickered)),
                ready(bear()),
                ready(mtg_testkit::cards::forest()),
                ready(mtg_testkit::cards::forest()),
                ready(mtg_testkit::cards::forest()),
                ready(mtg_testkit::cards::forest()),
            ],
        )
        .hand(0, [un34_stickerer("Stick")])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let stickered = driver.id_of("Stickered Bear")?;
    driver.cast("Stick")?;
    settle_onto(&mut driver, "4/1", &[], Some(stickered))?;
    assert_eq!(driver.state().stickers_on(stickered).len(), 1);
    let blocker = bears_of(&driver, 1)[0];
    let blocker_at = as_target(&driver, blocker);
    let stickered_at = as_target(&driver, stickered);
    driver.begin_activation("Lineprancers")?;
    let seen = settle(&mut driver, "", &[blocker_at, stickered_at])?;
    assert_eq!(seen.targets.len(), 2);
    assert_eq!(
        seen.targets[1],
        vec![stickered_at],
        "only a creature you control with a power and toughness sticker, other than ~"
    );
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Stickered Bear", 1)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    assert!(driver.block(&[]).is_err(), "it must block if able");
    driver.block(&[("Grizzly Bears", "Stickered Bear")])?;
    Ok(())
}

#[rstest]
#[case::accepted(true)]
#[case::declined(false)]
fn the_base_stats_become_the_totals_of_the_stickers(
    #[values(2, 3, 4)] seats: usize,
    #[case] accept: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [un34_sheet("Fun Sheet", sheet("Fun"))])
        .hand(
            0,
            [
                un34_stickerer("Stick"),
                un34_stickerer("Stick"),
                un34_stickerer("Stick"),
            ],
        )
        .battlefield(
            0,
            [
                ready(card("Ambassador Blorpityblorpboop").unwrap()),
                ready(bear()),
            ],
        )
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let ambassador = driver.id_of("Ambassador Blorpityblorpboop")?;
    let mine = bears_of(&driver, 0)[0];
    for (pick, onto) in [("4/1", mine), ("2/5", ambassador), ("Fun", mine)] {
        driver.cast("Stick")?;
        settle_onto(&mut driver, pick, &[], Some(onto))?;
    }
    assert_eq!(
        stats(&driver, ambassador),
        (Some(2), Some(5)),
        "its own sticker"
    );
    driver.pass_until(Step::BeginCombat)?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(accept)))?;
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    settle(&mut driver, "", &[])?;
    let expected = if accept {
        (Some(6), Some(6))
    } else {
        (Some(2), Some(5))
    };
    assert_eq!(
        stats(&driver, ambassador),
        expected,
        "only power and toughness stickers count"
    );
    assert_replays(&driver, scenario);
    Ok(())
}
