-- Stored decks: one row per deck and one per line of it. A deck belongs to an account of the
-- server, whose `users` table is the server's own, so `owner` has no foreign key here; the
-- server deletes a user's decks with the account.
CREATE TABLE decks (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner      uuid        NOT NULL,
    name       text        NOT NULL CHECK (name <> '' AND char_length(name) <= 200),
    -- A format key of the deck check (`commander`, `modern`, `freeform`, ...).
    format     text        NOT NULL CHECK (format ~ '^[a-z][a-z0-9_]{0,31}$'),
    notes      text        NOT NULL DEFAULT '' CHECK (char_length(notes) <= 20000),
    -- The last check the server ran when the deck was saved:
    -- {deck_hash, coverage_hash, checked_at, summary}. Whether the deck is verified now is derived
    -- from it and the check of the moment, never stored as a flag.
    last_check jsonb       CHECK (last_check IS NULL OR jsonb_typeof(last_check) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON COLUMN decks.owner IS 'Id of the account in the server''s users table; every query names it, so one account never reads another''s decks.';

-- Lists an owner's decks, the most recently saved first.
CREATE INDEX decks_owner_idx ON decks (owner, updated_at DESC, id);

CREATE TABLE deck_cards (
    deck_id     uuid    NOT NULL REFERENCES decks (id) ON DELETE CASCADE,
    -- The order of the lines as the owner keeps them.
    position    integer NOT NULL CHECK (position >= 0),
    zone        text    NOT NULL CHECK (zone IN ('main', 'sideboard', 'commander', 'companion')),
    oracle_id   uuid    NOT NULL REFERENCES cards (oracle_id),
    -- The art the owner chose; it is no part of what the deck check hashes.
    printing_id uuid    REFERENCES printings (printing_id) ON DELETE SET NULL,
    count       integer NOT NULL CHECK (count BETWEEN 1 AND 999),
    PRIMARY KEY (deck_id, position)
);

CREATE INDEX deck_cards_oracle_idx ON deck_cards (oracle_id);
