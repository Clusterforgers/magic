//! Goal: Esper Terra's first chapter, reached by Terra's Trance, plays at two to four seats: it makes a token copy of
//! the target nonlegendary enchantment, the token has haste, a Saga token gets up to three more lore counters as its
//! controller chooses (none for any other enchantment, where nothing is asked), the token is sacrificed at the
//! beginning of its controller's next end step, and each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const TERRA: &str = "Terra, Magical Adept // Esper Terra";

/// Seat 0 controls Terra, `enchantment` and six lands, in its first main phase.
fn table(seats: usize, enchantment: &str) -> Driver {
    let mut mine = vec![
        ready(card(TERRA).unwrap()),
        ready(card(enchantment).unwrap()),
    ];
    mine.extend(vec![ready(mountain()); 3]);
    mine.extend(vec![ready(forest()); 3]);
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, mine),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    Driver::from_scenario(scenario).unwrap()
}

/// The tokens on the battlefield.
fn tokens(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
        .collect()
}

/// Activates the Trance and resolves until the chapter's token exists and the stack holds no chapter of Terra,
/// answering the number of lore counters with `lore`; how many times a number was asked.
fn trance(driver: &mut Driver, enchantment: &str, lore: u32) -> usize {
    let front = "Terra, Magical Adept";
    let terra = driver.id_of(front).unwrap();
    // A Saga set out on the battlefield reaches its first chapter at once: that trigger is left out.
    while !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        || !driver.state().stack().is_empty()
    {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseTargets(_)
        ) {
            driver.remove_trigger().unwrap();
        } else {
            driver.answer_passively().unwrap();
        }
    }
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!(
            "priority was expected: {:?}",
            driver.decision().unwrap().kind.name()
        );
    };
    let ability = *options
        .abilities
        .iter()
        .find(|ability| ability.object == terra)
        .unwrap();
    driver
        .answer(Answer::Priority(
            mtg_engine::decision::PriorityAction::Activate(ability),
        ))
        .unwrap();
    let mut asked = 0;
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::ChooseTargets(_) if tokens(driver).is_empty() => {
                let target = driver.target_named(enchantment).unwrap();
                driver.choose_targets(&[target]).unwrap();
            }
            DecisionKind::ChooseNumber(options) => {
                assert_eq!((options.min, options.max), (0, Some(3)), "up to three");
                asked += 1;
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(lore)))
                    .unwrap();
            }
            _ if !tokens(driver).is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    asked
}

#[rstest]
fn esper_terra_copies_an_enchantment_with_haste_until_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = table(seats, "Glorious Anthem");
    assert_eq!(
        trance(&mut driver, "Glorious Anthem", 0),
        0,
        "nothing is asked"
    );
    let made = tokens(&driver);
    assert_eq!(made.len(), 1);
    let token = made[0];
    assert_eq!(
        driver.state().object(token).unwrap().card().name,
        "Glorious Anthem"
    );
    assert!(
        mtg_engine::keywords::keywords_of(driver.state(), token)
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Haste))
    );
    driver.idle_until(2, Step::Upkeep).unwrap();
    assert!(tokens(&driver).is_empty(), "sacrificed at the end step");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn esper_terra_adds_up_to_three_lore_counters_to_a_saga_token(
    #[values(2, 4)] seats: usize,
    #[values(0, 1, 3)] lore: u32,
) {
    let saga = "Arni Slays the Troll";
    let mut driver = table(seats, saga);
    assert_eq!(trance(&mut driver, saga, lore), 1);
    let token = tokens(&driver)[0];
    let held = driver
        .state()
        .object(token)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Other("lore".into()));
    assert_eq!(held, 1 + lore, "one as it enters, then the chosen number");
}
