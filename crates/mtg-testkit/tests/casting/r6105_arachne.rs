//! Goal: as Arachne, Psionic Weaver enters, its controller looks at an opponent's hand and then chooses a card type other
//! than creature, which spells of that type then cost {1} more for; at two to four seats, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::Event;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, instant, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::casting::lands;

#[rstest]
fn r6105_arachne_looks_then_chooses_a_noncreature_type(
    #[values(2, 3, 4)] seats: usize,
    #[values("Instant", "Land")] pick: &str,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, lands(3, plains))
        .hand(0, vec![card("Arachne, Psionic Weaver").unwrap()]);
    for seat in 1..seats {
        scenario = scenario.hand(
            seat,
            vec![creature("Elf", 0, &[Color::Green], 1, 1), instant("Shock", 0, &[Color::Red])],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Arachne, Psionic Weaver")?;
    let mut offered = None;
    for _ in 0..64 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseOption(options)
                if options.options.iter().any(|option| matches!(option, ChoiceOption::Label(label) if label == "Land")) =>
            {
                let labels: Vec<String> = options
                    .options
                    .iter()
                    .filter_map(|option| match option {
                        ChoiceOption::Label(label) => Some(label.clone()),
                        _ => None,
                    })
                    .collect();
                let at = labels.iter().position(|label| label == pick).unwrap();
                offered = Some(labels);
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(0)),
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    let offered = offered.expect("the card type is asked");
    assert_eq!(offered.len(), 8, "{offered:?}");
    assert!(!offered.iter().any(|label| label == "Creature"));
    assert!(
        driver.engine().events().iter().any(|event| matches!(event, Event::HandLooked(_))),
        "the hand is looked at first"
    );
    let id = driver.id_of("Arachne, Psionic Weaver")?;
    let chosen = driver.state().object(id).unwrap().permanent().unwrap().chosen().cloned();
    let wanted = if pick == "Instant" { CardType::Instant } else { CardType::Land };
    assert_eq!(chosen, Some(Chosen::CardType(wanted)));
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
