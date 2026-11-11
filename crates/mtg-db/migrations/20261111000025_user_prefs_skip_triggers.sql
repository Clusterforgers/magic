-- Whether an account's seats are passed for at every triggered ability on top of the stack ("Auto-skip triggers").
ALTER TABLE user_prefs
    ADD COLUMN auto_skip_triggers boolean NOT NULL DEFAULT false;
