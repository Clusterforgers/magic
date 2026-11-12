//! Goal: Ghostly Dancers' "return an enchantment card from your graveyard to your hand or unlock a locked door of a Room you control" is a choice between the two effects as the ability resolves: the controller is asked for one, only that one is done, and a choice that cannot be carried out does nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::plains;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

const ROOM: &str = "Grand Entryway // Elegant Rotunda";

fn scenario(room: bool, graveyard: bool) -> Scenario {
    let mut battlefield: Vec<_> = (0..6).map(|_| ready(plains())).collect();
    if room {
        battlefield.push(ready(mtg_testkit::corpus::card(ROOM).unwrap()));
    }
    Scenario::new(2)
        .library(0, vec![plains(); 12])
        .library(1, vec![plains(); 12])
        .hand_named(0, ["Ghostly Dancers"])
        .graveyard_named(
            0,
            if graveyard {
                vec!["Ghostly Prison"]
            } else {
                vec![]
            },
        )
        .battlefield(0, battlefield)
}

/// Casts Ghostly Dancers and lets it and its enters trigger resolve until the choice between the two effects is asked.
fn until_the_choice(driver: &mut Driver) -> Result<Vec<String>, ScriptError> {
    driver.cast("Ghostly Dancers")?;
    for _ in 0..12 {
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            return Ok(options
                .options
                .iter()
                .map(|option| match option {
                    ChoiceOption::Label(text) => text.clone(),
                    other => panic!("an option with a label, not {other:?}"),
                })
                .collect());
        }
        driver.pass()?;
    }
    panic!("the choice is never asked");
}

/// Answers whatever is asked passively until seat 0 holds priority on an empty stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?;
        let idle = decision.player == pid(0)
            && matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().zone(Zone::Stack).is_empty();
        if idle {
            return Ok(());
        }
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("the stack does not settle");
}

fn in_hand(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

fn in_graveyard(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

fn doors(driver: &Driver) -> Option<[bool; 2]> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .find_map(|id| driver.state().dk1c5_doors(*id))
}

#[rstest]
#[case::return_the_enchantment(0, true, true, true, Some([false, false]))]
#[case::unlock_a_door(1, true, true, false, Some([true, false]))]
#[case::return_with_no_enchantment_to_return(0, false, true, false, Some([false, false]))]
#[case::unlock_with_no_room(1, true, false, false, None)]
fn only_the_option_chosen_is_done(
    #[case] pick: usize,
    #[case] graveyard: bool,
    #[case] room: bool,
    #[case] returned: bool,
    #[case] unlocked: Option<[bool; 2]>,
) -> Result<(), ScriptError> {
    let build = || scenario(room, graveyard);
    let setup: GameSetup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    let options = until_the_choice(&mut driver)?;
    assert_eq!(options.len(), 2, "{options:?}");
    assert!(
        options[0].starts_with("Return an enchantment card") && options[1].starts_with("Unlock"),
        "the options are the words of the two effects: {options:?}"
    );
    assert_eq!(driver.decision()?.player, pid(0), "the controller chooses");
    driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
    settle(&mut driver)?;
    assert_eq!(in_hand(&driver, "Ghostly Prison"), returned);
    assert_eq!(
        in_graveyard(&driver, "Ghostly Prison"),
        graveyard && !returned
    );
    assert_eq!(doors(&driver), unlocked);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
