-- Other names a printing is known by, which deck lists use in place of the card's name.
ALTER TABLE printings
    ADD COLUMN flavor_name  text CHECK (flavor_name <> ''),
    ADD COLUMN printed_name text CHECK (printed_name <> '');

COMMENT ON COLUMN printings.flavor_name IS 'Name the printing shows in place of the card name (Secret Lair and similar); faces joined with " // ".';
COMMENT ON COLUMN printings.printed_name IS 'Name printed on the printing: the localized name of a foreign printing, or the name of an English digital version.';

-- The Default Cards file (every printing) is its own import, so its runs are told apart from the Oracle Cards runs.
ALTER TYPE import_source ADD VALUE 'scryfall_printings';
