//! Goal: the abilities of ability stickers on other objects play at tables of two to four seats: Clandestine Chameleon has the abilities of the ability stickers on other permanents its controller owns and on cards in their graveyard (a keyword and an activated ability alike, a name sticker giving nothing, none once the card leaves the graveyard), Pin Collection puts an ability sticker of ticket cost X or less on itself without paying and its equipped creature has that sticker's ability; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Sticker};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, ChoiceOption, NumberChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ZoneCause;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{bear, cost, plains, un34_sheet};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The corpus card, free to cast.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(cost(0, &[]));
    Arc::new(definition)
}

/// An ability sticker of the ticket cost.
fn ability(ability: Ability, tickets: u32) -> Sticker {
    Sticker::Ability {
        ability: Box::new(ability),
        tickets,
    }
}

/// "{0}: You gain 1 life."
fn gain_one() -> Ability {
    serde_json::from_str(r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#)
        .unwrap()
}

/// Seat 0 brings the sheet, has the cards in hand, a Bear and `lands` Plains; every other seat has a Bear.
fn table(
    seats: usize,
    sheet: Vec<Sticker>,
    hand: Vec<Arc<CardDefinition>>,
    lands: usize,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [un34_sheet("Fun Sheet", sheet)])
        .hand(0, hand);
    for seat in 0..seats {
        let plains = (0..if seat == 0 { lands } else { 0 }).map(|_| ready(plains()));
        scenario = scenario.battlefield(seat, std::iter::once(ready(bear())).chain(plains));
    }
    scenario
}

/// Answers until the stack is empty: yes to every question, the first sticker whose words contain `pick`, `onto` when it
/// is offered (else the first), `x` for a number, the first target; the sticker options offered are returned.
fn settle(
    driver: &mut Driver,
    pick: &str,
    onto: Option<ObjectId>,
    x: u32,
) -> Result<Vec<Vec<String>>, ScriptError> {
    let mut offered = Vec::new();
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(offered);
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
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
                    .position(|label| label.to_lowercase().contains(&pick.to_lowercase()))
                    .unwrap_or(0);
                offered.push(labels);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = onto
                    .filter(|id| options.from.contains(id))
                    .or_else(|| options.from.first().copied());
                driver.answer(Answer::ChooseCards(CardsChoice(
                    first.into_iter().collect(),
                )))?;
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

fn assert_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn moved(
    state: &mtg_engine::GameState,
    object: ObjectId,
    from: Zone,
    to: Zone,
) -> mtg_engine::GameState {
    let mut state = state.clone();
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
#[case::flying("Flying", true)]
#[case::name("Wicked", false)]
fn the_chameleon_has_the_abilities_of_stickers_on_what_its_controller_owns(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: &str,
    #[case] flies: bool,
) -> Result<(), ScriptError> {
    let sheet = vec![
        Sticker::Name {
            words: "Wicked".to_owned(),
            tickets: 0,
        },
        ability(Ability::Keyword(Keyword::Flying), 1),
    ];
    let scenario = table(seats, sheet, vec![free("Clandestine Chameleon")], 0);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear = bear_of(&driver, 0);
    driver.cast("Clandestine Chameleon")?;
    settle(&mut driver, pick, Some(bear), 0)?;
    let chameleon = driver.id_of("Clandestine Chameleon")?;
    let state = driver.state();
    assert_eq!(has_keyword(state, bear, &Keyword::Flying), flies);
    assert_eq!(has_keyword(state, chameleon, &Keyword::Flying), flies);
    for seat in 1..seats {
        assert!(!has_keyword(
            state,
            bear_of(&driver, seat),
            &Keyword::Flying
        ));
    }
    assert_replays(&driver, scenario);
    let graved = moved(state, bear, Zone::Battlefield, Zone::Graveyard(pid(0)));
    assert_eq!(
        has_keyword(&graved, chameleon, &Keyword::Flying),
        flies,
        "a card in the graveyard still lends its sticker"
    );
    let exiled = moved(&graved, bear, Zone::Graveyard(pid(0)), Zone::Exile);
    assert!(
        !has_keyword(&exiled, chameleon, &Keyword::Flying),
        "an exiled card lends nothing"
    );
    Ok(())
}

#[rstest]
fn the_chameleon_activates_a_stickers_ability(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        vec![ability(gain_one(), 0)],
        vec![free("Clandestine Chameleon")],
        0,
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear = bear_of(&driver, 0);
    driver.cast("Clandestine Chameleon")?;
    settle(&mut driver, "life", Some(bear), 0)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(
        driver.offered_abilities("Clandestine Chameleon")?.len(),
        1,
        "the sticker's ability, which the card does not print"
    );
    driver.begin_activation("Clandestine Chameleon")?;
    settle(&mut driver, "", None, 0)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life + 1);
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::zero(0, &["Trample"], "Trample")]
#[case::two(2, &["Trample", "Flying", "Haste"], "Haste")]
fn pin_collection_puts_a_sticker_of_ticket_cost_x_or_less_for_free(
    #[values(2, 3, 4)] seats: usize,
    #[case] x: u32,
    #[case] offered: &[&str],
    #[case] pick: &str,
) -> Result<(), ScriptError> {
    let sheet = vec![
        ability(Ability::Keyword(Keyword::Trample), 0),
        ability(Ability::Keyword(Keyword::Flying), 1),
        ability(Ability::Keyword(Keyword::Haste), 2),
        ability(Ability::Keyword(Keyword::Vigilance), 3),
        Sticker::Name {
            words: "Wicked".to_owned(),
            tickets: 0,
        },
    ];
    let scenario = table(seats, sheet, vec![card("Pin Collection").unwrap()], 5);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear = bear_of(&driver, 0);
    driver.begin_cast("Pin Collection")?;
    let seen = settle(&mut driver, pick, None, x)?;
    // A single sticker is put on without a question.
    let labels = seen.first().cloned().unwrap_or_default();
    let asked = if offered.len() > 1 { offered.len() } else { 0 };
    assert_eq!(labels.len(), asked, "{seen:?}");
    for (label, word) in labels.iter().zip(offered) {
        assert!(label.contains(word), "{label} offers {word}");
    }
    let pin = driver.id_of("Pin Collection")?;
    assert_eq!(driver.state().stickers_on(pin).len(), 1);
    let tickets = mtg_cards::value::CounterKind::Other("ticket".to_owned());
    assert_eq!(
        driver.state().player(pid(0)).unwrap().counters_of(&tickets),
        0,
        "nothing is paid"
    );
    let keyword = match pick {
        "Haste" => Keyword::Haste,
        _ => Keyword::Trample,
    };
    assert!(!has_keyword(driver.state(), bear, &keyword));
    driver.begin_activation("Pin Collection")?;
    settle(&mut driver, "", Some(bear), 0)?;
    let state = driver.state();
    assert!(
        has_keyword(state, bear, &keyword),
        "the equipped creature has it"
    );
    let chars = mtg_engine::characteristics::characteristics(state, bear).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    for seat in 1..seats {
        assert!(!has_keyword(state, bear_of(&driver, seat), &keyword));
    }
    assert_replays(&driver, scenario);
    Ok(())
}
