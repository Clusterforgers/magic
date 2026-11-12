//! Bodies of the tournament endpoints of `mtg-server` (`/api/tournaments`): creating one, signing up
//! by its invite, the organiser's controls, and the view of entrants, rounds and standings.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Longest tournament or team name, in characters.
pub const MAX_TOURNAMENT_NAME_CHARS: usize = 60;
/// Most Swiss rounds an organiser may plan.
pub const MAX_SWISS_ROUNDS: u32 = 15;
/// Most entrants in one tournament.
pub const MAX_ENTRANTS: usize = 64;

/// What each match of a tournament is played as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TournamentFormat {
    /// Commander, one player against one.
    #[serde(rename = "commander_1v1")]
    Commander1v1,
    /// Commander, a team of two against a team of two.
    #[serde(rename = "commander_2v2")]
    Commander2v2,
}

impl TournamentFormat {
    /// Players in one entrant: a player alone or a team.
    pub const fn team_size(self) -> usize {
        match self {
            TournamentFormat::Commander1v1 => 1,
            TournamentFormat::Commander2v2 => 2,
        }
    }

    /// The text on the wire and in the database.
    pub const fn as_str(self) -> &'static str {
        match self {
            TournamentFormat::Commander1v1 => "commander_1v1",
            TournamentFormat::Commander2v2 => "commander_2v2",
        }
    }

    /// The format a stored text names.
    pub fn parse(text: &str) -> Option<TournamentFormat> {
        [
            TournamentFormat::Commander1v1,
            TournamentFormat::Commander2v2,
        ]
        .into_iter()
        .find(|format| format.as_str() == text)
    }
}

/// How the rounds are paired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TournamentStructure {
    /// A fixed number of rounds; each round pairs entrants with like records who have not met.
    Swiss,
    /// Winners go on, losers are out, until one entrant is left.
    Elimination,
}

impl TournamentStructure {
    /// The text on the wire and in the database.
    pub const fn as_str(self) -> &'static str {
        match self {
            TournamentStructure::Swiss => "swiss",
            TournamentStructure::Elimination => "elimination",
        }
    }

    /// The structure a stored text names.
    pub fn parse(text: &str) -> Option<TournamentStructure> {
        [TournamentStructure::Swiss, TournamentStructure::Elimination]
            .into_iter()
            .find(|structure| structure.as_str() == text)
    }
}

/// Where a tournament is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TournamentStatus {
    /// Players sign up; nothing is paired yet.
    Signup,
    /// Rounds are being played.
    Running,
    /// The last round is over; the standings are final.
    Finished,
}

impl TournamentStatus {
    /// The text on the wire and in the database.
    pub const fn as_str(self) -> &'static str {
        match self {
            TournamentStatus::Signup => "signup",
            TournamentStatus::Running => "running",
            TournamentStatus::Finished => "finished",
        }
    }

    /// The status a stored text names.
    pub fn parse(text: &str) -> Option<TournamentStatus> {
        [
            TournamentStatus::Signup,
            TournamentStatus::Running,
            TournamentStatus::Finished,
        ]
        .into_iter()
        .find(|status| status.as_str() == text)
    }
}

/// How one match ended, from the side of its first entrant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MatchOutcome {
    /// The first entrant won.
    A,
    /// The second entrant won.
    B,
    /// Nobody won.
    Draw,
}

impl MatchOutcome {
    /// The text on the wire and in the database.
    pub const fn as_str(self) -> &'static str {
        match self {
            MatchOutcome::A => "a",
            MatchOutcome::B => "b",
            MatchOutcome::Draw => "draw",
        }
    }

    /// The outcome a stored text names.
    pub fn parse(text: &str) -> Option<MatchOutcome> {
        [MatchOutcome::A, MatchOutcome::B, MatchOutcome::Draw]
            .into_iter()
            .find(|outcome| outcome.as_str() == text)
    }
}

/// The body of `POST /api/tournaments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateTournament {
    /// What the tournament is called: 1 to 60 characters.
    pub name: String,
    /// What each match is played as.
    pub format: TournamentFormat,
    /// How rounds are paired.
    pub structure: TournamentStructure,
    /// Swiss rounds to play, 1 to 15; left out, the start picks enough rounds for one undefeated
    /// entrant. Ignored for elimination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rounds: Option<u32>,
}

/// The answer of `POST /api/tournaments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TournamentCreated {
    /// The new tournament's id.
    pub id: String,
    /// The invite code that lets a signed-in player sign up.
    pub invite: String,
}

/// The body of `POST /api/tournaments/invite/{code}/join`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct JoinTournament {
    /// In a team format, the entrant whose open place the player takes; left out, the player starts a
    /// new team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub team: Option<u32>,
    /// The name of a new team (1 to 60 characters); left out, the team is named after its first player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub team_name: Option<String>,
}

/// The body of `PUT /api/tournaments/{id}/matches/{round}/{table}`: the organiser's correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetMatchOutcome {
    /// The result to keep; it wins over the game's own.
    pub outcome: MatchOutcome,
}

/// One tournament in the signed-in player's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TournamentSummary {
    /// The tournament's id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What each match is played as.
    pub format: TournamentFormat,
    /// How rounds are paired.
    pub structure: TournamentStructure,
    /// Where it is in its life.
    pub status: TournamentStatus,
    /// The round being played; 0 before the start.
    pub round: u32,
    /// Entrants signed up, dropped ones included.
    pub entrants: u32,
    /// Whether the signed-in player organises it.
    pub organiser: bool,
}

/// The answer of `GET /api/tournaments`: the tournaments the player organises or plays in, newest
/// first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TournamentList {
    /// The tournaments.
    pub tournaments: Vec<TournamentSummary>,
}

/// One entrant: a player, or a team in a team format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct EntrantView {
    /// The entrant's number, in sign-up order from 0.
    pub entrant: u32,
    /// What the entrant is called: the player's name, or the team's.
    pub name: String,
    /// The usernames of its players, in sign-up order.
    pub players: Vec<String>,
    /// Whether the entrant left the tournament; a dropped entrant is not paired again.
    pub dropped: bool,
    /// Whether the signed-in player is one of its players.
    pub mine: bool,
}

/// One match of a round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatchView {
    /// The table number, from 1.
    pub table: u32,
    /// The first entrant.
    pub a: u32,
    /// The second entrant; none for a bye, which counts as a win for `a`.
    pub b: Option<u32>,
    /// The id of the match's game; none for a bye and while the game could not be made.
    pub game: Option<String>,
    /// The result; none while the game is being played.
    pub outcome: Option<MatchOutcome>,
    /// Whether the organiser set the result rather than the game.
    pub corrected: bool,
}

/// One round and its matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RoundView {
    /// The round number, from 1.
    pub round: u32,
    /// Its matches by table.
    pub matches: Vec<MatchView>,
}

/// One line of the standings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct StandingView {
    /// The place, from 1.
    pub rank: u32,
    /// The entrant.
    pub entrant: u32,
    /// Match points: 3 for a win or a bye, 1 for a draw.
    pub points: u32,
    /// Matches won, byes included.
    pub wins: u32,
    /// Matches lost.
    pub losses: u32,
    /// Matches drawn.
    pub draws: u32,
    /// The average match-win fraction of the entrant's opponents, each at least 1/3.
    pub opponents_match_win: f64,
    /// The entrant's game-win fraction, at least 1/3.
    pub game_win: f64,
    /// The average game-win fraction of the entrant's opponents, each at least 1/3.
    pub opponents_game_win: f64,
}

/// The answer of `GET /api/tournaments/{id}` and of the organiser's controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TournamentView {
    /// The tournament's id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What each match is played as.
    pub format: TournamentFormat,
    /// How rounds are paired.
    pub structure: TournamentStructure,
    /// Where it is in its life.
    pub status: TournamentStatus,
    /// The round being played; 0 before the start.
    pub round: u32,
    /// Rounds planned: the Swiss count, or what the bracket needs; none for Swiss until the start
    /// when the organiser left it to the server.
    pub rounds: Option<u32>,
    /// The organiser's username.
    pub organiser_name: String,
    /// Whether the signed-in player organises it.
    pub organiser: bool,
    /// The invite code; only the organiser is told.
    pub invite: Option<String>,
    /// Whether matches in this format can be made into games on this server.
    pub playable: bool,
    /// Every entrant in sign-up order.
    pub entrants: Vec<EntrantView>,
    /// Every round so far, the first first.
    pub rounds_played: Vec<RoundView>,
    /// The standings, the leader first; empty before the start.
    pub standings: Vec<StandingView>,
}

/// What an invite shows before signing up: `GET /api/tournaments/invite/{code}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TournamentInvite {
    /// The tournament's id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What each match is played as.
    pub format: TournamentFormat,
    /// How rounds are paired.
    pub structure: TournamentStructure,
    /// Where it is in its life; sign-up is open only in `signup`.
    pub status: TournamentStatus,
    /// The organiser's username.
    pub organiser_name: String,
    /// Entrants so far, so a player can pick a team with an open place.
    pub entrants: Vec<EntrantView>,
    /// Whether the signed-in player has signed up already.
    pub joined: bool,
}

/// The `error` code of a failed tournament request; the body is `{error, message}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TournamentErrorCode {
    /// 400: the body, a name or a number is not what the endpoint takes.
    InvalidRequest,
    /// 401: no valid session cookie.
    Unauthenticated,
    /// 403: only the organiser may do this.
    Forbidden,
    /// 404: no such tournament, invite, entrant or match, or one the player may not see.
    NotFound,
    /// 409: sign-up is closed because the tournament has started.
    Started,
    /// 409: the tournament has not started, or is over, so rounds and results do not change.
    NotRunning,
    /// 409: the player has signed up already.
    AlreadyJoined,
    /// 409: the tournament or the chosen team has no open place.
    Full,
    /// 409: too few complete entrants to start, or a team is missing a player.
    NotEnoughEntrants,
    /// 409: a match of the round has no result, or an elimination match is drawn.
    Undecided,
    /// 409: this server cannot make games in the tournament's format yet.
    FormatUnavailable,
    /// 503: the server runs without a database, so it keeps no tournaments.
    AccountsUnavailable,
    /// 503: the database failed; try again later.
    Unavailable,
}

impl TournamentErrorCode {
    /// The code as it appears in the body.
    pub const fn as_str(self) -> &'static str {
        match self {
            TournamentErrorCode::InvalidRequest => "invalid_request",
            TournamentErrorCode::Unauthenticated => "unauthenticated",
            TournamentErrorCode::Forbidden => "forbidden",
            TournamentErrorCode::NotFound => "not_found",
            TournamentErrorCode::Started => "started",
            TournamentErrorCode::NotRunning => "not_running",
            TournamentErrorCode::AlreadyJoined => "already_joined",
            TournamentErrorCode::Full => "full",
            TournamentErrorCode::NotEnoughEntrants => "not_enough_entrants",
            TournamentErrorCode::Undecided => "undecided",
            TournamentErrorCode::FormatUnavailable => "format_unavailable",
            TournamentErrorCode::AccountsUnavailable => "accounts_unavailable",
            TournamentErrorCode::Unavailable => "unavailable",
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every stored text of the tournament enums reads back as the value that wrote it.

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(TournamentFormat::Commander1v1, 1)]
    #[case(TournamentFormat::Commander2v2, 2)]
    fn formats_round_trip(#[case] format: TournamentFormat, #[case] size: usize) {
        assert_eq!(TournamentFormat::parse(format.as_str()), Some(format));
        assert_eq!(format.team_size(), size);
        let json = serde_json::to_string(&format).unwrap_or_default();
        assert_eq!(json, format!("\"{}\"", format.as_str()));
    }

    #[rstest]
    #[case("swiss")]
    #[case("elimination")]
    #[case("signup")]
    #[case("running")]
    #[case("finished")]
    #[case("a")]
    #[case("b")]
    #[case("draw")]
    fn stored_texts_round_trip(#[case] text: &str) {
        let back = TournamentStructure::parse(text)
            .map(TournamentStructure::as_str)
            .or(TournamentStatus::parse(text).map(TournamentStatus::as_str))
            .or(MatchOutcome::parse(text).map(MatchOutcome::as_str));
        assert_eq!(back, Some(text));
    }

    #[rstest]
    #[case("")]
    #[case("Swiss")]
    #[case("commander")]
    fn unknown_texts_are_none(#[case] text: &str) {
        assert_eq!(TournamentFormat::parse(text), None);
        assert_eq!(TournamentStructure::parse(text), None);
        assert_eq!(TournamentStatus::parse(text), None);
        assert_eq!(MatchOutcome::parse(text), None);
    }

    #[test]
    fn error_codes_serialize_as_their_text() {
        for code in [
            TournamentErrorCode::NotEnoughEntrants,
            TournamentErrorCode::FormatUnavailable,
            TournamentErrorCode::AlreadyJoined,
        ] {
            let json = serde_json::to_string(&code).unwrap_or_default();
            assert_eq!(json, format!("\"{}\"", code.as_str()));
        }
    }
}
