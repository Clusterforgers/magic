-- An admin can end any game: `aborted` keeps no result, and `end_reason` holds what the admin wrote.
ALTER TABLE games DROP CONSTRAINT games_status_check;
ALTER TABLE games ADD CONSTRAINT games_status_check
    CHECK (status IN ('lobby', 'running', 'finished', 'expired', 'failed', 'aborted'));
ALTER TABLE games ADD COLUMN end_reason text;
