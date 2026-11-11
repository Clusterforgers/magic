-- A deck keeps its Attraction deck and its sticker sheets as zones; a printing keeps the numbers lit up on an Attraction.
ALTER TABLE deck_cards DROP CONSTRAINT deck_cards_zone_check;
ALTER TABLE deck_cards ADD CONSTRAINT deck_cards_zone_check
    CHECK (zone IN ('main', 'sideboard', 'commander', 'companion', 'attractions', 'stickers'));

-- Bit n set for the number n (1 to 6) lit up on the printing (CR 717.1); 0 for any card that is no Attraction.
ALTER TABLE printings
    ADD COLUMN attraction_lights smallint NOT NULL DEFAULT 0 CHECK (attraction_lights BETWEEN 0 AND 126);
