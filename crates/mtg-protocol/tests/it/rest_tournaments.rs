//! Goal: the tournament bodies keep their documented wire shape: snake_case formats and outcomes, optional fields left out, unknown fields refused in requests, a view that survives a JSON round trip, and bindings for every body.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_protocol::rest::{
    CreateTournament, EntrantView, JoinTournament, MatchOutcome, MatchView, RoundView,
    SetMatchOutcome, StandingView, TournamentFormat, TournamentStatus, TournamentStructure,
    TournamentView,
};
use serde_json::json;

#[test]
fn a_create_body_leaves_out_rounds_and_refuses_unknown_fields() {
    let body = CreateTournament {
        name: "Friday".into(),
        format: TournamentFormat::Commander2v2,
        structure: TournamentStructure::Elimination,
        rounds: None,
    };
    assert_eq!(
        serde_json::to_value(&body).unwrap(),
        json!({"name": "Friday", "format": "commander_2v2", "structure": "elimination"})
    );
    let extra = json!({"name": "x", "format": "commander_1v1", "structure": "swiss", "seed": 1});
    assert!(serde_json::from_value::<CreateTournament>(extra).is_err());
    assert_eq!(
        serde_json::from_value::<JoinTournament>(json!({})).unwrap(),
        JoinTournament::default()
    );
    assert!(serde_json::from_value::<SetMatchOutcome>(json!({"outcome": "tie"})).is_err());
}

#[test]
fn a_view_survives_a_json_round_trip() {
    let view = TournamentView {
        id: "t".into(),
        name: "Cup".into(),
        format: TournamentFormat::Commander1v1,
        structure: TournamentStructure::Swiss,
        status: TournamentStatus::Running,
        round: 1,
        rounds: Some(3),
        organiser_name: "Org".into(),
        organiser: false,
        invite: None,
        playable: true,
        entrants: vec![EntrantView {
            entrant: 0,
            name: "Ann".into(),
            players: vec!["Ann".into()],
            dropped: false,
            mine: true,
        }],
        rounds_played: vec![RoundView {
            round: 1,
            matches: vec![MatchView {
                table: 1,
                a: 0,
                b: None,
                game: None,
                outcome: Some(MatchOutcome::Draw),
                corrected: true,
            }],
        }],
        standings: vec![StandingView {
            rank: 1,
            entrant: 0,
            points: 3,
            wins: 1,
            losses: 0,
            draws: 0,
            opponents_match_win: 0.0,
            game_win: 1.0,
            opponents_game_win: 0.0,
        }],
    };
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["rounds_played"][0]["matches"][0]["outcome"], "draw");
    assert_eq!(json["status"], "running");
    assert_eq!(
        serde_json::from_value::<TournamentView>(json).unwrap(),
        view
    );
}

#[test]
fn every_body_has_bindings() {
    let files = generated();
    for needed in [
        "CreateTournament.ts",
        "TournamentCreated.ts",
        "JoinTournament.ts",
        "SetMatchOutcome.ts",
        "TournamentList.ts",
        "TournamentView.ts",
        "TournamentInvite.ts",
        "TournamentErrorCode.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
}
