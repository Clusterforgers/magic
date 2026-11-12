-- Tournaments: an organiser's rounds of games between entrants who signed up by invite.
-- `invite` is kept in clear: it only lets a signed-in account sign up, and the organiser shows it
-- again from the tournament page. The organiser's account takes its tournaments with it.
CREATE TABLE tournaments (
    id            uuid        PRIMARY KEY,
    name          text        NOT NULL CHECK (char_length(name) BETWEEN 1 AND 60),
    organiser     uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    format        text        NOT NULL CHECK (format IN ('commander_1v1', 'commander_2v2')),
    structure     text        NOT NULL CHECK (structure IN ('swiss', 'elimination')),
    rounds        integer     CHECK (rounds BETWEEN 1 AND 15),
    status        text        NOT NULL DEFAULT 'signup' CHECK (status IN ('signup', 'running', 'finished')),
    round         integer     NOT NULL DEFAULT 0 CHECK (round >= 0),
    seed          bigint      NOT NULL,
    invite        text        NOT NULL UNIQUE,
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX tournaments_by_organiser ON tournaments (organiser);

-- An entrant is a player alone or a team, numbered from 0 in sign-up order.
CREATE TABLE tournament_entrants (
    tournament_id uuid    NOT NULL REFERENCES tournaments (id) ON DELETE CASCADE,
    entrant       integer NOT NULL CHECK (entrant >= 0),
    name          text    NOT NULL CHECK (char_length(name) BETWEEN 1 AND 60),
    dropped       boolean NOT NULL DEFAULT false,
    PRIMARY KEY (tournament_id, entrant)
);

-- The accounts of an entrant; an account plays for one entrant of a tournament.
CREATE TABLE tournament_players (
    tournament_id uuid        NOT NULL,
    entrant       integer     NOT NULL,
    user_id       uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    joined_at     timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tournament_id, user_id),
    FOREIGN KEY (tournament_id, entrant) REFERENCES tournament_entrants (tournament_id, entrant) ON DELETE CASCADE
);

CREATE INDEX tournament_players_by_user ON tournament_players (user_id);

-- One match of a round: two entrants, or a bye for `a`. `game_id` is the game the match is played
-- in, without a foreign key so a game record can go first; `corrected` marks a result the
-- organiser set.
CREATE TABLE tournament_matches (
    tournament_id uuid    NOT NULL REFERENCES tournaments (id) ON DELETE CASCADE,
    round         integer NOT NULL CHECK (round >= 1),
    table_no      integer NOT NULL CHECK (table_no >= 1),
    a             integer NOT NULL,
    b             integer,
    game_id       uuid,
    outcome       text    CHECK (outcome IN ('a', 'b', 'draw')),
    corrected     boolean NOT NULL DEFAULT false,
    PRIMARY KEY (tournament_id, round, table_no),
    CHECK (b IS NOT NULL OR (game_id IS NULL AND outcome IS NULL))
);
