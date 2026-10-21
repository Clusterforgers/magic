-- Rules the oracle-text compiler generated, one row per card: what the server reads next to the
-- hand-written rules files, which win for the same oracle id.
CREATE TABLE card_rules (
    oracle_id        uuid        PRIMARY KEY REFERENCES cards (oracle_id) ON DELETE CASCADE,
    -- The card's rules as the DSL's JSON (`mtg_cards::card::Card`).
    rules            jsonb       NOT NULL CHECK (jsonb_typeof(rules) = 'object'),
    -- How much of the card the rules cover; a card the compiler reads nothing of has no row.
    status           impl_status NOT NULL CHECK (status <> 'manual'),
    -- Digest of the printed data the compiler read (names, costs, type lines, oracle text, keywords).
    fingerprint      text        NOT NULL CHECK (fingerprint ~ '^[0-9a-f]{32}$'),
    -- Digest of the compiler and the DSL it wrote with; a row of another version is written again.
    compiler_version text        NOT NULL CHECK (compiler_version <> ''),
    created_at       timestamptz NOT NULL DEFAULT now(),
    updated_at       timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE card_rules IS 'Compiled rules per oracle card, written by `mtg-cli compile`; hand-written rules files win for the same oracle id.';
COMMENT ON COLUMN card_rules.fingerprint IS 'Digest of what the compiler read of the stored card; a different digest means the rules are out of date.';
