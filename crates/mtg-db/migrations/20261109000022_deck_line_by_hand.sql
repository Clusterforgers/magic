-- Whether the owner plays a deck line's card by hand: the game hosts it as a manual card whatever its rules.
ALTER TABLE deck_cards
    ADD COLUMN by_hand boolean NOT NULL DEFAULT false;
