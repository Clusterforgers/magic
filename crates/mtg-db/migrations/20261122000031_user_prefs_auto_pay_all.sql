-- Whether an account's seats also pay at once with life, sacrifices and other costs the automatic payment makes.
ALTER TABLE user_prefs
    ADD COLUMN auto_pay_all boolean NOT NULL DEFAULT false;
