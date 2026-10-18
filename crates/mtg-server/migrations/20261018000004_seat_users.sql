-- A seat may belong to an account. The seat's token keeps opening it (guests have only that);
-- the account adds a way back in with the session cookie and puts the game on the user's list.
-- If the account goes, the seat stays and is a guest seat again.
ALTER TABLE seats ADD COLUMN user_id uuid REFERENCES users (id) ON DELETE SET NULL;

-- One account holds at most one seat of a game, so "the user's seat in this game" is one seat.
CREATE UNIQUE INDEX seats_one_per_user ON seats (game_id, user_id) WHERE user_id IS NOT NULL;

-- A user's games, found without reading every seat.
CREATE INDEX seats_by_user ON seats (user_id) WHERE user_id IS NOT NULL;
