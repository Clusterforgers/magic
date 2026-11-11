-- How the hand dock orders an account's hand in every game: a JSON object of the sort and its order of groups; null
-- reads as the default ("as drawn").
ALTER TABLE user_prefs
    ADD COLUMN hand_sort jsonb CHECK (hand_sort IS NULL OR jsonb_typeof(hand_sort) = 'object');
