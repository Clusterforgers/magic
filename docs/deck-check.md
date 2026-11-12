# Deck check and the deck state

Design of the deck check: a check finds the cards the game plays by hand, a deck shows the state of the check of the moment, and the web highlights the cards. Code: `mtg-cards` (`support`, `deck`), `server.md` ("Cards and decks", "Choosing a deck", "Precons"), `web.md`.

A card without an implementation counts as `Manual` unless it has no rules text, so the report always says how much of the deck the table resolves by hand.

## Terms

- Card support, per oracle id, is computed from the card's rules file, its imported oracle text and the primitives the engine runs ([cards-dsl.md](cards-dsl.md) owns the definitions; `Support` in `mtg-cards`):
  - `Verified`: the rules cover the whole text, the text is unchanged, and the engine runs every primitive used. A card with no file and no rules text once reminder text (parentheses) is removed, such as a vanilla creature or a basic land, is `Verified` too.
  - `Read`: a whole draft the checker could not prove, read and found correct by a player; it runs like `Verified` and is counted apart.
  - `Forge`: a trusted whole translation of a Forge script whose Oracle text equals ours; it runs like `Read`, is counted apart and shown as "From Forge", and a `wrong` read ends it.
  - `Unverified`: a whole draft no player has read yet; it runs like `Read`, is counted apart and shown as "Unverified", and a `wrong` read ends it.
  - `Partial`: implemented except the listed clauses; each carries a plain reason and the span of oracle text it covers.
  - `Manual`: no implementation; the table resolves the whole text by hand.
  - `Stale`: the oracle text changed since the rules were written (errata). Counts as `Manual` until reviewed.
  - `Unplayable`: the engine cannot host the card at all (digital-only actions, ante, joke-set mechanics).
- Deck state (`DeckState`) says how the game runs the cards; legality is a separate warning (`summary.legal`) and never keeps a deck from a game:
  - `Verified`: every card of every zone is `Verified`, the sideboard included.
  - `Whole deck runs` (`read`): every card is `Verified`, `Read`, `Forge` or `Unverified`, at least one not `Verified`; a quiet line names the copies from Forge, unverified and read.
  - `Played by hand` (`manual_play`): some cards the game does not run whole (no rules, `Partial`, `Unplayable`, a card the database lacks, a `wrong` read); the table plays them with the manual panel.
  - `Blocked`: a stored value no report produces.
  - `Unchecked`: never checked.
- Not legal (`summary.legal` false): the deck breaks a rule of its format or of the table; the issues are listed as warnings ("Not legal in Commander: 2 banned cards, colour identity") and every play button stays enabled.
- To read: a card whose current draft no player has judged runs as `Unverified` (a whole draft) and is counted in `summary.to_read` (distinct cards, any zone). A draft is `pending` while nobody judged it, and again once it or the card's text changed since the last read. Each card of the report that has such a draft carries `draft: pending`; a card with a wrong read carries `draft: wrong` and an approved one `draft: approved`. A card has no `draft` while a rules file or a `Verified` row outranks the draft, or the engine could not run the draft whole.

## The deck state

- The state is derived, never a flag: every read of a deck (the list and the deck page) runs the check of the moment and shows its state, so a saved clean deck is Verified without being saved again, a deck that got worse shows it at once, and a rule or engine change never reads as "changed". A save stores `{deck_hash, coverage_hash, checked_at, summary}` in `last_check`; nothing compares it with the check of the moment. Only the builder's draft that differs from the saved deck shows "Unsaved draft" (the deck marker), and its counts say "as saved".
- `deck_hash` is SHA-256 over the sorted `(zone, oracle id, count)` of main, sideboard, commander and companion: the text `mtg-deck-check/deck/v1` and one line `<zone> <oracle id> <count>` per card and zone, ordered by zone name and id, copies in one zone summed, as 64 lower-case hex digits. Printings, the deck's name, notes and format are not in it, so swapping art keeps the state.
- `coverage_hash` is SHA-256 over the sorted `(oracle id, support fingerprint)` of the deck's distinct cards: the text `mtg-deck-check/coverage/v1` and one line `<oracle id> <fingerprint>` per card. It identifies a check and the rules it was made under; no state depends on it.
- The check is cheap, so the server re-runs it each time a deck is listed or opened and answers the report next to the stored check. Reading writes nothing; saving runs the check, so a clean deck is marked at once.
- `POST /api/decks/check` runs the same function without saving, on a stored deck or an unsaved list (a signed-in account's) or on a precon (`precon:<code>`, no account needed).

## The report

`DeckReport { deck_hash, coverage_hash, format, legality: [Issue], cards: [{oracle_id, name, zone, count, support, draft?, reasons: [{face, span, text, quote}], whole_text?}], summary }`; `summary` is `{format, state, legal, counts, zones, issues, to_read}` with copies by support state overall and per zone, `state` one of `verified`, `read`, `manual_play` (a report is never `blocked` or `unchecked`) and `legal` false while `legality` holds an issue. A reason's `quote` is the words of its face's text the span covers; `whole_text` is the card's text for a card left to the table whole (no clause of its own), so a client reads no card to quote one.

- One pure function in `mtg-cards` (`deck_report` in the `deck` module) over the deck, the facts of its cards and the rules library serves the deck endpoints, the CLI and tests.
- Legality is its own section (`legality`, a list of `Issue {code, message, zone, oracle_id, name}`), so "illegal" and "not implemented" never blur. Every issue is a warning. The rules:
  - `freeform`: at least one card, nothing else.
  - Constructed formats (`standard`, `pioneer`, `modern`, `legacy`, `vintage`, `pauper`, `historic`, `timeless`, `alchemy`, `penny`, `premodern`, `oldschool`): 60 cards or more in the main deck, at most 15 in the sideboard, no command zone, at most one companion, at most 4 copies of a card across main, sideboard and companion (1 where the format restricts it), every card legal or restricted in the format.
  - `commander`: one commander, or two that can be partnered (Partner, Partner with each other, the same `Partner—` group such as Friends forever, Choose a Background with a Background, Doctor's companion with a Time Lord Doctor), judged by the rule the game uses (`CardDefinition::partners_with`) over the printed sides, whose pairing keywords come from the oracle text (`pairing_keywords`) whatever the card's support; a commander is a legendary creature (or a Vehicle or Spacecraft with a power) or says it can be the commander; the main deck and the command zone make exactly 100 cards, one copy of each; every card of the main deck and the companion lies within the commanders' colour identity; every card of every zone is legal in the format; at most one companion. The sideboard is checked for legality only.
  - A companion whose card has a companion ability in the rules library (`Keyword::Companion(DeckCondition)`) is judged by its condition over the starting deck, the main deck and the commanders: `deck/companion.rs` builds a `DeckCard` for each and runs the same `DeckCondition::check` as the engine, with 100 as the minimum deck size in Commander and 60 elsewhere. A deck that fails it has a `companion_condition` issue in the companion zone that names the card breaking the condition. A companion whose card has no such rules is not judged: it is a manual card the game does not bring into play.
  - In every format, an Attraction deck (the `attractions` zone) holds ten cards or more with different names (CR 717.2a); a smaller one, or a name twice, is an `attraction_deck` warning in that zone.
  - Basic lands are exempt from copy limits; a card that says "a deck can have any number" or "up to N" follows its text.
  - A card the database lacks is an `unknown_card` issue and counts as `unplayable`; the game leaves it out. A format without rules is an `unknown_format` issue.
- Cards are grouped by zone (main, sideboard, commander, companion), each by name; the sideboard and the companion count toward the state like any zone.

## The engine's check

The engine can check each seat's deck against its format at setup (`FormatRules::validate_seat`, `engine.md`, Commander); the server turns that off (`GameSetup::validate_decks`) and uses the check only to name warnings. The identity (`CardDefinition::color_identity`) is derived from the definition: the colour indicator, the cost, the rules text and the basic land types over all faces; for the cards of the corpus it equals Scryfall's `color_identity`, which the report uses.

A seat with a companion is also checked by `FormatRules::validate_companion` (`engine.md`, Companion); its two errors map to the `companion_condition` code, and the server runs it for the stored deck's companion when the deck is chosen, after the seat's own check (`server.md`, Choosing a deck).

The server runs the engine's check in the game's format when a seat chooses a deck and turns each `DeckError` into an `Issue` of the report's codes: `commander_count`, `commander_eligibility`, `commander_pair`, `deck_size`, `copies`, `color_identity`, `no_commander_zone`. A deck's own report runs first and its issues are the ones shown; the engine's reasons appear for a deck the report accepts. A table's house rules relax the format's deck rules: the issues that such a rule replaces (`IssueCode::lifted_by`) no longer count at that table, and a deck the report flags only for them is judged by `validate_seat` under the table's rules (`DeckReport::set_legality` holds the result). The lobby's check for a table (`POST /api/decks/check` with a `game` or a `table`) answers that verdict, and a seat that chooses the deck there is told the same count (`SeatDeck.warnings`). The rest holds at every table: `IssueCode::liftable` is false for an unknown card, a ban, a commander that cannot lead and a command zone in a constructed deck. The engine knows "can be your commander" and the kinds of partners (Doctor's companion, Friends forever, Background) only from a card's rules, so a deck whose commander is a planeswalker or such a pair can pass the report and be flagged by the engine's check.

Only a deck with no card of its main deck or command zone that the database knows is refused (422 `deck_blocked`, issue `empty`): no game can start with it.

## Highlighting

- Every card that is not `Verified` gets an icon, a word (Read, From Forge, Unverified, Manual, Partial, Stale, Unplayable) and a coloured edge, never colour alone. The builder and the deck list count them with an "issues only" filter and a sort that puts them first, and say "N cards to read" where drafts wait. An unverified card links to its read; a card played by hand says whether its draft was found wrong.
- The builder lists each card played by hand with its state, reasons and a flag that marks it important to fix; the save bar names what stands in the way of saving (legality issues never do).
- Card preview: the oracle text with the unsupported clauses underlined and the reason beside it, and a "Mark important to fix" button (`web.md`, Fix marks).
- Not-validated list: `/decks/unvalidated` (every saved deck) and `/decks/$deckId/unvalidated` (one) list each card that is not `Verified` with its unread clauses (the quoted span beside its plain reason), the decks that hold it, a state filter and a "Copy as text" button; the cards of most decks come first. The precon details quote the same clauses.
- Game: a card in play with manual text carries a small "manual" badge for every seat, and its prompt quotes only the unsupported clause.
- Lobby and precon browser: each seat's deck and each precon shows its state ("Whole deck runs", with a quiet breakdown of the copies that are not Verified, "N cards played by hand"), a "Not legal" warning with an icon where the deck breaks a rule (the owner sees the list, the other seats the count) and, where drafts wait, "N cards to read". Every play and take button stays enabled.
- `mtg-cli coverage-decks [--json]` lists, read-only and without account names, every card in any saved deck that the game does not play whole, most used first, with the number of decks and of owners. Fix marks rank cards too (`mtg-cli fix-marks`, `server.md`, Fix marks).

## Decisions

1. Saving runs the check automatically; the button stays for unsaved decks and pasted lists.
2. A deck with cards played by hand may be played, with a note to everyone at the table. The note is a count ("Bo's deck: 3 cards played by hand"), never the names, because names would reveal the deck. A card's name, badge and the clause to resolve appear when it becomes public (cast or in play).
3. The sideboard counts toward the state: one manual card there keeps the deck from Verified. The report still shows it under its zone.
4. A precon is checked as `commander` when it has a commander and as `freeform` otherwise, since it is a product and not an entry in a tournament format. Its state is the report of the moment, kept in an index until the cards, the precons, the rules or the engine change; nothing is stored, so it is `verified`, `read` or `manual_play`. Its warnings are the report's legality, or the engine's check in the format it is for (a Commander precon in a Commander game, any other in a Normal game) when that is clean. `playable` is false only for a precon with no card the database knows.
5. A deck never refuses a card: a banned or off-identity card, a wrong size and a card with no rules are warnings or cards played by hand, and the deck goes to any game. At a Normal table a deck's commanders join its library.
