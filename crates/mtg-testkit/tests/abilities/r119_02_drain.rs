//! Goal: Drain Power makes its target activate a mana ability of each land they control (asking which of a land with
//! more than one, never of a tapped land) and moves all their unspent mana to its caster, and Balduvian Fallen gets
//! +1/+0 for each {B} or {R} spent on its cumulative upkeep and none for other mana; at two to four seats, and each game
//! replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ready, tapped};
use rstest::rstest;

/// The mana in a player's pool, by type in the order of `ManaType::ALL`.
fn pool(driver: &Driver, seat: u64) -> Vec<(ManaType, u32)> {
    let held = driver.state().player(PlayerId(seat)).unwrap().mana();
    ManaType::ALL
        .into_iter()
        .map(|mana| (mana, held.get(mana)))
        .filter(|(_, count)| *count > 0)
        .collect()
}

#[rstest]
#[case::glade_green(ManaType::Green, vec![(ManaType::Red, 2), (ManaType::Green, 1)])]
#[case::glade_red(ManaType::Red, vec![(ManaType::Red, 3)])]
fn drain_power_takes_the_mana_of_each_untapped_land(
    #[values(2, 3, 4)] seats: usize,
    #[case] taiga: ManaType,
    #[case] expected: Vec<(ManaType, u32)>,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(island()), ready(island())])
        .hand(0, [card("Drain Power").unwrap()])
        .battlefield(
            1,
            [
                ready(mountain()),
                ready(card("Cinder Glade").unwrap()),
                ready(mountain()),
                tapped(forest()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .cast_at("Drain Power", &[driver.target_player(1)])
        .unwrap();
    let mut asked = 0;
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                asked += 1;
                assert_eq!(decision.player, PlayerId(1));
                let wanted = format!("Add {{{}}}", taiga.symbol());
                let at = options
                    .options
                    .iter()
                    .position(|o| matches!(o, ChoiceOption::Label(l) if *l == wanted))
                    .unwrap();
                assert_eq!(
                    options.options.len(),
                    2,
                    "the Cinder Glade's two mana abilities"
                );
                driver
                    .answer(Answer::ChooseOption(OptionChoice(at)))
                    .unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert_eq!(asked, 1, "only the Cinder Glade asks");
    assert_eq!(pool(&driver, 0), expected);
    assert_eq!(pool(&driver, 1), Vec::new());
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn pygmy_hippo_drains_the_defender_into_the_next_main_phase(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] used: bool,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Pygmy Hippo").unwrap())])
        .battlefield(
            1,
            [
                ready(forest()),
                ready(island()),
                ready(card("Cinder Glade").unwrap()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let mut offered = false;
    for _ in 0..200 {
        let turn = driver.state().turn();
        if turn.step() == Step::Main2 && driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::DeclareAttackers(_) if decision.player == PlayerId(0) => {
                driver.attack(&[("Pygmy Hippo", 1)]).unwrap();
            }
            DecisionKind::YesNo(_) if decision.player == PlayerId(0) => {
                offered = true;
                driver.answer(Answer::YesNo(YesNoChoice(used))).unwrap();
            }
            DecisionKind::ChooseOption(_) => {
                assert_eq!(
                    decision.player,
                    PlayerId(1),
                    "the defender picks the Cinder Glade's mana"
                );
                driver
                    .answer(Answer::ChooseOption(OptionChoice(0)))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert!(offered);
    assert_eq!(driver.state().turn().step(), Step::Main2);
    let life = driver.state().player(PlayerId(1)).unwrap().life();
    if used {
        assert_eq!(life, 20, "the Hippo assigns no combat damage");
        assert_eq!(pool(&driver, 0), vec![(ManaType::Colorless, 3)]);
    } else {
        assert_eq!(life, 18);
        assert_eq!(pool(&driver, 0), Vec::new());
    }
    assert_eq!(pool(&driver, 1), Vec::new());
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::black_and_red(&["Swamp", "Mountain"], 5)]
#[case::black_only(&["Swamp", "Plains"], 4)]
#[case::neither(&["Plains", "Island"], 3)]
fn balduvian_fallen_counts_black_and_red_spent(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: &[&str],
    #[case] power: i32,
) {
    let fallen = with_counters(
        ready(card("Balduvian Fallen").unwrap()),
        CounterKind::Other("age".to_owned()),
        1,
    );
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            fallen,
            ready(swamp()),
            ready(mountain()),
            ready(plains()),
            ready(island()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)
        .unwrap();
    let mut paid = false;
    for _ in 0..60 {
        match &driver.decision().unwrap().kind {
            DecisionKind::YesNo(_) if !paid => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::PayMana(_) => {
                paid = true;
                driver.pay_tapping(lands, 0).unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert!(paid);
    let id = driver.id_of("Balduvian Fallen").unwrap();
    assert_eq!(
        characteristics(driver.state(), id).unwrap().power,
        Some(power)
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}
