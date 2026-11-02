-- The decisions an account lets the server answer for its seats: triggers in timestamp order, combat damage lethal in order.
ALTER TABLE user_prefs
    ADD COLUMN auto_order_triggers boolean NOT NULL DEFAULT false,
    ADD COLUMN auto_assign_damage  boolean NOT NULL DEFAULT false;
