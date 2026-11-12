//! The tournament tables: tournaments, entrants, their players and the matches of each round.

use mtg_protocol::rest::{MatchOutcome, TournamentFormat, TournamentStatus, TournamentStructure};
use sqlx::PgPool;
use uuid::Uuid;

use super::pairing::Pairing;
use crate::game_id::GameId;
use crate::records::RecordError;
use crate::user_id::UserId;

/// A new tournament as the organiser describes it.
#[derive(Debug, Clone)]
pub(crate) struct NewTournament {
    /// Its id.
    pub(crate) id: Uuid,
    /// Its name, already checked.
    pub(crate) name: String,
    /// Who runs it.
    pub(crate) organiser: UserId,
    /// What each match is played as.
    pub(crate) format: TournamentFormat,
    /// How rounds are paired.
    pub(crate) structure: TournamentStructure,
    /// Swiss rounds planned; none to decide at the start.
    pub(crate) rounds: Option<u32>,
    /// The seed of its shuffles.
    pub(crate) seed: u64,
    /// The invite code.
    pub(crate) invite: String,
}

/// One player of an entrant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Player {
    /// The account.
    pub(crate) user: UserId,
    /// Its username.
    pub(crate) name: String,
}

/// One entrant with its players in sign-up order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entrant {
    /// The entrant's number.
    pub(crate) number: u32,
    /// Its name.
    pub(crate) name: String,
    /// Whether it left.
    pub(crate) dropped: bool,
    /// Its players.
    pub(crate) players: Vec<Player>,
}

/// One stored match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchRow {
    /// The round, from 1.
    pub(crate) round: u32,
    /// The table, from 1.
    pub(crate) table: u32,
    /// The first entrant.
    pub(crate) a: u32,
    /// The second entrant; none for a bye.
    pub(crate) b: Option<u32>,
    /// The game the match is played in.
    pub(crate) game: Option<GameId>,
    /// The result.
    pub(crate) outcome: Option<MatchOutcome>,
    /// Whether the organiser set the result.
    pub(crate) corrected: bool,
}

/// A whole tournament as stored.
#[derive(Debug, Clone)]
pub(crate) struct Tournament {
    /// Its id.
    pub(crate) id: Uuid,
    /// Its name.
    pub(crate) name: String,
    /// Who runs it.
    pub(crate) organiser: UserId,
    /// The organiser's username.
    pub(crate) organiser_name: String,
    /// What each match is played as.
    pub(crate) format: TournamentFormat,
    /// How rounds are paired.
    pub(crate) structure: TournamentStructure,
    /// Rounds planned, once known.
    pub(crate) rounds: Option<u32>,
    /// Where it is in its life.
    pub(crate) status: TournamentStatus,
    /// The round being played; 0 before the start.
    pub(crate) round: u32,
    /// The seed of its shuffles.
    pub(crate) seed: u64,
    /// The invite code.
    pub(crate) invite: String,
    /// Every entrant in sign-up order.
    pub(crate) entrants: Vec<Entrant>,
    /// Every match, by round and table.
    pub(crate) matches: Vec<MatchRow>,
}

/// One tournament of a player's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listed {
    /// Its id.
    pub(crate) id: Uuid,
    /// Its name.
    pub(crate) name: String,
    /// What each match is played as.
    pub(crate) format: TournamentFormat,
    /// How rounds are paired.
    pub(crate) structure: TournamentStructure,
    /// Where it is in its life.
    pub(crate) status: TournamentStatus,
    /// The round being played.
    pub(crate) round: u32,
    /// Entrants signed up.
    pub(crate) entrants: u32,
    /// Whether the player organises it.
    pub(crate) organiser: bool,
}

/// What a sign-up came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Signup {
    /// The player is in, as this entrant.
    Joined(u32),
    /// The tournament has started.
    Started,
    /// The player is in already.
    Already,
    /// The tournament, or the chosen team, has no open place.
    Full,
    /// The chosen team does not exist.
    NoTeam,
}

/// The tournament tables over one pool.
#[derive(Debug, Clone)]
pub(crate) struct TournamentStore {
    pool: PgPool,
}

fn number(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

fn int(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn corrupt(what: &str, text: &str) -> RecordError {
    RecordError::Corrupt(format!("unknown tournament {what} {text:?}"))
}

impl TournamentStore {
    /// A store over `pool`, whose server migrations have run.
    pub(crate) fn new(pool: PgPool) -> TournamentStore {
        TournamentStore { pool }
    }

    /// Writes a new tournament in sign-up.
    pub(crate) async fn create(&self, new: &NewTournament) -> Result<(), RecordError> {
        sqlx::query!(
            "INSERT INTO tournaments (id, name, organiser, format, structure, rounds, seed, invite)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            new.id,
            new.name,
            new.organiser.uuid(),
            new.format.as_str(),
            new.structure.as_str(),
            new.rounds.map(int),
            // The seed's bits as stored; read back the same way.
            i64::from_ne_bytes(new.seed.to_ne_bytes()),
            new.invite,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The tournaments `user` organises or plays in, newest first, at most `limit`.
    pub(crate) async fn list_for(
        &self,
        user: UserId,
        limit: i64,
    ) -> Result<Vec<Listed>, RecordError> {
        let rows = sqlx::query!(
            r#"SELECT t.id, t.name, t.format, t.structure, t.status, t.round,
                      (t.organiser = $1) AS "organiser!",
                      (SELECT count(*) FROM tournament_entrants e WHERE e.tournament_id = t.id) AS "entrants!"
               FROM tournaments t
               WHERE t.organiser = $1
                  OR EXISTS (SELECT 1 FROM tournament_players p WHERE p.tournament_id = t.id AND p.user_id = $1)
               ORDER BY t.created_at DESC
               LIMIT $2"#,
            user.uuid(),
            limit,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(Listed {
                    id: row.id,
                    name: row.name,
                    format: TournamentFormat::parse(&row.format)
                        .ok_or_else(|| corrupt("format", &row.format))?,
                    structure: TournamentStructure::parse(&row.structure)
                        .ok_or_else(|| corrupt("structure", &row.structure))?,
                    status: TournamentStatus::parse(&row.status)
                        .ok_or_else(|| corrupt("status", &row.status))?,
                    round: number(row.round),
                    entrants: u32::try_from(row.entrants).unwrap_or(0),
                    organiser: row.organiser,
                })
            })
            .collect()
    }

    /// The tournament an invite code opens.
    pub(crate) async fn by_invite(&self, invite: &str) -> Result<Option<Uuid>, RecordError> {
        let id = sqlx::query_scalar!("SELECT id FROM tournaments WHERE invite = $1", invite)
            .fetch_optional(&self.pool)
            .await?;
        Ok(id)
    }

    /// One whole tournament.
    pub(crate) async fn load(&self, id: Uuid) -> Result<Option<Tournament>, RecordError> {
        let Some(row) = sqlx::query!(
            "SELECT t.id, t.name, t.organiser, u.username, t.format, t.structure, t.rounds,
                    t.status, t.round, t.seed, t.invite
             FROM tournaments t JOIN users u ON u.id = t.organiser
             WHERE t.id = $1",
            id,
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let entrant_rows = sqlx::query!(
            "SELECT entrant, name, dropped FROM tournament_entrants
             WHERE tournament_id = $1 ORDER BY entrant",
            id,
        )
        .fetch_all(&self.pool)
        .await?;
        let player_rows = sqlx::query!(
            "SELECT p.entrant, p.user_id, u.username FROM tournament_players p
             JOIN users u ON u.id = p.user_id
             WHERE p.tournament_id = $1 ORDER BY p.joined_at, p.user_id",
            id,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entrants: Vec<Entrant> = entrant_rows
            .into_iter()
            .map(|e| Entrant {
                number: number(e.entrant),
                name: e.name,
                dropped: e.dropped,
                players: Vec::new(),
            })
            .collect();
        for p in player_rows {
            if let Some(entrant) = entrants.iter_mut().find(|e| e.number == number(p.entrant)) {
                entrant.players.push(Player {
                    user: UserId(p.user_id),
                    name: p.username,
                });
            }
        }
        let status =
            TournamentStatus::parse(&row.status).ok_or_else(|| corrupt("status", &row.status))?;
        // An entrant whose players all deleted their accounts is gone before the start and
        // dropped after it.
        if status == TournamentStatus::Signup {
            entrants.retain(|e| !e.players.is_empty());
        }
        for entrant in &mut entrants {
            entrant.dropped |= entrant.players.is_empty();
        }
        let match_rows = sqlx::query!(
            "SELECT round, table_no, a, b, game_id, outcome, corrected FROM tournament_matches
             WHERE tournament_id = $1 ORDER BY round, table_no",
            id,
        )
        .fetch_all(&self.pool)
        .await?;
        let matches = match_rows
            .into_iter()
            .map(|m| {
                let outcome = match m.outcome {
                    Some(text) => {
                        Some(MatchOutcome::parse(&text).ok_or_else(|| corrupt("outcome", &text))?)
                    }
                    None => None,
                };
                Ok(MatchRow {
                    round: number(m.round),
                    table: number(m.table_no),
                    a: number(m.a),
                    b: m.b.map(number),
                    game: m.game_id.map(GameId::from),
                    outcome,
                    corrected: m.corrected,
                })
            })
            .collect::<Result<Vec<_>, RecordError>>()?;
        Ok(Some(Tournament {
            id: row.id,
            name: row.name,
            organiser: UserId(row.organiser),
            organiser_name: row.username,
            format: TournamentFormat::parse(&row.format)
                .ok_or_else(|| corrupt("format", &row.format))?,
            structure: TournamentStructure::parse(&row.structure)
                .ok_or_else(|| corrupt("structure", &row.structure))?,
            rounds: row.rounds.map(number),
            status,
            round: number(row.round),
            seed: u64::from_ne_bytes(row.seed.to_ne_bytes()),
            invite: row.invite,
            entrants,
            matches,
        }))
    }

    /// Signs `user` up: into the entrant `team` when given, else as a new entrant named `name`.
    /// `team_size` players fill an entrant and `max_entrants` fill the tournament.
    pub(crate) async fn join(
        &self,
        id: Uuid,
        user: UserId,
        team: Option<u32>,
        name: &str,
        team_size: usize,
        max_entrants: usize,
    ) -> Result<Signup, RecordError> {
        let mut tx = self.pool.begin().await?;
        // Sign-ups and the start take the tournament's row in turn.
        let status = sqlx::query_scalar!(
            "SELECT status FROM tournaments WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(&mut *tx)
        .await?;
        match status.as_deref() {
            Some("signup") => {}
            Some(_) => return Ok(Signup::Started),
            None => return Ok(Signup::NoTeam),
        }
        let already = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM tournament_players WHERE tournament_id = $1 AND user_id = $2) AS "e!""#,
            id,
            user.uuid(),
        )
        .fetch_one(&mut *tx)
        .await?;
        if already {
            return Ok(Signup::Already);
        }
        let entrant = if let Some(team) = team {
            let size = sqlx::query_scalar!(
                    r#"SELECT (SELECT count(*) FROM tournament_players p
                               WHERE p.tournament_id = e.tournament_id AND p.entrant = e.entrant) AS "n!"
                       FROM tournament_entrants e WHERE e.tournament_id = $1 AND e.entrant = $2"#,
                    id,
                    int(team),
                )
                .fetch_optional(&mut *tx)
                .await?;
            match size {
                None => return Ok(Signup::NoTeam),
                Some(size) if usize::try_from(size).unwrap_or(usize::MAX) >= team_size => {
                    return Ok(Signup::Full);
                }
                Some(_) => team,
            }
        } else {
            let (count, next) = sqlx::query!(
                r#"SELECT count(*) AS "n!", coalesce(max(entrant) + 1, 0) AS "next!"
                       FROM tournament_entrants WHERE tournament_id = $1"#,
                id,
            )
            .fetch_one(&mut *tx)
            .await
            .map(|row| (row.n, row.next))?;
            if usize::try_from(count).unwrap_or(usize::MAX) >= max_entrants {
                return Ok(Signup::Full);
            }
            sqlx::query!(
                    "INSERT INTO tournament_entrants (tournament_id, entrant, name) VALUES ($1, $2, $3)",
                    id,
                    next,
                    name,
                )
                .execute(&mut *tx)
                .await?;
            number(next)
        };
        sqlx::query!(
            "INSERT INTO tournament_players (tournament_id, entrant, user_id) VALUES ($1, $2, $3)",
            id,
            int(entrant),
            user.uuid(),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Signup::Joined(entrant))
    }

    /// Takes `user` out of a tournament in sign-up; an entrant left without players goes too.
    /// False when the user was not in it or it has started.
    pub(crate) async fn leave(&self, id: Uuid, user: UserId) -> Result<bool, RecordError> {
        let mut tx = self.pool.begin().await?;
        let status = sqlx::query_scalar!(
            "SELECT status FROM tournaments WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if status.as_deref() != Some("signup") {
            return Ok(false);
        }
        let entrant = sqlx::query_scalar!(
            "DELETE FROM tournament_players WHERE tournament_id = $1 AND user_id = $2 RETURNING entrant",
            id,
            user.uuid(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(entrant) = entrant else {
            return Ok(false);
        };
        sqlx::query!(
            "DELETE FROM tournament_entrants e WHERE e.tournament_id = $1 AND e.entrant = $2
               AND NOT EXISTS (SELECT 1 FROM tournament_players p
                               WHERE p.tournament_id = $1 AND p.entrant = $2)",
            id,
            entrant,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Marks an entrant dropped; false when there is no such entrant.
    pub(crate) async fn drop_entrant(&self, id: Uuid, entrant: u32) -> Result<bool, RecordError> {
        let done = sqlx::query!(
            "UPDATE tournament_entrants SET dropped = true WHERE tournament_id = $1 AND entrant = $2",
            id,
            int(entrant),
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    /// Stores round `round` with its pairings by table, sets the planned `rounds` and marks the
    /// tournament running; false when it is not at round `round - 1` any more.
    pub(crate) async fn begin_round(
        &self,
        id: Uuid,
        round: u32,
        rounds: Option<u32>,
        pairings: &[Pairing],
    ) -> Result<bool, RecordError> {
        let mut tx = self.pool.begin().await?;
        let moved = sqlx::query!(
            "UPDATE tournaments SET round = $2, rounds = $3, status = 'running'
             WHERE id = $1 AND round = $2 - 1 AND status <> 'finished'",
            id,
            int(round),
            rounds.map(int),
        )
        .execute(&mut *tx)
        .await?;
        if moved.rows_affected() != 1 {
            return Ok(false);
        }
        let tables: Vec<i32> = (1..=pairings.len())
            .map(|t| i32::try_from(t).unwrap_or(i32::MAX))
            .collect();
        let a: Vec<i32> = pairings.iter().map(|p| int(p.a)).collect();
        let b: Vec<Option<i32>> = pairings.iter().map(|p| p.b.map(int)).collect();
        sqlx::query!(
            "INSERT INTO tournament_matches (tournament_id, round, table_no, a, b)
             SELECT $1, $2, t.table_no, t.a, t.b
             FROM unnest($3::int4[], $4::int4[], $5::int4[]) AS t(table_no, a, b)",
            id,
            int(round),
            &tables[..],
            &a[..],
            // A bye has no second entrant: the macro cannot see that elements may be NULL.
            &b[..] as _,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Notes the game of a match that has none.
    pub(crate) async fn set_game(
        &self,
        id: Uuid,
        round: u32,
        table: u32,
        game: GameId,
    ) -> Result<bool, RecordError> {
        let done = sqlx::query!(
            "UPDATE tournament_matches SET game_id = $4
             WHERE tournament_id = $1 AND round = $2 AND table_no = $3
               AND b IS NOT NULL AND game_id IS NULL",
            id,
            int(round),
            int(table),
            game.uuid(),
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    /// Whether `game` is the game of a tournament match.
    pub(crate) async fn is_match_game(&self, game: GameId) -> Result<bool, RecordError> {
        let found = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM tournament_matches WHERE game_id = $1) AS "found!""#,
            game.uuid(),
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(found)
    }

    /// Forgets `game` as the game of a match that has no result, so a new one can be made; false when it was not.
    pub(crate) async fn clear_game(
        &self,
        id: Uuid,
        round: u32,
        table: u32,
        game: GameId,
    ) -> Result<bool, RecordError> {
        let done = sqlx::query!(
            "UPDATE tournament_matches SET game_id = NULL
             WHERE tournament_id = $1 AND round = $2 AND table_no = $3
               AND game_id = $4 AND outcome IS NULL",
            id,
            int(round),
            int(table),
            game.uuid(),
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    /// Notes a match's result. A result from the game fills only an empty one; the organiser's
    /// (`corrected`) always wins. False when no played match was changed.
    pub(crate) async fn set_outcome(
        &self,
        id: Uuid,
        round: u32,
        table: u32,
        outcome: MatchOutcome,
        corrected: bool,
    ) -> Result<bool, RecordError> {
        let done = sqlx::query!(
            "UPDATE tournament_matches SET outcome = $4, corrected = $5
             WHERE tournament_id = $1 AND round = $2 AND table_no = $3 AND b IS NOT NULL
               AND ($5 OR outcome IS NULL)",
            id,
            int(round),
            int(table),
            outcome.as_str(),
            corrected,
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    /// Ends the tournament.
    pub(crate) async fn finish(&self, id: Uuid) -> Result<(), RecordError> {
        sqlx::query!(
            "UPDATE tournaments SET status = 'finished' WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Removes a tournament with everything in it; false when there was none.
    pub(crate) async fn delete(&self, id: Uuid) -> Result<bool, RecordError> {
        let done = sqlx::query!("DELETE FROM tournaments WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() == 1)
    }
}
