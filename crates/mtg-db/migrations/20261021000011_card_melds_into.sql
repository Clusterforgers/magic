-- The card a meld piece melds into, which deck lists use to name the piece (`A // B`).
ALTER TABLE cards ADD COLUMN melds_into text CHECK (melds_into <> '');

COMMENT ON COLUMN cards.melds_into IS 'Name of the card this card melds into (Gisela, the Broken Blade: Brisela, Voice of Nightmares); null for every other card.';

-- Cards already stored have no value until the Oracle Cards file is read again, so the next ingest does not take it for done.
UPDATE import_runs SET input = NULL WHERE source = 'scryfall_cards';
