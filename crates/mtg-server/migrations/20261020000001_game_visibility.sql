-- Who may find a game, which of its seats a join may take, and the join code.
--
-- `visibility`: an `invite` game is reached only through its seat links and its join code, a
-- `public` one is listed as well. Games that exist already stay `invite`.
-- `code_digest`: the SHA-256 of the join code, never the code. Games made before codes existed have none.
ALTER TABLE games
    ADD COLUMN visibility  text  NOT NULL DEFAULT 'invite' CHECK (visibility IN ('public', 'invite')),
    ADD COLUMN code_digest bytea CHECK (octet_length(code_digest) = 32);

-- No two games that can still be joined share a code; a finished game's code may come up again.
CREATE UNIQUE INDEX games_live_code ON games (code_digest)
    WHERE code_digest IS NOT NULL AND status IN ('lobby', 'running');

-- A seat is `issued` (its token was made with the game: the seat link), `open` (set aside for a
-- join, no token yet) or `claimed` (a join took it and was given a token). An open seat has no
-- token and no account; its name is empty.
ALTER TABLE seats
    ADD COLUMN state text NOT NULL DEFAULT 'issued' CHECK (state IN ('issued', 'open', 'claimed')),
    ALTER COLUMN token_digest DROP NOT NULL,
    ADD CONSTRAINT seats_open_has_no_token CHECK ((state = 'open') = (token_digest IS NULL)),
    ADD CONSTRAINT seats_open_has_no_user CHECK (state <> 'open' OR user_id IS NULL);
