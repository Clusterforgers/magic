-- One row per game: the recipe that rebuilds its setup and where it is in its life.
-- The event log stays in `game_events` and has no foreign key here, so a log can be stored first.
CREATE TABLE games (
    id          uuid        PRIMARY KEY,
    status      text        NOT NULL CHECK (status IN ('lobby', 'running', 'finished', 'expired', 'failed')),
    recipe      jsonb       NOT NULL,
    result      jsonb,
    failure     text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    started_at  timestamptz,
    finished_at timestamptz
);

-- Boot reads only the games that can still be played.
CREATE INDEX games_open ON games (created_at) WHERE status IN ('lobby', 'running');

-- One row per seat. Only the SHA-256 of the seat's token is kept, never the token.
CREATE TABLE seats (
    game_id      uuid    NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    seat         integer NOT NULL CHECK (seat >= 0),
    name         text    NOT NULL,
    token_digest bytea   NOT NULL CHECK (octet_length(token_digest) = 32),
    joined       boolean NOT NULL DEFAULT false,
    PRIMARY KEY (game_id, seat)
);
