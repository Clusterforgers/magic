-- Precons an account marked as favourites: one row per account and precon code. A code is the one the
-- server derives from a precon's set and name, so a row holds no foreign key to `precon_decks`: a
-- favourite outlives a re-import and shows again when the precon is back. The server's `users` table is
-- its own, so `user_id` has no foreign key either: deleting an account has to delete its favourites.
CREATE TABLE favourite_precons (
    user_id    uuid        NOT NULL,
    code       text        NOT NULL CHECK (code ~ '^[a-z0-9][a-z0-9-]{0,119}$'),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, code)
);

COMMENT ON TABLE favourite_precons IS 'Precons an account starred; they join its own decks in the deck lists and stay precons, never copies.';
COMMENT ON COLUMN favourite_precons.user_id IS 'Id of the account in the server''s users table; every per-account query names it.';
