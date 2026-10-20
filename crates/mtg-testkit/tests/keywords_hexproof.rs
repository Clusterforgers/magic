//! Goal: hexproof decides who may target a permanent or a player along the whole path of a cast or an activation, at tables of two, three and four seats: the targets offered to a spell or an ability leave out what hexproof keeps from its controller, the answer that names one is refused without a trace, its holder's own spells and abilities still reach it, and a spell with nothing left to target is not offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{name_of, pid};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, drain, forest, free_creature, keyworded, mountain, pinger, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// What puts the targets to the choice: a spell cast from hand or an ability of a permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Spell,
    Ability,
}

/// `seats` seats and `caster` on turn with priority. Every seat controls a Bear named after it; seat 1 also controls
/// a hexproof Shade. The caster has a Bolt and a Mountain, or a Pinger, whichever `source` asks for.
fn table(seats: usize, caster: usize, source: Source) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, caster, Step::Main1);
    for seat in 0..seats {
        let mut board = vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))];
        if seat == 1 {
            board.push(ready(keyworded("Shade", 2, 2, &[Keyword::Hexproof])));
        }
        if seat == caster {
            match source {
                Source::Spell => board.push(ready(mountain())),
                Source::Ability => board.push(ready(pinger("Pinger", 0))),
            }
        }
        scenario = scenario.battlefield(seat, board);
    }
    if source == Source::Spell {
        scenario = scenario.hand(caster, [burn("Bolt", 3)]);
    }
    scenario
}

fn begin(driver: &mut Driver, source: Source) -> Result<(), ScriptError> {
    match source {
        Source::Spell => driver.begin_cast("Bolt"),
        Source::Ability => driver.begin_activation("Pinger").map(|_| ()),
    }
}

fn legal(driver: &Driver) -> Vec<Target> {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("a target is asked for first");
    };
    options.legal.clone()
}

#[rstest]
fn what_hexproof_keeps_from_a_controller_is_not_offered_to_a_spell_or_an_ability(
    #[values(2, 3, 4)] seats: usize,
    #[values(Source::Spell, Source::Ability)] source: Source,
    #[values(0, 1)] caster: usize,
) {
    let mut driver = Driver::from_scenario(table(seats, caster, source)).unwrap();
    begin(&mut driver, source).unwrap();
    let offered = legal(&driver);
    let shade = driver.target_named("Shade").unwrap();
    let own = caster == 1;
    assert_eq!(offered.contains(&shade), own);
    // Every player and every Bear stays, and a Pinger is a creature too.
    let players = offered.iter().filter(|t| t.player().is_some()).count();
    assert_eq!(players, seats);
    let creatures = offered.len() - players;
    let pinger = usize::from(source == Source::Ability);
    assert_eq!(creatures, seats + pinger + usize::from(own));
    // The engine refuses an answer that names a target it did not offer, and nothing changes.
    if !own {
        let before = (driver.engine().events().len(), driver.engine().state_hash());
        assert!(driver.choose_targets(&[shade]).is_err());
        assert_eq!(
            (driver.engine().events().len(), driver.engine().state_hash()),
            before
        );
    }
}

#[rstest]
fn the_holder_of_hexproof_can_still_hit_it_with_its_own_spell_or_ability(
    #[values(2, 3, 4)] seats: usize,
    #[values(Source::Spell, Source::Ability)] source: Source,
) {
    let mut driver = Driver::from_scenario(table(seats, 1, source)).unwrap();
    let shade = driver.target_named("Shade").unwrap();
    begin(&mut driver, source).unwrap();
    driver.choose_targets(&[shade]).unwrap();
    if source == Source::Spell {
        driver
            .answer(mtg_engine::Answer::PayMana(
                mtg_engine::decision::PayChoice::Auto,
            ))
            .unwrap();
    }
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    match source {
        Source::Spell => driver
            .expect_zone(Zone::Graveyard(pid(1)), &["Bolt", "Shade"])
            .or_else(|_| driver.expect_zone(Zone::Graveyard(pid(1)), &["Shade", "Bolt"]))
            .unwrap(),
        Source::Ability => {
            let id = driver.id_of("Shade").unwrap();
            let marked = driver.state().object(id).unwrap().permanent().unwrap();
            assert_eq!(marked.damage(), 1);
        }
    }
}

#[rstest]
#[case::two_seats_opponent(2, 0)]
#[case::two_seats_holder(2, 1)]
#[case::three_seats_first_opponent(3, 0)]
#[case::three_seats_holder(3, 1)]
#[case::three_seats_second_opponent(3, 2)]
#[case::four_seats_first_opponent(4, 0)]
#[case::four_seats_holder(4, 1)]
#[case::four_seats_last_opponent(4, 3)]
fn a_hexproof_player_is_offered_to_their_own_spells_alone(
    #[case] seats: usize,
    #[case] caster: usize,
) {
    let scenario = (0..seats)
        .fold(
            Scenario::new(seats).at(1, caster, Step::Main1),
            |s, seat| s.battlefield(seat, [ready(mountain())]),
        )
        .hand(caster, [burn("Bolt", 3)])
        .player_keywords(1, [Keyword::Hexproof]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Bolt").unwrap();
    let offered = legal(&driver);
    let holder = Target::Player(PlayerId(1));
    assert_eq!(offered.contains(&holder), caster == 1);
    let players = offered.iter().filter(|t| t.player().is_some()).count();
    assert_eq!(players, seats - usize::from(caster != 1));
    if caster != 1 {
        assert!(driver.choose_targets(&[holder]).is_err());
    }
}

#[rstest]
#[case::two_seats_nothing_left(2, &[])]
#[case::three_seats_one_left(3, &[2])]
#[case::four_seats_two_left(4, &[2, 3])]
fn a_spell_for_opponents_leaves_out_a_hexproof_opponent_and_is_not_offered_with_none_left(
    #[case] seats: usize,
    #[case] left: &[usize],
) {
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |s, seat| {
            s.battlefield(seat, [ready(forest())])
        })
        .battlefield(0, [ready(swamp()), ready(swamp())])
        .hand(0, [drain("Drain")])
        .player_keywords(1, [Keyword::Hexproof]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("seat 0 has priority");
    };
    let castable: Vec<String> = options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    if left.is_empty() {
        assert!(castable.is_empty(), "{castable:?}");
        assert!(driver.begin_cast("Drain").is_err());
        return;
    }
    assert_eq!(castable, ["Drain"]);
    driver.begin_cast("Drain").unwrap();
    let want: Vec<Target> = left
        .iter()
        .map(|seat| driver.target_player(*seat))
        .collect();
    assert_eq!(legal(&driver), want);
}
