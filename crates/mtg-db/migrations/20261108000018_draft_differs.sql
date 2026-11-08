-- Whether a draft reads back as other text in some clause: such a draft runs on a player's read alone, never on trust.
ALTER TABLE card_drafts ADD COLUMN differs boolean NOT NULL DEFAULT false;
COMMENT ON COLUMN card_drafts.differs IS 'The draft reads back as other text than the card in some clause; it needs a player''s read even from source `forge`.';
