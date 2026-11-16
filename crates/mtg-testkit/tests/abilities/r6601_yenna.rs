//! Goal: Yenna, Redtooth Regent plays as printed at two to four seats: it targets only an enchantment you control whose
//! name no other permanent you control has, the token copy isn't legendary (a legendary original and its copy both
//! stay), and only an Aura token untaps Yenna and scries 2; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::Supertype;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const YENNA: &str = "Yenna, Redtooth Regent";

/// The objects on the battlefield with the name.
fn r6601_named(driver: &Driver, name: &str) -> Vec<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .collect()
}

#[rstest]
#[case::an_enchantment("Glorious Anthem", false)]
#[case::a_legendary_enchantment("Teferi's Ageless Insight", false)]
#[case::an_aura("Pacifism", true)]
fn r6601_yenna_copies_a_unique_enchantment(
    #[values(2, 3, 4)] seats: usize,
    #[case] original: &str,
    #[case] aura: bool,
) {
    let mut mine = vec![
        ready(card(YENNA).unwrap()),
        ready(forest()),
        ready(plains()),
        ready(free_creature("Bear", 2, 2)),
    ];
    let enchantment = ready(card(original).unwrap());
    mine.push(if aura {
        attached_to(enchantment, 0, 3)
    } else {
        enchantment
    });
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, mine)
        .library(0, (0..5).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.begin_activation(YENNA).unwrap();
    let target = driver.target_named(original).unwrap();
    driver.choose_targets(&[target]).unwrap();
    let mut scried = false;
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::Scry(_) => {
                scried = true;
                driver.answer_passively().unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let copies = r6601_named(&driver, original);
    assert_eq!(copies.len(), 2, "the original and its copy");
    let token = copies
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
        .unwrap();
    let chars = characteristics(driver.state(), token).unwrap();
    assert!(
        !chars.supertypes.contains(&Supertype::Legendary)
            || chars.supertypes_removed.contains(&Supertype::Legendary)
    );
    let yenna = driver.id_of(YENNA).unwrap();
    let tapped = driver
        .state()
        .object(yenna)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(tapped, !aura, "only an Aura token untaps Yenna");
    assert_eq!(scried, aura);
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn r6601_yenna_cant_target_a_shared_name(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats).at(3, 0, Step::Main1).battlefield(
        0,
        [
            ready(card(YENNA).unwrap()),
            ready(forest()),
            ready(plains()),
            ready(card("Glorious Anthem").unwrap()),
            ready(card("Glorious Anthem").unwrap()),
            ready(card("Rhystic Study").unwrap()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation(YENNA).unwrap();
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("the target is chosen first");
    };
    let study = driver.target_named("Rhystic Study").unwrap();
    assert_eq!(
        options.legal,
        vec![study],
        "only the enchantment with a name of its own"
    );
}
