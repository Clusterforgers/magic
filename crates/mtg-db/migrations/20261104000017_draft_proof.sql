-- The proof of a partial Forge draft: a mark for every printed clause, `Auto` or `ByHand`; null for a whole draft.
ALTER TABLE card_drafts ADD COLUMN proof jsonb;
COMMENT ON COLUMN card_drafts.proof IS 'The proof of a partial draft (`{"marks":[{"face","span","mark"}]}`) that lets its by-hand lines stand; null for a whole draft.';
