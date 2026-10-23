-- Whether the table plays with manual cards: some seat's deck holds cards the game does not run
-- whole, which their controllers do by hand. It is stored when the game starts, with the decks the
-- seats chose, so a resumed game keeps it. A game stored before this column existed reads false.
ALTER TABLE games ADD COLUMN manual_play boolean NOT NULL DEFAULT false;
