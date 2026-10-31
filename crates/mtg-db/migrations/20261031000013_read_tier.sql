-- The read tier: drafts of cards the checker cannot prove, and the verdicts players give them.

-- A draft is a whole rules file for a card (`mtg_cards::card::Card` as JSON), written by
-- `mtg-cli compile` from the drafts the build embeds. A row is never deleted: a draft the build no
-- longer holds is retired, so a game that started on it resumes. A card has at most one row that is
-- not retired, its current draft.
CREATE TABLE card_drafts (
    oracle_id  uuid        NOT NULL REFERENCES cards (oracle_id) ON DELETE CASCADE,
    -- Digest of the oracle id, the faces' fingerprints and the draft's implementation.
    read_hash  text        NOT NULL CHECK (read_hash ~ '^[0-9a-f]{32}$'),
    rules      jsonb       NOT NULL CHECK (jsonb_typeof(rules) = 'object'),
    retired    boolean     NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (oracle_id, read_hash)
);

COMMENT ON TABLE card_drafts IS 'Read-tier drafts per oracle card, written by `mtg-cli compile`; retired, never deleted.';
COMMENT ON COLUMN card_drafts.read_hash IS 'What a verdict refers to: a changed draft or card text gives a new hash, which is read again.';

-- One current draft per card.
CREATE UNIQUE INDEX card_drafts_current_idx ON card_drafts (oracle_id) WHERE NOT retired;

CREATE TYPE read_verdict AS ENUM ('correct', 'wrong');

-- Verdicts are appended and never changed. An account's newest row on a hash is its verdict.
-- `user_id` is null for the server's own verdict (a resolution that failed in a game), which is
-- always `wrong`. The server's `users` table is its own, so `user_id` has no foreign key here:
-- deleting an account has to delete its verdicts as well.
CREATE TABLE card_reads (
    id         bigint       GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id    uuid,
    oracle_id  uuid         NOT NULL,
    read_hash  text         NOT NULL,
    verdict    read_verdict NOT NULL,
    note       text         CHECK (note IS NULL OR (note <> '' AND char_length(note) <= 280)),
    created_at timestamptz  NOT NULL DEFAULT now(),
    FOREIGN KEY (oracle_id, read_hash) REFERENCES card_drafts (oracle_id, read_hash) ON DELETE CASCADE,
    CHECK (verdict <> 'wrong' OR note IS NOT NULL),
    CHECK (user_id IS NOT NULL OR verdict = 'wrong')
);

COMMENT ON TABLE card_reads IS 'Append-only verdicts of accounts (and of the server) on read hashes; only `ReadRepo::delete_all_of` removes rows, with the account.';
COMMENT ON COLUMN card_reads.user_id IS 'Id of the account in the server''s users table; null for the server''s own verdict.';

-- The verdicts on a hash, and the newest verdict of one account on a card.
CREATE INDEX card_reads_hash_idx ON card_reads (oracle_id, read_hash, id);
CREATE INDEX card_reads_user_idx ON card_reads (user_id, oracle_id, id DESC);

CREATE FUNCTION card_reads_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'card_reads is append-only' USING ERRCODE = 'integrity_constraint_violation';
END
$$;

CREATE TRIGGER card_reads_no_update BEFORE UPDATE ON card_reads
    FOR EACH ROW EXECUTE FUNCTION card_reads_append_only();
