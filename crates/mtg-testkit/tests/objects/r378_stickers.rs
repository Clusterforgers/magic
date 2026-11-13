//! Goal: the sticker kinds play at tables of two to four seats: a put of one kind offers only stickers of that kind, up to N stickers go on one at a time, a card keeps its stickers in every zone and its name and abilities come back when it returns to the battlefield, the unique vowels of a name sticker set how many cards are looked at, and the triggers on putting a sticker (any, a name, an art sticker) fire with the kind they wait for; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Sticker, StickerKind};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ZoneCause;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
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

/// A sheet with a name sticker of the words, an art sticker, a flying sticker and a second art sticker, all free.
fn sheet(words: &str) -> Vec<Sticker> {
    vec![
        Sticker::Name {
            words: words.to_owned(),
            tickets: 0,
        },
        Sticker::Art {},
        Sticker::Ability {
            ability: Box::new(Ability::Keyword(Keyword::Flying)),
            tickets: 0,
        },
        Sticker::Art {},
    ]
}

/// Seat 0 brings the sheet and has the cards in hand and a Bear; every other seat has a Bear.
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

/// What the script answers and what it saw asked.
#[derive(Default)]
struct Seen {
    /// The sticker options offered, each time.
    offered: Vec<Vec<String>>,
    /// The size of each choice of cards.
    cards: Vec<usize>,
}

/// Answers until the stack is empty: yes to every question, the first sticker whose words contain `pick`, `onto` when it
/// is offered as the object to put a sticker on (else the first), the first target.
fn settle(driver: &mut Driver, pick: &str, onto: Option<ObjectId>) -> Result<Seen, ScriptError> {
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
                    .position(|label| label.contains(pick))
                    .unwrap_or(0);
                seen.offered.push(labels);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseCards(options) => {
                seen.cards.push(options.from.len());
                let first = onto
                    .filter(|id| options.from.contains(id))
                    .or_else(|| options.from.first().copied());
                let taken = if options.min == 0 && first.is_none() {
                    Vec::new()
                } else {
                    first.into_iter().collect()
                };
                driver.answer(Answer::ChooseCards(CardsChoice(taken)))?;
            }
            DecisionKind::ChooseTargets(options) if !options.legal.is_empty() => {
                let first = options.legal[0];
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

fn bear_of(driver: &Driver, seat: usize) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            let object = driver.state().object(*id).unwrap();
            object.owner() == pid(seat) && object.card().name == "Grizzly Bears"
        })
        .unwrap()
}

fn kinds_on(driver: &Driver, id: ObjectId) -> Vec<StickerKind> {
    let state = driver.state();
    state
        .stickers_on(id)
        .iter()
        .map(|at| state.sticker(*at).unwrap().kind())
        .collect()
}

fn assert_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn moved(driver: &Driver, object: ObjectId, from: Zone, to: Zone) -> mtg_engine::GameState {
    let mut state = driver.state().clone();
    mtg_engine::fold::apply_event(
        &mut state,
        &Event::ZoneChanged {
            object,
            from,
            to,
            cause: ZoneCause::Moved,
        },
    )
    .unwrap();
    state
}

#[rstest]
fn an_art_sticker_put_offers_only_art_stickers(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Wicked", vec![free("Proficient Pyrodancer")]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear = bear_of(&driver, 0);
    driver.cast("Proficient Pyrodancer")?;
    let seen = settle(&mut driver, "art", Some(bear))?;
    assert_eq!(seen.offered.len(), 1);
    assert!(
        seen.offered[0].iter().all(|label| label.contains("art")),
        "{:?}",
        seen.offered
    );
    assert_eq!(seen.offered[0].len(), 2, "both art stickers, nothing else");
    assert_eq!(kinds_on(&driver, bear), [StickerKind::Art]);
    for seat in 1..seats {
        assert_eq!(kinds_on(&driver, bear_of(&driver, seat)), []);
    }
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn a_card_keeps_its_name_sticker_and_it_works_again_on_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Wicked", vec![free("_____ _____ _____ Trespasser")]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("_____ _____ _____ Trespasser")?;
    settle(&mut driver, "Wicked", None)?;
    let trespasser = driver.id_of("_____ _____ _____ Trespasser")?;
    assert_eq!(kinds_on(&driver, trespasser), [StickerKind::Name]);
    assert_eq!(
        driver.state().name_of(trespasser).unwrap(),
        "Wicked _____ _____ _____ Trespasser"
    );
    assert_replays(&driver, scenario);
    let graved = moved(
        &driver,
        trespasser,
        Zone::Battlefield,
        Zone::Graveyard(pid(0)),
    );
    assert_eq!(graved.stickers_on(trespasser).len(), 1, "the card keeps it");
    let back = {
        let mut state = graved;
        mtg_engine::fold::apply_event(
            &mut state,
            &Event::ZoneChanged {
                object: trespasser,
                from: Zone::Graveyard(pid(0)),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            },
        )
        .unwrap();
        state
    };
    assert_eq!(
        back.name_of(trespasser).unwrap(),
        "Wicked _____ _____ _____ Trespasser",
        "the name sticker works again"
    );
    Ok(())
}

#[rstest]
#[case::five_vowels("Aeiou", 5)]
#[case::y_counts("Rhythm", 1)]
#[case::repeats_once("Banana", 1)]
#[case::two("Wicked", 2)]
#[case::none("Grr", 0)]
fn the_unique_vowels_of_the_name_sticker_are_the_cards_looked_at(
    #[values(2, 3, 4)] seats: usize,
    #[case] words: &str,
    #[case] vowels: usize,
) -> Result<(), ScriptError> {
    let library: Vec<_> = (0..8).map(|_| bear()).collect();
    let scenario = table(seats, words, vec![free("Wizards of the _____")]).library(0, library);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.cast("Wizards of the _____")?;
    let seen = settle(&mut driver, words, None)?;
    // One card looked at is taken unasked.
    let asked: &[usize] = if vowels > 1 { &[vowels] } else { &[] };
    assert_eq!(seen.cards, asked, "the choice is among the cards looked at");
    let kept = usize::from(vowels > 0);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 + kept
    );
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 8 - kept);
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::art("art", true)]
#[case::name("Wicked", false)]
#[case::ability("Flying", false)]
fn putting_a_sticker_triggers_with_its_kind(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: &str,
    #[case] art: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Wicked", vec![un34_stickerer("Stickerer")])
        .battlefield(0, [ready(card("Wee Champion").unwrap()), ready(bear())])
        .graveyard(0, [card("Unlawful Entry").unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear = bear_of(&driver, 0);
    driver.cast("Stickerer")?;
    settle(&mut driver, pick, Some(bear))?;
    let champion = driver.id_of("Wee Champion")?;
    let state = driver.state();
    let counters = state
        .object(champion)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    let chars = mtg_engine::characteristics::characteristics(state, champion).unwrap();
    if art {
        assert_eq!((counters, chars.power), (1, Some(1)), "a counter instead");
    } else {
        assert_eq!(
            (counters, chars.power),
            (0, Some(1)),
            "+1/+1 until end of turn"
        );
    }
    let zone = driver
        .state()
        .objects()
        .find(|object| object.card().name == "Unlawful Entry")
        .unwrap()
        .zone();
    let expected = if art {
        Zone::Hand(pid(0))
    } else {
        Zone::Graveyard(pid(0))
    };
    assert_eq!(
        zone, expected,
        "only an art sticker on a creature returns it"
    );
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn up_to_two_art_stickers_count_on_the_battlefield_and_in_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Wicked", vec![free("Roxi, Publicist to the Stars")]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear = bear_of(&driver, 0);
    driver.cast("Roxi, Publicist to the Stars")?;
    settle(&mut driver, "art", Some(bear))?;
    assert_eq!(
        kinds_on(&driver, bear),
        [StickerKind::Art, StickerKind::Art]
    );
    let roxi = driver.id_of("Roxi, Publicist to the Stars")?;
    let power = |state: &mtg_engine::GameState| {
        mtg_engine::characteristics::characteristics(state, roxi)
            .unwrap()
            .power
    };
    assert_eq!(
        power(driver.state()),
        Some(1),
        "one permanent with an art sticker"
    );
    let graved = moved(&driver, bear, Zone::Battlefield, Zone::Graveyard(pid(0)));
    assert_eq!(
        power(&graved),
        Some(1),
        "the card in the graveyard still counts"
    );
    assert_replays(&driver, scenario);
    Ok(())
}
