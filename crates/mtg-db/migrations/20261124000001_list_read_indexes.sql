-- Indexes for the reads every deck list and precon list makes: a card's default printing (paper before digital, then
-- the newest) is one index probe instead of a sort of all its printings, and the newest change of the cards, the
-- printings and the stored rules is read from an index instead of a scan of each table.
CREATE INDEX printings_default_idx ON printings (oracle_id, digital, released_at DESC NULLS LAST, printing_id);
CREATE INDEX cards_updated_idx ON cards (updated_at);
CREATE INDEX printings_updated_idx ON printings (updated_at);
CREATE INDEX card_rules_updated_idx ON card_rules (updated_at);
