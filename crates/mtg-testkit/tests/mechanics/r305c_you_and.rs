//! Goal: a static ability over "you and permanents you control" (Gor Muldrak, Serra's Emissary) gives its protection to its controller and to the permanents the selector names, never to an opponent or their permanents, at 2 to 4 seats; Serra's Emissary protects from the card type chosen as it entered, creatures and instants alike; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::replays;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::{keywords_of, player_keywords};
use mtg_testkit::cards::{creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn protection(preds: Vec<Pred>) -> Keyword {
    Keyword::Protection(Selector(preds))
}

fn creature_named(driver: &Driver, name: &str, seat: u64) -> mtg_core::ids::ObjectId {
    *driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .find(|id| {
            let object = driver.state().object(**id).unwrap();
            object.card().name == name && object.controller() == PlayerId(seat)
        })
        .unwrap()
}

#[rstest]
fn gor_muldrak_protects_you_and_your_permanents(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut scenario = Scenario::new(seats).battlefield(
            0,
            [
                ready(card("Gor Muldrak, Amphinologist").unwrap()),
                ready(creature("Mine", 1, &[Color::Green], 2, 2)),
            ],
        );
        for seat in 1..seats {
            scenario =
                scenario.battlefield(seat, [ready(creature("Theirs", 1, &[Color::Red], 2, 2))]);
        }
        scenario
    };
    let setup = scenario().setup().unwrap();
    let driver = Driver::from_scenario(scenario())?;
    let wanted = protection(vec![Pred::Subtype(Subtype("Salamander".into()))]);
    assert!(player_keywords(driver.state(), PlayerId(0)).contains(&&wanted));
    for name in ["Mine", "Gor Muldrak, Amphinologist"] {
        let id = creature_named(&driver, name, 0);
        assert!(keywords_of(driver.state(), id).contains(&&wanted), "{name}");
    }
    for seat in 1..seats as u64 {
        assert!(!player_keywords(driver.state(), PlayerId(seat)).contains(&&wanted));
        let id = creature_named(&driver, "Theirs", seat);
        assert!(!keywords_of(driver.state(), id).contains(&&wanted));
    }
    replays(&driver, &setup);
    Ok(())
}

/// Casts Serra's Emissary from seat 0's hand and names `kind` as it enters.
fn cast_emissary(driver: &mut Driver, kind: &str) -> Result<(), ScriptError> {
    driver.cast_tapping("Serra's Emissary", &["Plains"; 7])?;
    for _ in 0..100 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            let index = options
                .options
                .iter()
                .position(|option| match option {
                    ChoiceOption::Label(text) => text == kind,
                    other => format!("{other:?}").contains(kind),
                })
                .unwrap();
            driver.answer(Answer::ChooseOption(OptionChoice(index)))?;
            continue;
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the Emissary".into()))
}

#[rstest]
#[case::creature("Creature", CardType::Creature)]
#[case::instant("Instant", CardType::Instant)]
fn serras_emissary_protects_from_the_chosen_card_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] label: &str,
    #[case] kind: CardType,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Serra's Emissary").unwrap()])
            .battlefield(
                0,
                (0..7)
                    .map(|_| ready(plains()))
                    .chain(std::iter::once(ready(creature(
                        "Mine",
                        1,
                        &[Color::White],
                        2,
                        2,
                    )))),
            )
            .battlefield(1, [ready(creature("Theirs", 1, &[Color::Red], 2, 2))])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    cast_emissary(&mut driver, label)?;
    let wanted = protection(vec![Pred::Type(kind)]);
    assert!(player_keywords(driver.state(), PlayerId(0)).contains(&&wanted));
    assert!(keywords_of(driver.state(), creature_named(&driver, "Mine", 0)).contains(&&wanted));
    assert!(!player_keywords(driver.state(), PlayerId(1)).contains(&&wanted));
    assert!(!keywords_of(driver.state(), creature_named(&driver, "Theirs", 1)).contains(&&wanted));
    replays(&driver, &setup);
    Ok(())
}
