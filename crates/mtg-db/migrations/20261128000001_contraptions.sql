-- A deck keeps its Contraption deck as a zone.
ALTER TABLE deck_cards DROP CONSTRAINT deck_cards_zone_check;
ALTER TABLE deck_cards ADD CONSTRAINT deck_cards_zone_check
    CHECK (zone IN ('main', 'sideboard', 'commander', 'companion', 'attractions', 'contraptions', 'stickers'));
