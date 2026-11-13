//! Goal: a land that chooses a basic land type as it enters and then asks for 2 life (Multiversal Passage) asks both
//! questions as it is played at two, three and four seats, enters untapped only when the life is paid, is exactly the
//! chosen type and taps for that type's mana and no other, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::card::Subtype;
use mtg_core::mana::ManaType;
use mtg_engine::choose::{ChoiceOption, OptionChoice, ReplacementChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{characteristics, replay};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

const PASSAGE: &str = "Multiversal Passage";

#[rstest]
#[case::island_paid("Island", ManaType::Blue, ManaType::Red, true)]
#[case::island_declined("Island", ManaType::Blue, ManaType::Red, false)]
#[case::mountain_paid("Mountain", ManaType::Red, ManaType::Green, true)]
#[case::forest_declined("Forest", ManaType::Green, ManaType::White, false)]
fn the_passage_is_the_chosen_type_and_enters_tapped_unless_life_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: &str,
    #[case] makes: ManaType,
    #[case] not: ManaType,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let scenario = || Scenario::new(seats).hand(0, [card(PASSAGE).unwrap()]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.play_land(PASSAGE)?;
    let (mut chose, mut asked) = (false, false);
    for _ in 0..6 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseReplacement(_) => Answer::ChooseReplacement(ReplacementChoice(0)),
            DecisionKind::ChooseOption(options) => {
                chose = true;
                let at = options
                    .options
                    .iter()
                    .position(
                        |option| matches!(option, ChoiceOption::Label(label) if label == kind),
                    )
                    .expect("every basic land type is offered");
                assert_eq!(options.options.len(), 5, "the five basic land types");
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::YesNo(question) => {
                asked = true;
                assert_eq!(question.text, "Pay 2 life?");
                Answer::YesNo(YesNoChoice(pays))
            }
            _ => break,
        };
        assert_eq!(decision.player, pid(0));
        driver.answer(answer)?;
    }
    assert!(chose && asked, "both questions are asked");
    let id = driver.id_of(PASSAGE).unwrap();
    let held = driver.state().object(id).unwrap().permanent().unwrap();
    assert_eq!(held.tapped(), !pays);
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, if pays { 18 } else { 20 });
    let chars = characteristics(driver.state(), id).unwrap();
    assert_eq!(chars.subtypes, vec![Subtype(kind.to_owned())]);
    if pays {
        assert!(driver.tap_for_mana_of(PASSAGE, not).is_err());
        driver.tap_for_mana_of(PASSAGE, makes)?;
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
