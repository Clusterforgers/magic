//! Goal: sticker kicker (Wicker Picker, the Unfinity rulings) plays at tables of two to four seats: each Wicker Picker offers its own sticker kicker {1} as a creature spell is cast, each paid one gives a ticket and offers a sticker the caster can then pay for, the sticker is on the spell before `SpellCast` and stays on the permanent it becomes, a sticker kicker declined gives nothing, a noncreature spell is offered none, and a creature with its own kicker counts only that kicker for "if this creature was kicked"; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::ability::Sticker;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{ChoiceOption, CostsChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{bear, forest, instant, un34_sheet};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 has `pickers` Wicker Pickers, six Forests, a Bear and an instant in hand, and a sheet with a name sticker of
/// one ticket and an ability sticker of two.
fn table(seats: usize, pickers: usize) -> Scenario {
    let mut mine: Vec<_> = (0..pickers)
        .map(|_| ready(card("Wicker Picker").unwrap()))
        .collect();
    mine.extend((0..6).map(|_| ready(forest())));
    let sheet = vec![
        Sticker::Name {
            words: "Wicked".to_owned(),
            tickets: 1,
        },
        Sticker::Ability {
            ability: Box::new(mtg_cards::ability::Ability::Keyword(
                mtg_cards::keyword::Keyword::Flying,
            )),
            tickets: 2,
        },
    ];
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [un34_sheet("Fun Sheet", sheet)])
        .battlefield(0, mine)
        .hand(0, [bear(), instant("Shock", 1, &[])])
}

/// What the cast asked: the optional costs offered and the sticker options of each question.
#[derive(Default)]
struct Seen {
    costs: Vec<String>,
    stickers: Vec<Vec<String>>,
}

/// Casts `name` and answers until the stack is empty: every optional cost when `kick`, else none; the sticker option
/// whose words contain `pick` (else the first); mana paid automatically.
fn cast(driver: &mut Driver, name: &str, kick: bool, pick: &str) -> Result<Seen, ScriptError> {
    let mut seen = Seen::default();
    driver.begin_cast(name)?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(seen);
            }
            DecisionKind::ChooseCosts(options) => {
                seen.costs = options
                    .optional
                    .iter()
                    .map(|cost| cost.label.clone())
                    .collect();
                let chosen = if kick {
                    options.optional.iter().map(|cost| cost.index).collect()
                } else {
                    Vec::new()
                };
                driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
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
                seen.stickers.push(labels);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
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

fn tickets(driver: &Driver) -> u32 {
    driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&CounterKind::Other("ticket".to_owned()))
}

fn assert_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn each_picker_gives_a_sticker_kicker_and_a_ticket(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] pickers: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, pickers);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let seen = cast(&mut driver, "Grizzly Bears", true, "Wicked")?;
    assert_eq!(seen.costs.len(), pickers, "{:?}", seen.costs);
    assert!(
        seen.costs
            .iter()
            .all(|label| label.contains("Sticker kicker"))
    );
    // The first kick's ticket pays the name sticker; the second's alone pays for no other.
    assert_eq!(seen.stickers.len(), 1, "{:?}", seen.stickers);
    assert_eq!(seen.stickers[0].len(), 2, "no sticker or the name sticker");
    let bear = driver.id_of("Grizzly Bears")?;
    assert_eq!(driver.state().stickers_on(bear).len(), 1);
    assert_eq!(
        driver.state().name_of(bear).unwrap(),
        "Wicked Grizzly Bears"
    );
    assert_eq!(tickets(&driver), u32::try_from(pickers).unwrap() - 1);
    let events = driver.engine().events();
    let put = events
        .iter()
        .position(|event| matches!(event, Event::StickerPut { .. }))
        .unwrap();
    let cast_at = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .unwrap();
    assert!(put < cast_at, "the sticker is on the spell as it is cast");
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn a_declined_or_skipped_sticker_kicker_puts_none(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] kick: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, 1);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    // Kicked with no sticker chosen: the ticket stays.
    cast(&mut driver, "Grizzly Bears", kick, "Put no sticker")?;
    let bear = driver.id_of("Grizzly Bears")?;
    assert_eq!(driver.state().stickers_on(bear).len(), 0);
    assert_eq!(tickets(&driver), u32::from(kick));
    assert_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn a_noncreature_spell_has_no_sticker_kicker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 2))?;
    let seen = cast(&mut driver, "Shock", true, "")?;
    assert_eq!(seen.costs.len(), 0, "{:?}", seen.costs);
    assert_eq!(tickets(&driver), 0);
    Ok(())
}

#[rstest]
#[case::sticker_kicker_only("Sticker kicker", 0)]
#[case::own_kicker("Kicker", 4)]
fn a_sticker_kicker_is_no_kick_of_the_spells_own(
    #[values(2, 3, 4)] seats: usize,
    #[case] paid: &str,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Wicker Picker").unwrap())];
    mine.extend((0..8).map(|_| ready(mtg_testkit::cards::island())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .hand(0, [card("Vodalian Serpent").unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.begin_cast("Vodalian Serpent")?;
    let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
        panic!("the costs are offered");
    };
    let chosen: Vec<usize> = options
        .optional
        .iter()
        .filter(|cost| cost.label.starts_with(paid))
        .map(|cost| cost.index)
        .collect();
    assert_eq!(chosen.len(), 1, "{:?}", options.optional);
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let serpent = driver.id_of("Vodalian Serpent")?;
    let permanent = driver.state().object(serpent).unwrap().permanent().unwrap();
    assert_eq!(
        permanent.counters_of(&CounterKind::PlusOnePlusOne),
        counters
    );
    assert_replays(&driver, scenario);
    Ok(())
}
