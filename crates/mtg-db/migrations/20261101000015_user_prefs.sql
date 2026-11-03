-- Pace preferences of an account: the lasting stops a new game starts with and whether its seats end turns by hand.
-- The server's `users` table is its own, so `user_id` has no foreign key: deleting an account has to delete its row.
CREATE TABLE user_prefs (
    user_id           uuid        PRIMARY KEY,
    default_stops     jsonb       NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(default_stops) = 'array'),
    end_turn_by_hand  boolean     NOT NULL DEFAULT false,
    updated_at        timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE user_prefs IS 'Per-account pace preferences, one row per account; absent means the defaults.';
COMMENT ON COLUMN user_prefs.default_stops IS 'JSON array of {side, step, keep} stops a new game starts with.';
