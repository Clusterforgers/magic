-- One row per engine event; a game's log is the rows with its id in `seq` order, with no gaps.
CREATE TABLE game_events (
    game_id    uuid        NOT NULL,
    seq        bigint      NOT NULL CHECK (seq >= 0),
    event      jsonb       NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (game_id, seq)
);
