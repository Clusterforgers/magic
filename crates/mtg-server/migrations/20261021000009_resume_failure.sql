-- What a failed game tells its players, and which of them took it off their list.
--
-- `failure` stays the operator's detail (errors, fingerprints); `failure_reason` is a sentence for
-- players, free of internals. A game marked failed before this column existed has none.
-- `seats.hidden`: the seat's account removed the failed game from its list; the record stays so an
-- operator can still revive the game with every seat bound as it was.
ALTER TABLE games ADD COLUMN failure_reason text;
ALTER TABLE seats ADD COLUMN hidden boolean NOT NULL DEFAULT false;
