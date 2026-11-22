-- The card feed: each daily refresh of the card data, and the coverage snapshots the Card support page shows.
CREATE TYPE refresh_status AS ENUM ('running', 'succeeded', 'deferred', 'failed');

CREATE TABLE card_refresh_runs (
    id                  bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    started_at          timestamptz NOT NULL DEFAULT now(),
    finished_at         timestamptz,
    status              refresh_status NOT NULL DEFAULT 'running',
    release             text CHECK (release <> ''),
    scryfall_updated_at text,
    mtgjson_build       text,
    new_card_count      integer NOT NULL DEFAULT 0 CHECK (new_card_count >= 0),
    new_cards           jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(new_cards) = 'array'),
    new_precons         jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(new_precons) = 'array'),
    first_load          boolean NOT NULL DEFAULT false,
    steps               jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(steps) = 'array'),
    failures            jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(failures) = 'array'),
    CONSTRAINT card_refresh_runs_finished_iff_done CHECK ((status = 'running') = (finished_at IS NULL))
);

CREATE INDEX card_refresh_runs_started_idx ON card_refresh_runs (started_at DESC, id DESC);

COMMENT ON TABLE card_refresh_runs IS 'One run of `mtg-cli refresh`: the upstream builds it saw, the cards and precons it added and what failed.';
COMMENT ON COLUMN card_refresh_runs.new_cards IS 'Names of the deck cards the run added, at most 1,000; `new_card_count` counts them all.';
COMMENT ON COLUMN card_refresh_runs.new_precons IS 'The precons the run added, as `{code, name, set_name}` objects.';
COMMENT ON COLUMN card_refresh_runs.first_load IS 'The run loaded an empty database, so its new cards and precons are not news.';

CREATE TABLE coverage_snapshots (
    id              bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    taken_at        timestamptz NOT NULL DEFAULT now(),
    release         text CHECK (release <> ''),
    cards           integer NOT NULL CHECK (cards >= 0),
    automatic       integer NOT NULL CHECK (automatic >= 0 AND automatic <= cards),
    states          jsonb NOT NULL CHECK (jsonb_typeof(states) = 'object'),
    buckets         jsonb NOT NULL CHECK (jsonb_typeof(buckets) = 'array'),
    missing_count   integer NOT NULL CHECK (missing_count >= 0),
    missing         jsonb CHECK (missing IS NULL OR jsonb_typeof(missing) = 'array'),
    newly_supported_count integer NOT NULL DEFAULT 0 CHECK (newly_supported_count >= 0),
    newly_supported jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(newly_supported) = 'array')
);

CREATE INDEX coverage_snapshots_taken_idx ON coverage_snapshots (taken_at DESC, id DESC);

COMMENT ON TABLE coverage_snapshots IS 'How many Commander-legal cards play automatically, stored by `mtg-cli coverage --snapshot` when it changed.';
COMMENT ON COLUMN coverage_snapshots.missing IS 'The cards that do not play automatically, with their first blocker; only the newest two snapshots keep it.';
COMMENT ON COLUMN coverage_snapshots.newly_supported IS 'Names of the cards that play automatically now and did not in the snapshot before, at most 1,000.';
