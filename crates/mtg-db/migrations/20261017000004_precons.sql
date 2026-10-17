CREATE TABLE precon_decks (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    code         text NOT NULL CHECK (code <> ''),
    name         text NOT NULL CHECK (name <> ''),
    deck_type    text,
    release_date date,
    CONSTRAINT precon_decks_natural_key UNIQUE (code, name)
);

CREATE INDEX precon_decks_release_idx ON precon_decks (release_date DESC NULLS LAST, name, id);

CREATE TABLE precon_deck_cards (
    deck_id          bigint NOT NULL REFERENCES precon_decks (id) ON DELETE CASCADE,
    position         integer NOT NULL CHECK (position >= 0),
    board            text NOT NULL CHECK (board IN ('commander', 'main', 'sideboard')),
    card_name        text NOT NULL CHECK (card_name <> ''),
    -- NULL keeps the entry visible when the card could not be matched to `cards`.
    oracle_id        uuid REFERENCES cards (oracle_id) ON DELETE SET NULL,
    -- No foreign key: the card bulk file may not contain every printing a deck uses.
    printing_id      uuid,
    set_code         text,
    collector_number text,
    count            integer NOT NULL CHECK (count > 0),
    is_foil          boolean NOT NULL DEFAULT false,
    PRIMARY KEY (deck_id, position)
);

COMMENT ON COLUMN precon_deck_cards.position IS 'Order within the deck: commander entries first, then main, then sideboard.';

CREATE INDEX precon_deck_cards_oracle_idx ON precon_deck_cards (oracle_id);
