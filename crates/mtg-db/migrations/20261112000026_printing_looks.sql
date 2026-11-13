-- What tells one printing's look from another in an art picker: the set's name, the frame, the finishes and full art.
ALTER TABLE printings
    ADD COLUMN set_name text CHECK (set_name <> ''),
    ADD COLUMN frame    text CHECK (frame <> ''),
    ADD COLUMN finishes text[] NOT NULL DEFAULT '{}',
    ADD COLUMN full_art boolean NOT NULL DEFAULT false;

COMMENT ON COLUMN printings.frame IS 'Scryfall frame edition: 1993, 1997, 2003, 2015 or future.';
COMMENT ON COLUMN printings.finishes IS 'Finishes the printing exists in: nonfoil, foil, etched.';

-- The art an account plays a card in when a deck line chooses none.
CREATE TABLE preferred_printings (
    owner       uuid NOT NULL,
    oracle_id   uuid NOT NULL REFERENCES cards (oracle_id) ON DELETE CASCADE,
    printing_id uuid NOT NULL REFERENCES printings (printing_id) ON DELETE CASCADE,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (owner, oracle_id)
);

COMMENT ON COLUMN preferred_printings.owner IS 'Id of the account in the server''s users table, as decks.owner.';
