CREATE TABLE rulings (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    oracle_id    uuid NOT NULL REFERENCES cards (oracle_id) ON DELETE CASCADE,
    source       text NOT NULL CHECK (source IN ('wotc', 'scryfall', 'unknown')),
    published_at date,
    comment      text NOT NULL CHECK (comment <> ''),
    -- Rulings have no id of their own; the hash keeps the natural key small and indexable.
    comment_md5  text GENERATED ALWAYS AS (md5(comment)) STORED,
    CONSTRAINT rulings_natural_key UNIQUE NULLS NOT DISTINCT (oracle_id, source, published_at, comment_md5)
);

CREATE INDEX rulings_oracle_idx ON rulings (oracle_id, published_at NULLS LAST, id);
