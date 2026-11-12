//! Goal: a keyword a static ability gives its controller ("You have hexproof.", "You have shroud.") is derived by the layers at tables of two, three and four seats: only the controller has it, it goes with the abilities of its source, it is counted once beside the keywords a player starts with, and it decides who may target the player with a spell.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::keywords::player_keywords;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

fn card(name: &str) -> Result<Arc<CardDefinition>, ScriptError> {
    mtg_testkit::corpus::card(name).map_err(|error| ScriptError::Stuck(error.to_string()))
}

/// What the player in `seat` has now.
fn held(driver: &Driver, seat: usize) -> Vec<Keyword> {
    player_keywords(driver.state(), pid(seat))
        .into_iter()
        .cloned()
        .collect()
}

fn legal(driver: &Driver) -> Vec<Target> {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("a target is asked for first");
    };
    options.legal.clone()
}

#[rstest]
#[case::spirit_two_seats("Spirit of the Hearth", Keyword::Hexproof, 2, 0)]
#[case::spirit_three_seats("Spirit of the Hearth", Keyword::Hexproof, 3, 1)]
#[case::spirit_four_seats("Spirit of the Hearth", Keyword::Hexproof, 4, 3)]
#[case::reformer_two_seats("Metropolis Reformer", Keyword::Hexproof, 2, 1)]
#[case::reformer_four_seats("Metropolis Reformer", Keyword::Hexproof, 4, 2)]
#[case::mask_two_seats("Ivory Mask", Keyword::Shroud, 2, 0)]
#[case::mask_three_seats("Ivory Mask", Keyword::Shroud, 3, 2)]
#[case::mask_four_seats("Ivory Mask", Keyword::Shroud, 4, 1)]
fn only_the_controller_of_the_permanent_has_the_keyword(
    #[case] name: &str,
    #[case] keyword: Keyword,
    #[case] seats: usize,
    #[case] holder: usize,
) {
    let scenario = Scenario::new(seats).battlefield(holder, [ready(card(name).unwrap())]);
    let driver = Driver::from_scenario(scenario).unwrap();
    for seat in 0..seats {
        let want = if seat == holder {
            vec![keyword.clone()]
        } else {
            Vec::new()
        };
        assert_eq!(held(&driver, seat), want, "seat {seat}");
    }
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_table_without_the_permanent_gives_nobody_the_keyword(#[case] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    for seat in 0..seats {
        assert_eq!(held(&driver, seat), Vec::<Keyword>::new());
    }
}

#[rstest]
#[case::one_of_each_kind(&["Spirit of the Hearth", "Ivory Mask"], &[Keyword::Hexproof, Keyword::Shroud])]
#[case::two_with_one_keyword(&["Spirit of the Hearth", "Metropolis Reformer"], &[Keyword::Hexproof])]
#[case::three_permanents(&["Spirit of the Hearth", "Metropolis Reformer", "Ivory Mask"], &[Keyword::Hexproof, Keyword::Shroud])]
fn several_sources_give_each_keyword_once(#[case] names: &[&str], #[case] want: &[Keyword]) {
    let board: Vec<_> = names
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect();
    let driver = Driver::from_scenario(Scenario::new(3).battlefield(1, board)).unwrap();
    assert_eq!(held(&driver, 1), want);
    assert_eq!(held(&driver, 0), Vec::<Keyword>::new());
}

#[test]
fn a_keyword_the_player_starts_with_is_not_listed_twice() {
    let scenario = Scenario::new(2)
        .battlefield(1, [ready(card("Spirit of the Hearth").unwrap())])
        .player_keywords(1, [Keyword::Hexproof]);
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(held(&driver, 1), [Keyword::Hexproof]);
    let scenario = Scenario::new(2)
        .battlefield(1, [ready(card("Ivory Mask").unwrap())])
        .player_keywords(1, [Keyword::Hexproof]);
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(held(&driver, 1), [Keyword::Hexproof, Keyword::Shroud]);
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_source_that_loses_its_abilities_gives_nothing(#[case] seats: usize) {
    let scenario =
        Scenario::new(seats).battlefield(1, [ready(card("Spirit of the Hearth").unwrap())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(held(&driver, 1), [Keyword::Hexproof]);
    let id = driver.id_of("Spirit of the Hearth").unwrap();
    driver
        .add_effect(
            driver.on(id),
            Modification::LoseAbilities,
            Duration::UntilEndOfTurn,
        )
        .unwrap();
    assert_eq!(held(&driver, 1), Vec::<Keyword>::new());
}

#[rstest]
#[case::hexproof_two_seats("Spirit of the Hearth", 2, 0, false)]
#[case::hexproof_three_seats("Spirit of the Hearth", 3, 2, false)]
#[case::hexproof_four_seats("Metropolis Reformer", 4, 3, false)]
#[case::hexproof_holder("Spirit of the Hearth", 3, 1, true)]
#[case::shroud_two_seats("Ivory Mask", 2, 0, false)]
#[case::shroud_four_seats("Ivory Mask", 4, 3, false)]
#[case::shroud_holder("Ivory Mask", 4, 1, false)]
fn a_player_with_the_keyword_is_offered_to_the_spells_it_allows(
    #[case] name: &str,
    #[case] seats: usize,
    #[case] caster: usize,
    #[case] holder_casts: bool,
) {
    // Seat 1 holds the permanent; `holder_casts` makes seat 1 the caster, which hexproof does not stop.
    let caster = if holder_casts { 1 } else { caster };
    let mut scenario = Scenario::new(seats).at(1, caster, Step::Main1);
    for seat in 0..seats {
        let mut board = vec![ready(mountain())];
        if seat == 1 {
            board.push(ready(card(name).unwrap()));
        }
        scenario = scenario.battlefield(seat, board);
    }
    let scenario = scenario.hand(caster, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Bolt").unwrap();
    let offered = legal(&driver);
    let holder = Target::Player(pid(1));
    let hexproof = name != "Ivory Mask";
    let may_target = caster == 1 && hexproof;
    assert_eq!(
        offered.contains(&holder),
        may_target,
        "{name}, seat {caster} casts"
    );
    let players = offered.iter().filter(|t| t.player().is_some()).count();
    assert_eq!(players, seats - usize::from(!may_target), "{name}");
    if !may_target {
        assert!(driver.choose_targets(&[holder]).is_err());
    }
}
