-- Whether an account's seats start every game in Full Control held ("Held until I turn it off").
ALTER TABLE user_prefs
    ADD COLUMN full_control_held boolean NOT NULL DEFAULT false;
