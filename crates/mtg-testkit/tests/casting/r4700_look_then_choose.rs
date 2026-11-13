//! Goal: a permanent that looks at an opponent's hand and then chooses a card type other than creature as it enters
//! (Arachne, Psionic Weaver) asks for the opponent (at three and four seats; at two the one opponent is taken), looks at
//! that hand before the type is asked, offers the eight card types other than creature, remembers the type, and spells
//! of that type cost {1} more for every player while spells of other types do not; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

const ARACHNE: &str = "Arachne, Psionic Weaver";

#[rstest]
#[case::two_seats_instant(2, 1, "Instant", "Lightning Bolt", "{1}{R}")]
#[case::three_seats_sorcery(3, 2, "Sorcery", "Lightning Bolt", "{R}")]
#[case::four_seats_instant(4, 3, "Instant", "Lightning Bolt", "{1}{R}")]
#[case::a_creature_is_untaxed(3, 1, "Instant", "Grizzly Bears", "{1}{G}")]
fn r4700_arachne_looks_then_taxes_the_chosen_type(
    #[case] seats: usize,
    #[case] looked: usize,
    #[case] kind: &str,
    #[case] spell: &str,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .at(1, 0, mtg_engine::turn::Step::Main1)
            .hand(
                0,
                [
                    card(ARACHNE).unwrap(),
                    card("Lightning Bolt").unwrap(),
                    bear(),
                ],
            )
            .hand(looked, [bear()])
            .battlefield(0, lands(8, plains))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast(ARACHNE)?;
    let mut asked = Vec::new();
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                assert_eq!(decision.player, pid(0));
                let want = if matches!(options.options[0], ChoiceOption::Player(_)) {
                    asked.push("opponent");
                    assert_eq!(
                        options.options.len(),
                        seats - 1,
                        "only opponents are offered"
                    );
                    ChoiceOption::Player(pid(looked))
                } else {
                    asked.push("type");
                    assert_eq!(options.options.len(), 8, "every card type but creature");
                    assert!(
                        !options
                            .options
                            .contains(&ChoiceOption::Label("Creature".to_owned()))
                    );
                    // The hand is looked at before the type is asked.
                    let looked_at = driver
                        .engine()
                        .events()
                        .iter()
                        .any(|event| matches!(event, Event::HandLooked(_)));
                    assert!(looked_at, "the hand is looked at first");
                    ChoiceOption::Label(kind.to_owned())
                };
                let at = options.options.iter().position(|o| *o == want).unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let expected: Vec<&str> = if seats > 2 {
        vec!["opponent", "type"]
    } else {
        vec!["type"]
    };
    assert_eq!(asked, expected);
    let id = driver.id_of(ARACHNE)?;
    let held = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    let want = match kind {
        "Instant" => CardType::Instant,
        _ => CardType::Sorcery,
    };
    assert_eq!(held, Some(Chosen::CardType(want)));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    driver.begin_cast(spell)?;
    if spell == "Lightning Bolt" {
        let at = driver.target_player(looked);
        driver.choose_targets(&[at])?;
    }
    assert_eq!(
        driver.pay_options()?.cost.to_string(),
        mtg_core::mana::ManaCost::parse(cost).unwrap().to_string()
    );
    Ok(())
}
