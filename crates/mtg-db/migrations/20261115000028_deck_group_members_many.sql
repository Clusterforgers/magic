-- A deck may sit in several groups at once: a membership is the pair (deck, group), so the same deck can
-- be filed in as many groups as the owner likes and a deck in none stays ungrouped. Every membership that
-- exists keeps its row. The primary key's index leads with `deck_id`, so it also serves the lookup of one
-- deck's groups; `deck_group_members_group_idx` serves the decks of one group.
ALTER TABLE deck_group_members DROP CONSTRAINT deck_group_members_pkey;
ALTER TABLE deck_group_members ADD PRIMARY KEY (deck_id, group_id);
