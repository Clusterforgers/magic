//! Goal: Gandalf of the Secret Fire exiles an instant its controller casts from their hand on an opponent's turn as it
//! resolves, with three time counters, and the card gains suspend (CR 702.62); on its controller's own turn the spell
//! goes to the graveyard as usual.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{instant, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

#[rstest]
fn r111_an_instant_cast_on_an_opponents_turn_is_suspended(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] their_turn: bool,
) -> Result<(), ScriptError> {
    let active = usize::from(their_turn);
    let mut permanents = lands(2, island);
    permanents.push(ready(card("Gandalf of the Secret Fire").unwrap()));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, active, Step::Main1)
            .hand(0, [instant("Quick Word", 1, &[Color::Blue])])
            .battlefield(0, permanents),
    )?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    driver.cast_tapping("Quick Word", &["Island", "Island"])?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let word = driver
        .state()
        .objects()
        .find(|object| object.card().name == "Quick Word")
        .unwrap();
    if !their_turn {
        assert_eq!(word.zone(), Zone::Graveyard(pid(0)));
        return Ok(());
    }
    assert_eq!(word.zone(), Zone::Exile);
    assert_eq!(
        word.card_counters_of(&CounterKind::Other("time".to_owned())),
        3
    );
    assert!(
        mtg_engine::keywords::keywords_of(driver.state(), word.id())
            .iter()
            .any(|keyword| matches!(keyword, Keyword::Suspend { .. }))
    );
    Ok(())
}
