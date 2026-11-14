//! Goal: in a team game a creature is summoning sick until its own controller's turn begins (CR 302.6): a teammate's turn, an opponent's turn and a turn of another team change nothing for it, and the engine, the attackers a turn offers, every seat's projected view and the batches folded into those views agree on it, in Teams with teammates apart or side by side and in Two-Headed Giant, with haste, with a creature that was there all along and with a creature cast on a teammate's turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::summoning_sick;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ready, sick};
use rstest::rstest;

use crate::common::table::Table;

const KINDS: [&str; 4] = ["Entered", "Hasty", "Ready", "Cast"];

fn teams(mode: TeamMode, seats: &[u32]) -> Teams {
    Teams {
        mode,
        seats: seats.to_vec(),
    }
}

/// Every seat controls a creature that entered this turn, a hasty one that entered this turn and one that has been
/// there all along; `caster` also holds a creature to cast. The game begins in the main phase of turn `turn`.
fn board(teams: &Teams, turn: u32, active: usize, caster: usize) -> Scenario {
    let seats = teams.seats.len();
    let mut scenario = Scenario::new(seats)
        .house_rules(HouseRules {
            teams: Some(teams.clone()),
            ..HouseRules::default()
        })
        .at(turn, active, Step::Main1)
        .hand(caster, [free_creature(&format!("Cast {caster}"), 2, 2)]);
    for seat in 0..seats {
        scenario = scenario.battlefield(
            seat,
            [
                sick(free_creature(&format!("Entered {seat}"), 2, 2)),
                sick(keyworded(&format!("Hasty {seat}"), 2, 2, &[Keyword::Haste])),
                ready(free_creature(&format!("Ready {seat}"), 2, 2)),
            ],
        );
    }
    scenario
}

/// Lets priority go round to `seat`, who casts `name`, and lets it resolve, with every seat's view folded along.
fn cast(table: &mut Table, seat: usize, name: &str) {
    while table.driver.decision().unwrap().player.index() != seat {
        table.run(Driver::pass);
    }
    table.run(|driver| driver.cast(name));
    while !table.driver.state().stack().is_empty() {
        table.run(Driver::answer_passively);
    }
}

/// Whether `turn` is a turn of `seat`'s own: its own turn in Teams, its team's in Two-Headed Giant.
fn own_turn(teams: &Teams, turn: u32, seat: usize) -> bool {
    let seats = u32::try_from(teams.seats.len()).unwrap();
    let at = usize::try_from((turn - 1) % seats).unwrap();
    if teams.shared() {
        // The teams take turns 1, 3, 5 and 2, 4, 6, one team to a pair of seats.
        let team = (turn - 1) % 2;
        teams.seats[seat] == team
    } else {
        at == seat
    }
}

/// Whether the creature of `seat` that entered during turn `since` is held back at `turn`: its controller's turn has
/// not begun since.
fn held(teams: &Teams, since: u32, turn: u32, seat: usize) -> bool {
    !(since + 1..=turn).any(|each| own_turn(teams, each, seat))
}

/// Whether the seat's view of `id` says it is held back.
fn viewed(table: &Table, viewer: usize, id: ObjectId) -> bool {
    table.views[viewer]
        .objects
        .iter()
        .find(|object| object.id == id)
        .and_then(|object| object.permanent.as_ref())
        .map(|permanent| permanent.summoning_sick)
        .unwrap()
}

/// Plays passes until turn `last` has begun and checks, at every decision, that the creatures of every seat are held
/// back as `held` says. A creature named `Cast n` that is not on the battlefield yet is skipped.
fn watch(table: &mut Table, teams: &Teams, last: u32, held: impl Fn(&str, usize, u32) -> bool) {
    let seats = teams.seats.len();
    for _ in 0..4000 {
        let state = table.driver.state();
        let (turn, step) = (state.turn().number(), state.turn().step());
        if turn > last {
            return;
        }
        let offered: Vec<ObjectId> = match &table.driver.decision().unwrap().kind {
            DecisionKind::DeclareAttackers(options) => {
                options.attackers.iter().map(|each| each.object).collect()
            }
            _ => Vec::new(),
        };
        for seat in 0..seats {
            for kind in KINDS {
                let name = format!("{kind} {seat}");
                let Ok(id) = table.driver.id_of(&name) else {
                    continue;
                };
                let want = held(kind, seat, turn);
                let at = format!("turn {turn} {step:?}");
                assert_eq!(
                    summoning_sick(table.driver.state(), id),
                    want,
                    "{at}: the engine on {name}"
                );
                for viewer in 0..seats {
                    assert_eq!(
                        viewed(table, viewer, id),
                        want,
                        "{at}: seat {viewer}'s view of {name}"
                    );
                }
                // The creatures of the turn's players attack unless held back; nobody else's are offered.
                if step == Step::DeclareAttackers && own_turn(teams, turn, seat) {
                    assert_eq!(offered.contains(&id), !want, "{at}: attacking with {name}");
                }
            }
        }
        table.run(Driver::answer_passively);
    }
    panic!("the game did not reach turn {last}");
}

/// What a creature of each kind is at `turn` when it entered at the start turn (`Entered`, `Hasty`) or was cast then (`Cast`).
fn expect(teams: &Teams, start: u32) -> impl Fn(&str, usize, u32) -> bool + '_ {
    move |kind, seat, turn| match kind {
        "Entered" | "Cast" => held(teams, start, turn, seat),
        _ => false,
    }
}

#[rstest]
#[case::teams_apart_two(TeamMode::Teams, &[0, 1, 0, 1])]
#[case::teams_apart_three(TeamMode::Teams, &[0, 1, 0, 1, 0, 1])]
#[case::teams_side_by_side_two(TeamMode::Teams, &[0, 0, 1, 1])]
#[case::teams_side_by_side_three(TeamMode::Teams, &[0, 0, 0, 1, 1, 1])]
#[case::teams_three_teams(TeamMode::Teams, &[0, 1, 2, 0, 1, 2])]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, &[0, 0, 1, 1])]
fn a_creature_is_sick_until_its_controllers_own_turn_begins(
    #[case] mode: TeamMode,
    #[case] assignment: &[u32],
) {
    let teams = teams(mode, assignment);
    let seats = u32::try_from(assignment.len()).unwrap();
    let mut table = Table::new(board(&teams, 1, 0, 0));
    cast(&mut table, 0, "Cast 0");
    watch(&mut table, &teams, seats * 2 + 1, expect(&teams, 1));
}

#[rstest]
#[case::teams_a_teammate_apart(TeamMode::Teams, &[0, 1, 0, 1], 3, 2)]
#[case::teams_a_teammate_side_by_side(TeamMode::Teams, &[0, 0, 1, 1], 2, 1)]
#[case::teams_the_third_seat(TeamMode::Teams, &[0, 1, 0, 1, 0, 1], 5, 4)]
#[case::two_headed_giant_the_second_head(TeamMode::TwoHeadedGiant, &[0, 0, 1, 1], 1, 1)]
#[case::two_headed_giant_the_other_team(TeamMode::TwoHeadedGiant, &[0, 0, 1, 1], 2, 3)]
fn a_creature_cast_on_a_teammates_turn_is_sick_until_its_controllers_turn_begins(
    #[case] mode: TeamMode,
    #[case] assignment: &[u32],
    #[case] start: u32,
    #[case] caster: usize,
) {
    let teams = teams(mode, assignment);
    let seats = u32::try_from(assignment.len()).unwrap();
    let active = if teams.shared() {
        // The first seat of the team holds the turn.
        teams.members(teams.seats[caster]).next().unwrap().index()
    } else {
        caster
    };
    let mut table = Table::new(board(&teams, start, active, caster));
    cast(&mut table, caster, &format!("Cast {caster}"));
    watch(&mut table, &teams, start + seats * 2, expect(&teams, start));
}
