-- Deck groups: an account's named shelves for its decks ("Fun", "Hardcore"), in the owner's order, each
-- collapsed or open. A deck sits in at most one group; a deck in none is ungrouped. The server's `users`
-- table is its own, so `owner` has no foreign key: deleting an account has to delete its groups as well.
CREATE TABLE deck_groups (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner      uuid        NOT NULL,
    name       text        NOT NULL CHECK (btrim(name) <> '' AND char_length(name) <= 60),
    position   integer     NOT NULL CHECK (position >= 0),
    collapsed  boolean     NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX deck_groups_owner_idx ON deck_groups (owner, position, id);

-- Which group a deck sits in. Deleting the group or the deck deletes the row, so the deck is ungrouped.
CREATE TABLE deck_group_members (
    deck_id  uuid NOT NULL PRIMARY KEY REFERENCES decks (id) ON DELETE CASCADE,
    group_id uuid NOT NULL REFERENCES deck_groups (id) ON DELETE CASCADE
);

CREATE INDEX deck_group_members_group_idx ON deck_group_members (group_id);

COMMENT ON COLUMN deck_groups.owner IS 'Id of the account in the server''s users table; every query names it, so one account never reads another''s groups.';
