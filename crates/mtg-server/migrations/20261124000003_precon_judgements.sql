-- The precon list as the server last judged it, kept across starts: one row, the fingerprint of the cards, precons,
-- stored rules and server build it was judged under, and the list items as JSON.
CREATE TABLE precon_judgements (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    fingerprint TEXT NOT NULL,
    items TEXT NOT NULL,
    judged_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
