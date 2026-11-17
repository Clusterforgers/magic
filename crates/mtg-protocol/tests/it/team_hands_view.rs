//! Goal: at a table with teams (2v2, Two-Headed Giant, a Delves party of three against one) each seat's view lists its teammates' hands by default and no opponent's, a spectator's lists none, libraries stay counts for everyone, `team_hands` off hides a teammate's hand again, and the batches of a passive turn keep every viewer's folded view equal to its snapshot.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::zones::Zone;
use mtg_protocol::{StateView, project_events, spectator_viewer};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn card(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// A table of `teams` where every seat holds uniquely named cards in hand and library.
fn table(mode: TeamMode, teams: &[u32], team_hands: Option<bool>) -> Scenario {
    let mut scenario = Scenario::new(teams.len()).house_rules(HouseRules {
        teams: Some(Teams {
            mode,
            seats: teams.to_vec(),
        }),
        team_hands,
        ..HouseRules::default()
    });
    for n in 0..teams.len() {
        scenario = scenario
            .hand(
                n,
                [card(&format!("Hand {n}")), card(&format!("Hand {n} b"))],
            )
            .library(n, (0..3).map(|c| card(&format!("Library {n} {c}"))));
    }
    scenario
}

#[rstest]
#[case::two_v_two(TeamMode::Teams, &[0, 1, 0, 1])]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, &[0, 0, 1, 1])]
#[case::delves_party(TeamMode::SharedTurns, &[0, 0, 0, 1])]
fn a_seat_sees_its_teammates_hands_and_no_one_elses(
    #[case] mode: TeamMode,
    #[case] teams: &[u32],
    #[values(None, Some(true), Some(false))] team_hands: Option<bool>,
) {
    let driver = Driver::from_scenario(table(mode, teams, team_hands)).unwrap();
    let state = driver.state();
    let shown = team_hands != Some(false);
    let spectator = spectator_viewer(state);
    for viewer in (0..teams.len()).map(seat).chain([spectator]) {
        let view = StateView::for_player(state, viewer);
        let text = serde_json::to_string(&view).unwrap();
        for (owner, team) in teams.iter().enumerate() {
            let player = &view.table.players[owner];
            let hand = state.zone(Zone::Hand(seat(owner))).to_vec();
            let mate = viewer.index() < teams.len() && teams[viewer.index()] == *team;
            let sees = viewer == seat(owner) || (mate && shown);
            assert_eq!(player.hand.count as usize, hand.len());
            assert_eq!(
                player.hand.cards,
                sees.then(|| hand.clone()),
                "{viewer:?} of {owner}"
            );
            assert_eq!(
                text.contains(&format!("\"Hand {owner}\"")),
                sees,
                "{viewer:?} of {owner}"
            );
            assert!(
                hand.iter()
                    .all(|id| view.objects.iter().any(|o| o.id == *id) == sees)
            );
            assert!(player.library.cards.is_none());
            assert!(
                !text.contains(&format!("Library {owner}")),
                "{viewer:?} reads a library"
            );
        }
    }
}

#[rstest]
#[case::two_v_two(TeamMode::Teams, &[0, 1, 0, 1])]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, &[0, 0, 1, 1])]
#[case::delves_party(TeamMode::SharedTurns, &[0, 0, 0, 1])]
fn the_batches_of_a_team_table_fold_to_each_viewers_snapshot(
    #[case] mode: TeamMode,
    #[case] teams: &[u32],
    #[values(None, Some(false))] team_hands: Option<bool>,
) {
    let scenario = || table(mode, teams, team_hands);
    let before = mtg_engine::GameState::from_setup(&scenario().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario()).unwrap();
    // A few passes run turns on, with draws into hands every seat holds a view of.
    for _ in 0..40 {
        if driver.engine().decision().is_none() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let viewers: Vec<_> = (0..teams.len())
        .map(seat)
        .chain([spectator_viewer(&before)])
        .collect();
    let batches = project_events(&before, driver.engine().events(), &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let mut folded = StateView::for_player(&before, *viewer);
        folded.apply(batch);
        assert_eq!(
            folded,
            StateView::for_player(driver.state(), *viewer),
            "{viewer:?}"
        );
    }
}
