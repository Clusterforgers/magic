CREATE TABLE printings (
    printing_id      uuid PRIMARY KEY,
    oracle_id        uuid NOT NULL REFERENCES cards (oracle_id) ON DELETE CASCADE,
    set_code         text NOT NULL,
    collector_number text NOT NULL,
    rarity           text CHECK (rarity IN ('common', 'uncommon', 'rare', 'mythic', 'special', 'bonus')),
    released_at      date,
    lang             text NOT NULL CHECK (lang <> ''),
    digital          boolean NOT NULL DEFAULT false,
    images           jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(images) = 'object'),
    created_at       timestamptz NOT NULL DEFAULT now(),
    updated_at       timestamptz NOT NULL DEFAULT now()
);

COMMENT ON COLUMN printings.set_code IS 'Set code; named set_code because SET is an SQL keyword.';
COMMENT ON COLUMN printings.images IS 'JSON object {"card": image urls or null, "faces": [image urls or null, ...]}.';

-- Lists a card's printings newest first without a sort.
CREATE INDEX printings_oracle_idx ON printings (oracle_id, released_at DESC NULLS LAST, printing_id);
CREATE INDEX printings_set_number_idx ON printings (set_code, collector_number);
