-- Cards a player marked as important to fix: one row per account and card, with an optional short
-- note. The server's `users` table is its own, so `user_id` has no foreign key here: deleting an
-- account has to delete its marks as well.
CREATE TABLE card_fix_marks (
    user_id    uuid        NOT NULL,
    oracle_id  uuid        NOT NULL REFERENCES cards (oracle_id) ON DELETE CASCADE,
    note       text        CHECK (note IS NULL OR (note <> '' AND char_length(note) <= 280)),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, oracle_id)
);

COMMENT ON TABLE card_fix_marks IS 'Cards an account marked as important to fix; the owner ranks cards by how many accounts marked them (`mtg-cli fix-marks`).';
COMMENT ON COLUMN card_fix_marks.user_id IS 'Id of the account in the server''s users table; every per-account query names it.';

-- Counts the marks of a card and finds the newest note.
CREATE INDEX card_fix_marks_oracle_idx ON card_fix_marks (oracle_id, created_at DESC);
