-- Accounts. A username is stored as typed (`username`, what other players see) and in lower case
-- (`username_key`, unique), so "Ann" and "ann" are one name. The checks repeat the server's rules,
-- so a bug in the server cannot store a name it would refuse. `password_hash` is an Argon2id
-- PHC string, never the password.
CREATE TABLE users (
    id            uuid        PRIMARY KEY,
    username      text        NOT NULL CHECK (username ~ '^[A-Za-z0-9_-]{3,32}$'),
    username_key  text        NOT NULL UNIQUE CHECK (username_key = lower(username)),
    password_hash text        NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now()
);

-- Browser sessions. Only the SHA-256 of the cookie's token is kept, like the seat tokens.
-- A session ends at `expires_at`, by the database's clock, or when its row is deleted.
CREATE TABLE sessions (
    token_digest bytea       PRIMARY KEY CHECK (octet_length(token_digest) = 32),
    user_id      uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL
);

CREATE INDEX sessions_by_user ON sessions (user_id);
CREATE INDEX sessions_by_expiry ON sessions (expires_at);
