-- Delves: co-op boss-rush runs. A run is its seed and the log of its events (`docs/delves.md`); the ladder and the
-- party's state are folded from them. The host's account takes its runs with it.
CREATE TABLE delves (
    id          uuid        PRIMARY KEY,
    code        text        NOT NULL UNIQUE,
    host        uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    theme       text        NOT NULL,
    seed        bigint      NOT NULL,
    heat        smallint    NOT NULL CHECK (heat BETWEEN 0 AND 2),
    status      text        NOT NULL DEFAULT 'lobby',
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX delves_by_host ON delves (host);

-- The events of a run in order, from 0.
CREATE TABLE delve_events (
    delve_id uuid    NOT NULL REFERENCES delves (id) ON DELETE CASCADE,
    seq      integer NOT NULL CHECK (seq >= 0),
    event    jsonb   NOT NULL,
    PRIMARY KEY (delve_id, seq)
);

-- The accounts in a run's party, for their list of runs.
CREATE TABLE delve_members (
    delve_id uuid NOT NULL REFERENCES delves (id) ON DELETE CASCADE,
    user_id  uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    PRIMARY KEY (delve_id, user_id)
);

CREATE INDEX delve_members_by_user ON delve_members (user_id);
