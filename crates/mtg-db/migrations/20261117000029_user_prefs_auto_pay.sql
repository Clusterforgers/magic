-- Whether an account's seats pay every mana cost the automatic payment can make at once ("Always pay automatically").
ALTER TABLE user_prefs
    ADD COLUMN auto_pay boolean NOT NULL DEFAULT false;
