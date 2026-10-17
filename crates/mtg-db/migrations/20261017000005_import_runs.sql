CREATE TYPE import_source AS ENUM ('scryfall_cards', 'scryfall_rulings', 'mtgjson_decks');
CREATE TYPE import_status AS ENUM ('running', 'succeeded', 'failed');

CREATE TABLE import_runs (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source      import_source NOT NULL,
    started_at  timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    status      import_status NOT NULL DEFAULT 'running',
    counts      jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(counts) = 'object'),
    error       text,
    CONSTRAINT import_runs_finished_iff_done CHECK ((status = 'running') = (finished_at IS NULL)),
    CONSTRAINT import_runs_error_iff_failed CHECK ((status = 'failed') = (error IS NOT NULL)),
    CONSTRAINT import_runs_finish_after_start CHECK (finished_at IS NULL OR finished_at >= started_at)
);

CREATE INDEX import_runs_source_started_idx ON import_runs (source, started_at DESC, id DESC);
