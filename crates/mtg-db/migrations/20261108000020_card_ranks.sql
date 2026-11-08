-- The popularity of cards: Scryfall's EDHREC rank of each card that has one, which orders the read queues.
CREATE TABLE card_ranks (
    oracle_id   uuid PRIMARY KEY REFERENCES cards (oracle_id) ON DELETE CASCADE,
    edhrec_rank integer NOT NULL CHECK (edhrec_rank > 0)
);
COMMENT ON TABLE card_ranks IS 'The EDHREC rank of each card that has one (1 is the most played); `mtg-cli ingest` replaces it from the Oracle Cards file.';
