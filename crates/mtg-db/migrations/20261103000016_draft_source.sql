-- Where a draft came from, and whether its Oracle text matched the source's, so the server can trust a Forge script.
ALTER TABLE card_drafts
    ADD COLUMN source text NOT NULL DEFAULT 'sweep',
    ADD COLUMN oracle_matches boolean NOT NULL DEFAULT false;
COMMENT ON COLUMN card_drafts.source IS 'Who wrote the draft: `sweep` (a model proposal) or `forge` (a translation of a Forge script).';
COMMENT ON COLUMN card_drafts.oracle_matches IS 'The source''s Oracle text equals the printed text after normalising; with source `forge` the draft runs without a human read.';
