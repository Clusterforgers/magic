-- Whether an account's seats have smart stops: a step where one of their permanents can answer becomes a window.
ALTER TABLE user_prefs
    ADD COLUMN auto_smart_stops boolean NOT NULL DEFAULT false;
