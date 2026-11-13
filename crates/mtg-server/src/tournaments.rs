//! Tournaments: an organiser's rounds of Commander games between signed-in players who signed up by
//! invite. Each match is a normal game the server makes with its seats bound to the players; its
//! result is read from the game record, and the organiser may set it instead.

mod pairing;
mod store;

use std::collections::HashSet;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::state::GameResult;
use mtg_protocol::ClientMessage;
use mtg_protocol::rest::{
    CreateTournament, EntrantView, JoinTournament, MAX_ENTRANTS, MAX_SWISS_ROUNDS,
    MAX_TOURNAMENT_NAME_CHARS, MatchOutcome, MatchView, RoundView, SetMatchOutcome, StandingView,
    TournamentCreated, TournamentErrorCode, TournamentFormat, TournamentInvite, TournamentList,
    TournamentStatus, TournamentStructure, TournamentSummary, TournamentView,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::{ApiError, read_json};
use crate::game_id::GameId;
use crate::join_code::JoinCode;
use crate::recipe::FormatKind;
use crate::records::{RecordError, RecordStatus, Visibility};
use crate::registry::GameOptions;
use crate::secret::random_seed;
use crate::server::AppState;
use crate::user_id::UserId;

use pairing::{Pairing, Played, Shuffler};
pub(crate) use store::TournamentStore;
use store::{Entrant, MatchRow, NewTournament, Signup, Tournament};

/// Whether this server makes team games (Team Commander) for 2v2 matches. Without them, a 2v2
/// tournament signs teams up but cannot start.
pub(crate) const TEAM_GAMES: bool = true;
/// Most tournaments a player's list shows.
const MAX_LISTED: i64 = 100;

/// The tournament service: the store and the lock that runs one organiser action at a time, so a
/// round is paired and its games are made once.
#[derive(Debug, Clone)]
pub(crate) struct Tournaments {
    store: TournamentStore,
    turn: Arc<tokio::sync::Mutex<()>>,
}

impl Tournaments {
    /// The service over `pool`, whose server migrations have run.
    pub(crate) fn new(pool: PgPool) -> Tournaments {
        Tournaments {
            store: TournamentStore::new(pool),
            turn: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Whether `game` is the game of a tournament match, which has no player host: the organiser runs it.
    pub(crate) async fn is_match_game(&self, game: GameId) -> Result<bool, RecordError> {
        self.store.is_match_game(game).await
    }
}

fn failure(status: StatusCode, code: TournamentErrorCode, message: &str) -> ApiError {
    ApiError::new(status, code.as_str(), message)
}

fn not_found() -> ApiError {
    failure(
        StatusCode::NOT_FOUND,
        TournamentErrorCode::NotFound,
        "no such tournament",
    )
}

fn conflict(code: TournamentErrorCode, message: &str) -> ApiError {
    failure(StatusCode::CONFLICT, code, message)
}

fn invalid(message: &str) -> ApiError {
    failure(
        StatusCode::BAD_REQUEST,
        TournamentErrorCode::InvalidRequest,
        message,
    )
}

fn unavailable(error: &RecordError) -> ApiError {
    tracing::error!(%error, "tournament records failed");
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        TournamentErrorCode::Unavailable,
        "tournaments are not available right now; try again later",
    )
}

/// The service, or 503 on a server without a database.
fn service(app: &AppState) -> Result<&Tournaments, ApiError> {
    app.tournaments().ok_or_else(|| {
        failure(
            StatusCode::SERVICE_UNAVAILABLE,
            TournamentErrorCode::AccountsUnavailable,
            "this server runs without a database, so it keeps no tournaments",
        )
    })
}

/// A tournament id from a path; a malformed one is a 404, like an unknown tournament.
fn id_of(path: Result<Path<String>, PathRejection>) -> Result<Uuid, ApiError> {
    let Path(id) = path.map_err(|_| not_found())?;
    Uuid::parse_str(&id).map_err(|_| not_found())
}

/// A trimmed name of 1 to 60 characters without control characters.
fn checked_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > MAX_TOURNAMENT_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(invalid(
            "a name is 1 to 60 characters without control characters",
        ));
    }
    Ok(name.to_owned())
}

/// Whether `user` plays in `tournament`.
fn plays(tournament: &Tournament, user: UserId) -> bool {
    tournament
        .entrants
        .iter()
        .any(|entrant| entrant.players.iter().any(|player| player.user == user))
}

async fn load(tournaments: &Tournaments, id: Uuid) -> Result<Tournament, ApiError> {
    tournaments
        .store
        .load(id)
        .await
        .map_err(|e| unavailable(&e))?
        .ok_or_else(not_found)
}

/// The tournament, for its organiser alone: 404 for a stranger, 403 for a player.
async fn as_organiser(
    tournaments: &Tournaments,
    id: Uuid,
    user: UserId,
) -> Result<Tournament, ApiError> {
    let tournament = load(tournaments, id).await?;
    if tournament.organiser == user {
        Ok(tournament)
    } else if plays(&tournament, user) {
        Err(failure(
            StatusCode::FORBIDDEN,
            TournamentErrorCode::Forbidden,
            "only the organiser may do this",
        ))
    } else {
        Err(not_found())
    }
}

/// The match's outcome from a game's result: seats alternate between the two entrants, the first
/// entrant on the even seats and team 0.
fn outcome_of(result: GameResult) -> MatchOutcome {
    match result {
        GameResult::Winner(seat) if seat.index() % 2 == 0 => MatchOutcome::A,
        GameResult::Team(0) => MatchOutcome::A,
        GameResult::Winner(_) | GameResult::Team(_) => MatchOutcome::B,
        GameResult::Draw => MatchOutcome::Draw,
    }
}

/// The players of a match by seat, alternating between the entrants: A1, B1, A2, B2.
fn seated(tournament: &Tournament, played: &MatchRow) -> Option<Vec<(String, UserId)>> {
    let entrant = |n: u32| tournament.entrants.iter().find(|e| e.number == n);
    let a = entrant(played.a)?;
    let b = entrant(played.b?)?;
    let size = tournament.format.team_size();
    if a.players.len() < size || b.players.len() < size {
        return None;
    }
    Some(
        (0..size)
            .flat_map(|i| [&a.players[i], &b.players[i]])
            .map(|player| (player.name.clone(), player.user))
            .collect(),
    )
}

/// Fills the results of matches whose games have ended from the game records, and forgets a game that was dropped
/// without a result (expired, closed, failed or gone), so the match gets a new one. True when it forgot one.
async fn sync_results(app: &AppState, tournament: &mut Tournament) -> Result<bool, ApiError> {
    let tournaments = service(app)?;
    let mut forgot = false;
    for played in &mut tournament.matches {
        let (Some(game), None) = (played.game, played.outcome) else {
            continue;
        };
        let record = app
            .registry()
            .record_result(game)
            .await
            .map_err(|e| unavailable(&e))?;
        let result = match record {
            Some((RecordStatus::Finished, Some(result))) => result,
            Some((RecordStatus::Lobby | RecordStatus::Running, _)) => continue,
            // Still in the registry: the record is behind the table, which may yet end.
            _ if app.registry().handle(game).is_some() => continue,
            _ => {
                tournaments
                    .store
                    .clear_game(tournament.id, played.round, played.table, game)
                    .await
                    .map_err(|e| unavailable(&e))?;
                played.game = None;
                forgot = true;
                continue;
            }
        };
        let outcome = outcome_of(result);
        tournaments
            .store
            .set_outcome(tournament.id, played.round, played.table, outcome, false)
            .await
            .map_err(|e| unavailable(&e))?;
        played.outcome = Some(outcome);
    }
    Ok(forgot)
}

/// The rules of a match game: a team format seats the entrants' players alternately as two teams of separate
/// turns (CR 808), the first entrant team 0, which [`outcome_of`] reads back.
fn match_rules(format: TournamentFormat) -> HouseRules {
    let size = format.team_size();
    HouseRules {
        teams: (size > 1)
            .then(|| Teams::even(TeamMode::Teams, 2, u32::try_from(size).unwrap_or(u32::MAX))),
        take_backs: Some(false),
        ..HouseRules::default()
    }
}

/// Makes the game of every match of the current round that has none; a game that cannot be made
/// is logged and left for the organiser's retry.
async fn make_games(app: &AppState, tournament: &mut Tournament) -> Result<(), ApiError> {
    let tournaments = service(app)?;
    if tournament.format.team_size() > 1 && !TEAM_GAMES {
        return Ok(());
    }
    let round = tournament.round;
    for at in 0..tournament.matches.len() {
        let played = &tournament.matches[at];
        if played.round != round || played.game.is_some() || played.outcome.is_some() {
            continue;
        }
        let Some(seats) = seated(tournament, played) else {
            continue;
        };
        let (names, users): (Vec<String>, Vec<UserId>) = seats.into_iter().unzip();
        let options = GameOptions {
            format: FormatKind::Commander,
            visibility: Visibility::Invite,
            accounts: users.into_iter().map(Some).collect(),
            house_rules: match_rules(tournament.format),
            ..GameOptions::default()
        };
        let decks = vec![None; names.len()];
        let table = played.table;
        match app
            .registry()
            .create_decked(names, decks, options, None)
            .await
        {
            Ok(created) => {
                tournaments
                    .store
                    .set_game(tournament.id, round, table, created.id)
                    .await
                    .map_err(|e| unavailable(&e))?;
                tournament.matches[at].game = Some(created.id);
            }
            Err(error) => {
                tracing::warn!(%error, tournament = %tournament.id, round, table, "a match game could not be made");
            }
        }
    }
    Ok(())
}

/// The rounds of a tournament as the pairing functions take them.
fn rounds_of(tournament: &Tournament) -> Vec<Vec<Played>> {
    let mut rounds: Vec<Vec<Played>> = vec![Vec::new(); tournament.round as usize];
    for m in &tournament.matches {
        if let Some(round) = rounds.get_mut((m.round as usize).wrapping_sub(1)) {
            round.push(Played {
                a: m.a,
                b: m.b,
                outcome: m.outcome,
            });
        }
    }
    rounds
}

/// The tournament as `user` sees it.
fn view(tournament: &Tournament, user: UserId) -> TournamentView {
    let organiser = tournament.organiser == user;
    let entrants = tournament
        .entrants
        .iter()
        .map(|entrant| entrant_view(entrant, user))
        .collect();
    let rounds_played = (1..=tournament.round)
        .map(|round| RoundView {
            round,
            matches: tournament
                .matches
                .iter()
                .filter(|m| m.round == round)
                .map(|m| MatchView {
                    table: m.table,
                    a: m.a,
                    b: m.b,
                    game: m.game.map(|game| game.to_string()),
                    outcome: m.outcome,
                    corrected: m.corrected,
                })
                .collect(),
        })
        .collect();
    let standings = if tournament.status == TournamentStatus::Signup {
        Vec::new()
    } else {
        let ids: Vec<u32> = tournament.entrants.iter().map(|e| e.number).collect();
        pairing::standings(&ids, &rounds_of(tournament))
            .into_iter()
            .enumerate()
            .map(|(at, s)| StandingView {
                rank: u32::try_from(at + 1).unwrap_or(u32::MAX),
                entrant: s.entrant,
                points: s.points,
                wins: s.wins,
                losses: s.losses,
                draws: s.draws,
                opponents_match_win: rounded(s.opponents_match_win),
                game_win: rounded(s.game_win),
                opponents_game_win: rounded(s.opponents_game_win),
            })
            .collect()
    };
    TournamentView {
        id: tournament.id.to_string(),
        name: tournament.name.clone(),
        format: tournament.format,
        structure: tournament.structure,
        status: tournament.status,
        round: tournament.round,
        rounds: tournament.rounds.or_else(|| {
            (tournament.structure == TournamentStructure::Elimination)
                .then(|| pairing::elimination_rounds(tournament.entrants.len()))
        }),
        organiser_name: tournament.organiser_name.clone(),
        organiser,
        invite: organiser.then(|| tournament.invite.clone()),
        playable: tournament.format.team_size() == 1 || TEAM_GAMES,
        entrants,
        rounds_played,
        standings,
    }
}

/// A fraction to four places, so the wire carries no float noise.
fn rounded(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn entrant_view(entrant: &Entrant, user: UserId) -> EntrantView {
    EntrantView {
        entrant: entrant.number,
        name: entrant.name.clone(),
        players: entrant.players.iter().map(|p| p.name.clone()).collect(),
        dropped: entrant.dropped,
        mine: entrant.players.iter().any(|p| p.user == user),
    }
}

// --- the routes ------------------------------------------------------------------------------

/// `GET /api/tournaments`: the tournaments the player organises or plays in, newest first.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TournamentList>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let listed = tournaments
        .store
        .list_for(user.id, MAX_LISTED)
        .await
        .map_err(|e| unavailable(&e))?;
    Ok(Json(TournamentList {
        tournaments: listed
            .into_iter()
            .map(|t| TournamentSummary {
                id: t.id.to_string(),
                name: t.name,
                format: t.format,
                structure: t.structure,
                status: t.status,
                round: t.round,
                entrants: t.entrants,
                organiser: t.organiser,
            })
            .collect(),
    }))
}

/// `POST /api/tournaments`: a new tournament in sign-up, organised by the signed-in player; 201.
pub(crate) async fn create(
    State(app): State<AppState>,
    request: Request,
) -> Result<(StatusCode, Json<TournamentCreated>), ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(request.headers()).await?;
    let body: CreateTournament = read_json(request).await?;
    let name = checked_name(&body.name)?;
    let rounds = match (body.structure, body.rounds) {
        (TournamentStructure::Swiss, Some(rounds)) if !(1..=MAX_SWISS_ROUNDS).contains(&rounds) => {
            return Err(invalid("a Swiss tournament plays 1 to 15 rounds"));
        }
        (TournamentStructure::Swiss, rounds) => rounds,
        (TournamentStructure::Elimination, _) => None,
    };
    let seed = random_seed().map_err(|_| ApiError::internal())?;
    let invite = JoinCode::generate().map_err(|_| ApiError::internal())?;
    let new = NewTournament {
        id: Uuid::new_v4(),
        name,
        organiser: user.id,
        format: body.format,
        structure: body.structure,
        rounds,
        seed,
        invite: invite.expose().to_owned(),
    };
    tournaments
        .store
        .create(&new)
        .await
        .map_err(|e| unavailable(&e))?;
    Ok((
        StatusCode::CREATED,
        Json(TournamentCreated {
            id: new.id.to_string(),
            invite: new.invite,
        }),
    ))
}

/// The tournament an invite opens; an unknown or malformed code is one 404.
async fn by_invite(
    tournaments: &Tournaments,
    path: Result<Path<String>, PathRejection>,
) -> Result<Tournament, ApiError> {
    let Path(code) = path.map_err(|_| not_found())?;
    let code = JoinCode::parse(&code).ok_or_else(not_found)?;
    let id = tournaments
        .store
        .by_invite(code.expose())
        .await
        .map_err(|e| unavailable(&e))?
        .ok_or_else(not_found)?;
    load(tournaments, id).await
}

/// `GET /api/tournaments/invite/{code}`: what a player signs up for.
pub(crate) async fn invite(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<TournamentInvite>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let tournament = by_invite(tournaments, path).await?;
    Ok(Json(TournamentInvite {
        id: tournament.id.to_string(),
        name: tournament.name.clone(),
        format: tournament.format,
        structure: tournament.structure,
        status: tournament.status,
        organiser_name: tournament.organiser_name.clone(),
        entrants: tournament
            .entrants
            .iter()
            .map(|e| entrant_view(e, user.id))
            .collect(),
        joined: plays(&tournament, user.id),
    }))
}

/// `POST /api/tournaments/invite/{code}/join`: signs the player up, alone or into a team; answers
/// the tournament.
pub(crate) async fn join(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(request.headers()).await?;
    let body: JoinTournament = read_json(request).await?;
    let tournament = by_invite(tournaments, path).await?;
    let size = tournament.format.team_size();
    let team = if size > 1 { body.team } else { None };
    let name = match (&body.team_name, size) {
        (Some(name), 2..) => checked_name(name)?,
        _ => user.username.clone(),
    };
    let signup = tournaments
        .store
        .join(tournament.id, user.id, team, &name, size, MAX_ENTRANTS)
        .await
        .map_err(|e| unavailable(&e))?;
    match signup {
        Signup::Joined(_) => {}
        Signup::Started => {
            return Err(conflict(
                TournamentErrorCode::Started,
                "the tournament has started",
            ));
        }
        Signup::Already => {
            return Err(conflict(
                TournamentErrorCode::AlreadyJoined,
                "you have signed up already",
            ));
        }
        Signup::Full => {
            return Err(conflict(
                TournamentErrorCode::Full,
                "there is no open place",
            ));
        }
        Signup::NoTeam => return Err(not_found()),
    }
    let tournament = load(tournaments, tournament.id).await?;
    Ok(Json(view(&tournament, user.id)))
}

/// `GET /api/tournaments/{id}`: the tournament with its rounds and standings, for its organiser and
/// players; anybody else gets 404. Results of ended games are filled in first, and a game that ended without one
/// (expired) is made again.
pub(crate) async fn get_one(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let _turn = tournaments.turn.lock().await;
    let mut tournament = load(tournaments, id).await?;
    if tournament.organiser != user.id && !plays(&tournament, user.id) {
        return Err(not_found());
    }
    if sync_results(&app, &mut tournament).await? && tournament.status == TournamentStatus::Running
    {
        make_games(&app, &mut tournament).await?;
    }
    Ok(Json(view(&tournament, user.id)))
}

/// `DELETE /api/tournaments/{id}`: the organiser removes the tournament; its games go on as normal
/// games. 204.
pub(crate) async fn remove(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let _turn = tournaments.turn.lock().await;
    as_organiser(tournaments, id, user.id).await?;
    tournaments
        .store
        .delete(id)
        .await
        .map_err(|e| unavailable(&e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/tournaments/{id}/leave`: the player leaves before the start; 204.
pub(crate) async fn leave(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let tournament = load(tournaments, id).await?;
    if !plays(&tournament, user.id) {
        return Err(not_found());
    }
    if tournament.status != TournamentStatus::Signup {
        return Err(conflict(
            TournamentErrorCode::Started,
            "the tournament has started; drop out instead",
        ));
    }
    tournaments
        .store
        .leave(id, user.id)
        .await
        .map_err(|e| unavailable(&e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Concedes the seats of `players` in the running `game` through the normal concession, so the game ends as their
/// loss. False when the game does not run (a lobby, or gone), which leaves the result to the organiser's ruling.
async fn concede_game(app: &AppState, game: GameId, players: &[UserId]) -> Result<bool, ApiError> {
    let running = matches!(
        app.registry()
            .record_result(game)
            .await
            .map_err(|e| unavailable(&e))?,
        Some((RecordStatus::Running, _))
    );
    if !running {
        return Ok(false);
    }
    for &user in players {
        if let Some(access) = app.registry().authenticate_user(game, user) {
            // A game that closed meanwhile has ended on its own; the drop still decides the match.
            let _ = access
                .handle
                .send(access.seat, ClientMessage::Concede)
                .await;
        }
    }
    Ok(true)
}

/// Drops `entrant` from a running tournament: it is not paired again, and an undecided match of
/// the current round goes to its opponent; a running game of that match ends as the entrant's concession.
async fn drop_entrant(
    app: &AppState,
    tournament: &mut Tournament,
    entrant: u32,
) -> Result<(), ApiError> {
    let tournaments = service(app)?;
    if tournament.status != TournamentStatus::Running {
        return Err(conflict(
            TournamentErrorCode::NotRunning,
            "only a running tournament has entrants to drop",
        ));
    }
    sync_results(app, tournament).await?;
    let dropped = tournaments
        .store
        .drop_entrant(tournament.id, entrant)
        .await
        .map_err(|e| unavailable(&e))?;
    if !dropped {
        return Err(not_found());
    }
    let players: Vec<UserId> = tournament
        .entrants
        .iter()
        .filter(|e| e.number == entrant)
        .flat_map(|e| e.players.iter().map(|p| p.user))
        .collect();
    let round = tournament.round;
    for played in &mut tournament.matches {
        let Some(b) = played.b else { continue };
        if played.round != round
            || played.outcome.is_some()
            || (played.a != entrant && b != entrant)
        {
            continue;
        }
        let outcome = if played.a == entrant {
            MatchOutcome::B
        } else {
            MatchOutcome::A
        };
        let conceded = match played.game {
            Some(game) => concede_game(app, game, &players).await?,
            None => false,
        };
        tournaments
            .store
            .set_outcome(tournament.id, round, played.table, outcome, !conceded)
            .await
            .map_err(|e| unavailable(&e))?;
        played.outcome = Some(outcome);
        played.corrected = !conceded;
    }
    if let Some(e) = tournament.entrants.iter_mut().find(|e| e.number == entrant) {
        e.dropped = true;
    }
    Ok(())
}

/// `POST /api/tournaments/{id}/drop`: the player's entrant drops out of a running tournament.
pub(crate) async fn drop_self(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let _turn = tournaments.turn.lock().await;
    let mut tournament = load(tournaments, id).await?;
    let Some(entrant) = tournament
        .entrants
        .iter()
        .find(|e| e.players.iter().any(|p| p.user == user.id))
        .map(|e| e.number)
    else {
        return Err(not_found());
    };
    drop_entrant(&app, &mut tournament, entrant).await?;
    Ok(Json(view(&tournament, user.id)))
}

/// `POST /api/tournaments/{id}/entrants/{entrant}/drop`: the organiser drops an entrant.
pub(crate) async fn drop_other(
    State(app): State<AppState>,
    path: Result<Path<(String, u32)>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let Path((id, entrant)) = path.map_err(|_| not_found())?;
    let id = Uuid::parse_str(&id).map_err(|_| not_found())?;
    let _turn = tournaments.turn.lock().await;
    let mut tournament = as_organiser(tournaments, id, user.id).await?;
    drop_entrant(&app, &mut tournament, entrant).await?;
    Ok(Json(view(&tournament, user.id)))
}

/// Stores the next round's pairings and makes its games.
async fn begin_round(
    app: &AppState,
    tournament: &mut Tournament,
    rounds: u32,
    pairings: &[Pairing],
) -> Result<(), ApiError> {
    let tournaments = service(app)?;
    let next = tournament.round + 1;
    let begun = tournaments
        .store
        .begin_round(tournament.id, next, Some(rounds), pairings)
        .await
        .map_err(|e| unavailable(&e))?;
    if !begun {
        return Err(conflict(
            TournamentErrorCode::NotRunning,
            "the round has changed meanwhile; reload",
        ));
    }
    *tournament = load(tournaments, tournament.id).await?;
    make_games(app, tournament).await
}

/// `POST /api/tournaments/{id}/start`: closes sign-up and pairs round 1.
pub(crate) async fn start(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let _turn = tournaments.turn.lock().await;
    let mut tournament = as_organiser(tournaments, id, user.id).await?;
    if tournament.status != TournamentStatus::Signup {
        return Err(conflict(
            TournamentErrorCode::Started,
            "the tournament has started",
        ));
    }
    let size = tournament.format.team_size();
    if size > 1 && !TEAM_GAMES {
        return Err(conflict(
            TournamentErrorCode::FormatUnavailable,
            "this server cannot make team games yet",
        ));
    }
    if tournament.entrants.iter().any(|e| e.players.len() < size) {
        return Err(conflict(
            TournamentErrorCode::NotEnoughEntrants,
            "a team is missing a player",
        ));
    }
    if tournament.entrants.len() < 2 {
        return Err(conflict(
            TournamentErrorCode::NotEnoughEntrants,
            "a tournament needs two entrants or more",
        ));
    }
    let mut order: Vec<u32> = tournament.entrants.iter().map(|e| e.number).collect();
    Shuffler::new(tournament.seed).shuffle(&mut order);
    let (rounds, pairings) = match tournament.structure {
        TournamentStructure::Swiss => (
            tournament
                .rounds
                .unwrap_or_else(|| pairing::swiss_rounds(order.len())),
            pairing::swiss(&order, &[]),
        ),
        TournamentStructure::Elimination => (
            pairing::elimination_rounds(order.len()),
            pairing::elimination(&order, &[]),
        ),
    };
    begin_round(&app, &mut tournament, rounds, &pairings).await?;
    Ok(Json(view(&tournament, user.id)))
}

/// `POST /api/tournaments/{id}/next`: once every match of the round has a result, pairs the next
/// round, or ends the tournament after its last.
pub(crate) async fn next(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let _turn = tournaments.turn.lock().await;
    let mut tournament = as_organiser(tournaments, id, user.id).await?;
    if tournament.status != TournamentStatus::Running {
        return Err(conflict(
            TournamentErrorCode::NotRunning,
            "the tournament is not running",
        ));
    }
    sync_results(&app, &mut tournament).await?;
    let rounds = rounds_of(&tournament);
    let current = rounds.last().cloned().unwrap_or_default();
    if !current.iter().all(Played::decided) {
        return Err(conflict(
            TournamentErrorCode::Undecided,
            "a match of the round has no result yet",
        ));
    }
    let dropped: HashSet<u32> = tournament
        .entrants
        .iter()
        .filter(|e| e.dropped)
        .map(|e| e.number)
        .collect();
    let planned = tournament.rounds.unwrap_or(tournament.round);
    let pairings = match tournament.structure {
        TournamentStructure::Swiss => {
            let ids: Vec<u32> = tournament.entrants.iter().map(|e| e.number).collect();
            let ranked: Vec<u32> = pairing::standings(&ids, &rounds)
                .into_iter()
                .map(|s| s.entrant)
                .filter(|e| !dropped.contains(e))
                .collect();
            (tournament.round < planned && ranked.len() >= 2)
                .then(|| pairing::swiss(&ranked, &rounds))
        }
        TournamentStructure::Elimination => {
            if current
                .iter()
                .any(|m| m.outcome == Some(MatchOutcome::Draw))
            {
                return Err(conflict(
                    TournamentErrorCode::Undecided,
                    "an elimination match needs a winner; set one",
                ));
            }
            let alive = pairing::survivors(&current, &dropped);
            (alive.len() >= 2).then(|| pairing::elimination(&alive, &rounds))
        }
    };
    if let Some(pairings) = pairings {
        // A bracket that lost entrants to drops may need more rounds than planned.
        let planned = planned.max(tournament.round + 1);
        begin_round(&app, &mut tournament, planned, &pairings).await?;
    } else {
        tournaments
            .store
            .finish(id)
            .await
            .map_err(|e| unavailable(&e))?;
        tournament.status = TournamentStatus::Finished;
    }
    Ok(Json(view(&tournament, user.id)))
}

/// `POST /api/tournaments/{id}/games`: makes the games of the round that could not be made before, or were dropped
/// without a result.
pub(crate) async fn retry_games(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let _turn = tournaments.turn.lock().await;
    let mut tournament = as_organiser(tournaments, id, user.id).await?;
    if tournament.status != TournamentStatus::Running {
        return Err(conflict(
            TournamentErrorCode::NotRunning,
            "the tournament is not running",
        ));
    }
    sync_results(&app, &mut tournament).await?;
    make_games(&app, &mut tournament).await?;
    Ok(Json(view(&tournament, user.id)))
}

/// `PUT /api/tournaments/{id}/matches/{round}/{table}`: the organiser sets a match's result. Any
/// round of a running Swiss tournament, the current round of an elimination one.
pub(crate) async fn set_outcome(
    State(app): State<AppState>,
    path: Result<Path<(String, u32, u32)>, PathRejection>,
    request: Request,
) -> Result<Json<TournamentView>, ApiError> {
    let tournaments = service(&app)?;
    let user = app.auth().require_user(request.headers()).await?;
    let Path((id, round, table)) = path.map_err(|_| not_found())?;
    let id = Uuid::parse_str(&id).map_err(|_| not_found())?;
    let body: SetMatchOutcome = read_json(request).await?;
    let _turn = tournaments.turn.lock().await;
    let mut tournament = as_organiser(tournaments, id, user.id).await?;
    if tournament.status != TournamentStatus::Running {
        return Err(conflict(
            TournamentErrorCode::NotRunning,
            "results change only while the tournament runs",
        ));
    }
    if tournament.structure == TournamentStructure::Elimination && round != tournament.round {
        return Err(conflict(
            TournamentErrorCode::NotRunning,
            "an elimination round is final once the next is paired",
        ));
    }
    let set = tournaments
        .store
        .set_outcome(id, round, table, body.outcome, true)
        .await
        .map_err(|e| unavailable(&e))?;
    if !set {
        return Err(not_found());
    }
    if let Some(played) = tournament
        .matches
        .iter_mut()
        .find(|m| m.round == round && m.table == table)
    {
        played.outcome = Some(body.outcome);
        played.corrected = true;
    }
    sync_results(&app, &mut tournament).await?;
    Ok(Json(view(&tournament, user.id)))
}

#[cfg(test)]
mod tests {
    //! Goal: game results map to the right entrant and a match's seats alternate between the teams.

    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use mtg_protocol::rest::TournamentFormat;

    use super::*;
    use store::Player;

    #[rstest]
    #[case(GameResult::Winner(PlayerId::from_index(0)), MatchOutcome::A)]
    #[case(GameResult::Winner(PlayerId::from_index(1)), MatchOutcome::B)]
    #[case(GameResult::Winner(PlayerId::from_index(2)), MatchOutcome::A)]
    #[case(GameResult::Winner(PlayerId::from_index(3)), MatchOutcome::B)]
    #[case(GameResult::Draw, MatchOutcome::Draw)]
    #[case(GameResult::Team(0), MatchOutcome::A)]
    #[case(GameResult::Team(1), MatchOutcome::B)]
    fn results_map_to_the_seat_parity(#[case] result: GameResult, #[case] outcome: MatchOutcome) {
        assert_eq!(outcome_of(result), outcome);
    }

    fn entrant(n: u32, players: &[&str]) -> Entrant {
        Entrant {
            number: n,
            name: format!("E{n}"),
            dropped: false,
            players: players
                .iter()
                .map(|name| Player {
                    user: UserId::random(),
                    name: (*name).to_owned(),
                })
                .collect(),
        }
    }

    fn tournament(format: TournamentFormat, entrants: Vec<Entrant>) -> Tournament {
        Tournament {
            id: Uuid::nil(),
            name: "T".into(),
            organiser: UserId::random(),
            organiser_name: "org".into(),
            format,
            structure: TournamentStructure::Swiss,
            rounds: None,
            status: TournamentStatus::Running,
            round: 1,
            seed: 1,
            invite: String::new(),
            entrants,
            matches: Vec::new(),
        }
    }

    fn row(a: u32, b: Option<u32>) -> MatchRow {
        MatchRow {
            round: 1,
            table: 1,
            a,
            b,
            game: None,
            outcome: None,
            corrected: false,
        }
    }

    #[rstest]
    #[case(TournamentFormat::Commander1v1, vec![entrant(0, &["ann"]), entrant(1, &["bo"])], Some(vec!["ann", "bo"]))]
    #[case(TournamentFormat::Commander2v2, vec![entrant(0, &["ann", "al"]), entrant(1, &["bo", "bea"])], Some(vec!["ann", "bo", "al", "bea"]))]
    #[case(TournamentFormat::Commander2v2, vec![entrant(0, &["ann"]), entrant(1, &["bo", "bea"])], None)]
    fn seats_alternate_between_entrants(
        #[case] format: TournamentFormat,
        #[case] entrants: Vec<Entrant>,
        #[case] names: Option<Vec<&str>>,
    ) {
        let t = tournament(format, entrants);
        let seats = seated(&t, &row(0, Some(1)));
        let got: Option<Vec<String>> = seats.map(|s| s.into_iter().map(|(n, _)| n).collect());
        assert_eq!(
            got,
            names.map(|n| n.into_iter().map(String::from).collect())
        );
        assert_eq!(seated(&t, &row(0, None)), None);
    }

    #[test]
    fn a_team_match_seats_two_teams_that_alternate_and_a_duel_none() {
        let teams = match_rules(TournamentFormat::Commander2v2).teams.unwrap();
        assert_eq!(teams.mode, TeamMode::Teams);
        assert_eq!(teams.seats, [0, 1, 0, 1]);
        assert_eq!(
            match_rules(TournamentFormat::Commander1v1),
            HouseRules::default()
        );
    }

    #[rstest]
    #[case("Friday Night", Ok("Friday Night"))]
    #[case("  padded  ", Ok("padded"))]
    #[case("", Err(()))]
    #[case("tab\there", Err(()))]
    fn names_are_trimmed_and_checked(#[case] name: &str, #[case] expected: Result<&str, ()>) {
        assert_eq!(
            checked_name(name).map_err(|_| ()),
            expected.map(String::from)
        );
        assert!(checked_name(&"x".repeat(61)).is_err());
    }
}
