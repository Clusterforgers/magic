//! Goal: "Discard the last card you drew this turn" as a cost plays as printed at tables of two, three and four seats:
//! Jandor's Ring can't be activated before its controller drew a card this turn; after one or two draws it discards
//! exactly the card drawn last (never another card of the hand, nor a card drawn earlier) and draws a card; the log
//! replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::pay;
use crate::common::pid;

const RING: &str = "Jandor's Ring";

fn r5500_card(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{5}").unwrap()),
    )
}

fn r5500_hand(driver: &Driver) -> Vec<ObjectId> {
    driver.state().zone(Zone::Hand(pid(0))).to_vec()
}

fn r5500_ring_offered(driver: &Driver) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options.abilities.iter().any(|ability| {
        driver
            .state()
            .object(ability.object)
            .is_some_and(|object| object.card().name == RING)
    }))
}

/// Passes until seat 0 holds priority with an empty stack.
fn r5500_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        let decision = driver.decision()?;
        if decision.player == pid(0)
            && matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("seat 0 never held priority".to_owned()))
}

/// Seat 0 holds the Ring, five Islands, a Keeper card and `opts` Opts; it casts each Opt (scry 1, draw), then activates
/// the Ring when it can: only the card drawn last goes to the graveyard and a card is drawn.
#[rstest]
#[case::no_draw(0)]
#[case::one_draw(1)]
#[case::two_draws(2)]
fn r5500_the_ring_discards_the_card_drawn_last(
    #[values(2, 3, 4)] seats: usize,
    #[case] opts: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card(RING).unwrap())];
    mine.extend((0..5).map(|_| ready(island())));
    let mut hand = vec![r5500_card("Keeper")];
    hand.extend((0..opts).map(|_| card("Opt").unwrap()));
    let library: Vec<_> = (0..8).map(|i| r5500_card(&format!("Page {i}"))).collect();
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .library(0, library)
        .hand(0, hand);
    let mut driver = Driver::from_scenario(scenario)?;
    let mut drawn = Vec::new();
    for _ in 0..opts {
        let before = r5500_hand(&driver);
        driver.cast_tapping("Opt", &["Island"])?;
        r5500_settle(&mut driver)?;
        drawn.extend(
            r5500_hand(&driver)
                .into_iter()
                .filter(|id| !before.contains(id)),
        );
    }
    assert_eq!(drawn.len(), opts);
    assert_eq!(r5500_ring_offered(&driver)?, opts > 0);
    if opts == 0 {
        return Ok(());
    }
    let keeper = *r5500_hand(&driver)
        .iter()
        .find(|id| driver.state().object(**id).unwrap().card().name == "Keeper")
        .unwrap();
    let last = *drawn.last().unwrap();
    driver.begin_activation(RING)?;
    for _ in 0..20 {
        if driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.decision()?.player == pid(0)
            && driver.state().object(last).unwrap().zone() != Zone::Hand(pid(0))
        {
            break;
        }
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            assert_eq!(options.from, [last], "only the card drawn last");
        }
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            pay(&mut driver)?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert_eq!(
        driver.state().object(last).unwrap().zone(),
        Zone::Graveyard(pid(0))
    );
    assert_eq!(
        driver.state().object(keeper).unwrap().zone(),
        Zone::Hand(pid(0))
    );
    for earlier in &drawn[..drawn.len() - 1] {
        assert_eq!(
            driver.state().object(*earlier).unwrap().zone(),
            Zone::Hand(pid(0))
        );
    }
    // The Ring drew a card of its own, which is now the last drawn.
    assert_eq!(r5500_hand(&driver).len(), 1 + drawn.len());
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
