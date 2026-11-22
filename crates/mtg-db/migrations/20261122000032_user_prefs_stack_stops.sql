-- Which objects on top of the stack an account's seats are asked at: 'All', 'Opponents' or 'Aimed' (StackStops).
ALTER TABLE user_prefs
    ADD COLUMN stack_stops text NOT NULL DEFAULT 'All';
