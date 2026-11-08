-- The abilities an account's seats always yield to: a JSON array of the abilities' names and words.
ALTER TABLE user_prefs
    ADD COLUMN auto_yields jsonb NOT NULL DEFAULT '[]'::jsonb;
