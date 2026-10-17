CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- How much of a card the rules engine implements; imports never touch it.
CREATE TYPE impl_status AS ENUM ('manual', 'partial', 'full');

CREATE TABLE cards (
    oracle_id             uuid PRIMARY KEY,
    name                  text NOT NULL CHECK (name <> ''),
    -- Byte-wise collation keeps the default ordering identical on every server.
    name_normalized       text COLLATE "C" NOT NULL CHECK (name_normalized <> ''),
    front_name_normalized text NOT NULL CHECK (front_name_normalized <> ''),
    layout                text NOT NULL CHECK (layout <> ''),
    is_deck_card          boolean NOT NULL,
    mana_cost             text NOT NULL DEFAULT '',
    mana_value            double precision NOT NULL CHECK (mana_value >= 0),
    type_line             text NOT NULL DEFAULT '',
    oracle_text           text NOT NULL DEFAULT '',
    keywords              text[] NOT NULL DEFAULT '{}',
    colors                smallint NOT NULL DEFAULT 0 CHECK (colors BETWEEN 0 AND 31),
    color_identity        smallint NOT NULL DEFAULT 0 CHECK (color_identity BETWEEN 0 AND 31),
    produced_mana         smallint NOT NULL DEFAULT 0 CHECK (produced_mana BETWEEN 0 AND 31),
    produces_colorless    boolean NOT NULL DEFAULT false,
    legalities            jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(legalities) = 'object'),
    faces                 jsonb NOT NULL CHECK (jsonb_typeof(faces) = 'array' AND jsonb_array_length(faces) >= 1),
    impl_status           impl_status NOT NULL DEFAULT 'manual',
    oracle_tsv            tsvector GENERATED ALWAYS AS (to_tsvector('english', oracle_text)) STORED,
    created_at            timestamptz NOT NULL DEFAULT now(),
    updated_at            timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE cards IS 'One row per oracle card; printings live in `printings`.';
COMMENT ON COLUMN cards.name_normalized IS 'Name folded by mtg_import::resolve::normalize_name; the key for exact and fuzzy lookup.';
COMMENT ON COLUMN cards.front_name_normalized IS 'Normalized front-face name; equals name_normalized for single-faced cards.';
COMMENT ON COLUMN cards.is_deck_card IS 'False for tokens, emblems, art cards and other objects that cannot be put in a deck.';
COMMENT ON COLUMN cards.oracle_text IS 'Rules text of all faces, joined with newline, double slash, newline.';
COMMENT ON COLUMN cards.colors IS 'Color bitmask: W=1, U=2, B=4, R=8, G=16; 0 is colorless.';
COMMENT ON COLUMN cards.color_identity IS 'Color identity bitmask, same bits as colors; a deck fits a commander when identity & ~commander = 0.';
COMMENT ON COLUMN cards.produced_mana IS 'Bitmask of colored mana the card can produce, same bits as colors.';
COMMENT ON COLUMN cards.legalities IS 'Format name to legality keyword, e.g. {"commander": "legal"}.';
COMMENT ON COLUMN cards.faces IS 'Per-face oracle data as a JSON array, front face first; images live on printings.';

-- Serves the default ordering and exact lookups by normalized full name.
CREATE INDEX cards_name_order_idx ON cards (name_normalized, oracle_id);
CREATE INDEX cards_front_name_idx ON cards (front_name_normalized);
CREATE INDEX cards_name_trgm_idx ON cards USING gin (name_normalized gin_trgm_ops);
CREATE INDEX cards_type_line_trgm_idx ON cards USING gin (type_line gin_trgm_ops);
CREATE INDEX cards_oracle_tsv_idx ON cards USING gin (oracle_tsv);
CREATE INDEX cards_mana_value_idx ON cards (mana_value);
CREATE INDEX cards_colors_idx ON cards (colors);
CREATE INDEX cards_color_identity_idx ON cards (color_identity);
CREATE INDEX cards_legalities_idx ON cards USING gin (legalities jsonb_path_ops);
