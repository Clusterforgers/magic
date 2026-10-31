# Deck check and the Verified marker

Design of the deck check: when a deck is chosen or built, a check finds the cards the game plays by hand and highlights them, and a saved deck that passes shows a Verified marker until it changes.

The per-card support lookup (`Library::support` in `mtg-cards`), the report (`mtg_cards::deck::deck_report`), the deck endpoints and the derived marker are implemented (`server.md`, "Cards and decks"); the highlighting in the builder and the list, the card preview and the live check are implemented (`web.md`); the table note, a seat's deck name, format and count of manual cards in the lobby roster, is implemented in the server (`server.md`, "Choosing a deck"), and so is each precon's state, judged by the same report, and its check at a table without an account (`server.md`, "Precons"); their display in the lobby and the precon browser is design. A card without an implementation counts as `Manual` unless it has no rules text, so the report always says how much of the deck the table resolves by hand.

## Terms

- Card support, per oracle id, is computed from the card's rules file, its imported oracle text and the primitives the engine runs ([cards-dsl.md](cards-dsl.md) owns the definitions):
  - `Verified`: the rules cover the whole text, the text is unchanged, and the engine runs every primitive used. A card with no file and no rules text once reminder text (parentheses) is removed, such as a vanilla creature or a basic land, is `Verified` too.
  - `Read`: a whole draft the checker could not prove, read and found correct by a player; it runs like `Verified` and is counted apart ([cards-dsl.md](cards-dsl.md), Read tier).
  - `Partial`: implemented except the listed clauses; each carries a plain reason and the span of oracle text it covers.
  - `Manual`: no implementation; the table resolves the whole text by hand.
  - `Stale`: the oracle text changed since the rules were written (errata). Counts as `Manual` until reviewed.
  - `Unplayable`: the engine cannot host the card at all (digital-only actions, ante, joke-set mechanics).
- Deck state:
  - `Verified`: legal, and every card of every zone is `Verified`, the sideboard included.
  - `Runs on reads` (`read`): legal, and every card is `Verified` or `Read`, at least one `Read`.
  - `Played by hand` (`manual_play`): legal, with some cards neither `Verified` nor `Read`.
- To read: a card whose current draft no player has judged (`pending`) is played by hand and counted in `summary.to_read` (distinct cards); each such card of the report carries `draft: pending`, and a card with a wrong read `draft: wrong`.
  - `Blocked`: illegal, or holds an `Unplayable` card.
  - `Unchecked`: never checked, or changed since.

## The marker

- The marker is derived, never a flag set by hand. A check stores `{deck_hash, coverage_hash, checked_at, summary}` with the deck. The marker shows only while the stored check is clean, the stored `deck_hash` equals the current one, the stored `coverage_hash` equals the one computed from the cards' current support records, and the check of the moment is clean too, so a ban that changes no hash still removes it. A stored check that was not clean never gives the marker; `mtg_cards::deck::marker` derives the state (`verified`, `manual_play`, `blocked`, `unchecked`) and `changed` ("changed since the last check").
- Editing the deck changes the hash, so the marker disappears at once with the note "changed since the last check"; nothing can leave it behind by mistake.
- `deck_hash` is SHA-256 over the sorted `(zone, oracle id, count)` of main, sideboard, commander and companion: the text `mtg-deck-check/deck/v1` and one line `<zone> <oracle id> <count>` per card and zone, ordered by zone name and id, copies in one zone summed, as 64 lower-case hex digits. Printings, the deck's name, notes and format are not in it, so swapping art keeps the marker.
- `coverage_hash` is SHA-256 over the sorted `(oracle id, support fingerprint)` of the deck's distinct cards: the text `mtg-deck-check/coverage/v1` and one line `<oracle id> <fingerprint>` per card. An engine or card-data release that changes one of them drops the marker with the reason ("Sheoldred's implementation changed").
- The check is cheap, so the server re-runs it each time a deck is opened and answers the report next to the stored check. Opening writes nothing: a clean result of the moment brings the marker back when the deck is saved again, and a worse one shows as `blocked` or `manual_play` at once.
- Saving runs the check, so a clean deck is marked at once. `POST /api/decks/check` runs the same function without saving, on a stored deck or an unsaved list (a signed-in account's) or on a precon (`precon:<code>`, no account needed); a pasted list without an account is an extension.

## The report

`DeckReport { deck_hash, coverage_hash, format, legality: [Issue], cards: [{oracle_id, name, zone, count, support, reasons: [{face, span, text}]}], summary }`; `summary` is `{format, state, counts, zones, issues}` with copies by support state (`total`, `verified`, `partial`, `manual`, `stale`, `unplayable`) overall and per zone, and `state` one of `verified`, `manual_play`, `blocked` (a report is never `unchecked`).

- One pure function in `mtg-cards` (`deck_report(format, lines, cards, library, caps)` in the `deck` module) over the deck, the facts of its cards and the rules library serves the deck endpoints, the CLI and tests.
- Legality is its own section (`legality`, a list of `Issue {code, message, zone, oracle_id, name}`), so "illegal" and "not implemented" never blur. The rules:
  - `freeform`: at least one card, nothing else.
  - Constructed formats (`standard`, `pioneer`, `modern`, `legacy`, `vintage`, `pauper`, `historic`, `timeless`, `alchemy`, `penny`, `premodern`, `oldschool`): 60 cards or more in the main deck, at most 15 in the sideboard, no command zone, at most one companion, at most 4 copies of a card across main, sideboard and companion (1 where the format restricts it), every card legal or restricted in the format.
  - `commander`: one commander, or two that can be partnered (Partner, Partner with, Friends forever, Choose a Background, Doctor's companion); a commander is a legendary creature (or a Vehicle or Spacecraft with a power) or says it can be the commander; the main deck and the command zone make exactly 100 cards, one copy of each; every card of the main deck and the companion lies within the commanders' colour identity; every card of every zone is legal in the format; at most one companion. The sideboard is checked for legality only.
  - A companion whose card has a companion ability in the rules library (`Keyword::Companion(DeckCondition)`) is judged by its condition over the starting deck, the main deck and the commanders: `deck/companion.rs` builds a `DeckCard` for each from the card's assembled definition and runs the same `DeckCondition::check` as the engine, with 100 as the minimum deck size in Commander and 60 elsewhere. A deck that fails it has a `companion_condition` issue in the companion zone that names the card breaking the condition. A companion whose card has no such rules is not judged: it is a manual card the game does not bring into play.
  - Basic lands are exempt from copy limits; a card that says "a deck can have any number" or "up to N" follows its text.
  - A card the database lacks is an `unknown_card` issue and counts as `unplayable`; a format without rules is an `unknown_format` issue.
- Cards are grouped by zone (main, sideboard, commander, companion), each by name, so the owner sees where an issue sits; the sideboard and the companion count toward the state like any zone.

## The engine's check

A game also checks each seat's deck against its format when it begins (`FormatRules::validate_seat`, `engine.md`, Commander), with the card definitions the engine is given. Commander there asks for the commanders in the command zone (one, or a Partner pair), 100 cards with them, one copy of each card but basic lands and cards that allow more, and every card inside the commanders' colour identity. The identity (`CardDefinition::color_identity`) is derived from the definition: the colour indicator, the cost, the rules text and the basic land types over all faces; for the cards of the corpus it equals Scryfall's `color_identity`, which the report above uses.

A seat with a companion is also checked by `FormatRules::validate_companion` (`engine.md`, Companion); its two errors map to the `companion_condition` code, and the server runs it for the stored deck's companion when the deck is chosen, after the seat's own check (`server.md`, Choosing a deck). A companion card without a companion ability in the rules is not judged and is no companion for the game.

The server runs the engine's check in the game's format when a seat chooses a deck and turns each `DeckError` into an `Issue` of the report's codes: no commander or too many is `commander_count`, a card that cannot lead `commander_eligibility`, two commanders that are not partners `commander_pair`, a size other than 100 `deck_size`, a second copy `copies`, a card outside the identity `color_identity`, a command zone in a Normal game `no_commander_zone`. A deck's own report runs first and its issues are the ones shown; the engine's reasons appear for a deck the report accepts, so the two stay in step except where the table changed a rule or the engine reads less than the card text says. A table's house rules relax the format's deck rules (singleton off or a copy limit, colour identity off, a deck size, two commanders per seat without Partner): the issues of the report that such a rule replaces (`IssueCode::lifted_by`: `copies`, `color_identity`, `deck_size`, `commander_pair`) no longer count at that table, and a deck the report refuses only for them is judged by `validate_seat` under the table's rules (`DeckReport::set_legality` holds the result). The lobby's check for a table (`POST /api/decks/check` with a `game` or a `table`) answers that verdict for a stored deck, an unsaved list or a precon, and a seat that chooses the deck there gets the same. The rest holds at every table: `IssueCode::liftable` is false for an unknown card, a ban, a commander that cannot lead and a command zone in a constructed deck. The engine reads less than the card text says here: it knows "can be your commander" and the kinds of partners (Doctor's companion, Friends forever, Background) only from a card's rules, so a deck whose commander is a planeswalker or such a pair passes the report and is held back by the engine until the card system reads those lines.

## Highlighting

- Deck list and builder: every card that is not `Verified` gets an icon, a word (Manual, Partial, Stale, Unplayable) and a coloured edge, never colour alone. The header counts them ("96 of 100 verified, 4 played by hand", the partial, manual and stale copies added up) with an "issues only" filter and a sort that puts them first.
- Builder summary: the header says "N cards played by hand" (and how many cannot be played), a "Played by hand" list gives each such card with its state, reasons and a flag that marks it important to fix, and the save bar names what stands in the way of saving (legality issues never do).
- Card preview: the oracle text with the unsupported clauses underlined and the reason beside it, and a "Mark important to fix" button for any card (`web.md`, Fix marks).
- Not-validated list: `/decks/unvalidated` (every saved deck) and `/decks/$deckId/unvalidated` (one) list each card that is not `Verified` with its unread clauses, each the quoted span of the card's oracle text beside its plain reason, the decks that hold it, a filter by state and a "Copy as text" button; the cards of most decks come first, so the owner sees what to fix first (`web.md`). The precon details quote the same clauses.
- Game: a card in play with manual text carries a small "manual" badge for every seat, and its prompt quotes only the unsupported clause.
- Lobby and precon browser: each seat's deck and each precon shows its state, so the coverage target (100% of the chosen precons) is a visible list.

## Extensions

Not built yet, in order of value:

1. Smoke test: run the deck through bot games in the engine (the testkit harness) and cache the result by `deck_hash`. "Tested in 20 bot games" backs the static label and catches crashes in card combinations.
2. Swap suggestions: for a flagged card, verified cards of the same type, mana value and colour identity, one click to swap.
3. Demand-ranked backlog: every flagged card in every saved deck counts toward the coverage dashboard, so implementation follows what the table plays. Fix marks are the first signal of it: a player marks a card as important to fix and `mtg-cli fix-marks` and `check-rules --gaps --marks` rank by the marks ([server.md](server.md), Fix marks). The proposal checker repairs the mechanical slips of a model before it judges ([cards-dsl.md](cards-dsl.md), Model-assisted reading), so more proposals reach a verdict.
4. Table setting "verified decks only" for friends who want no cards played by hand; the default allows them with a visible note to all seats.
5. History: the last few checks per deck (date, engine version, result), which explains why a marker went away.
6. Paste-a-list check without an account, and `mtg-cli check <file>` for the same report.

## Rollout

- Decks table and REST, the report function and endpoint, the builder panel with highlighting, the marker and precon badges.
- The coverage dashboard ranks cards by deck demand; Stale detection comes from import drift (a changed oracle fingerprint).
- Then the smoke test, swap suggestions and the table setting.

## Decisions

1. Saving runs the check automatically; the button stays for unsaved decks and pasted lists.
2. A deck with cards played by hand may be played, with a note to everyone at the table. The note is a count ("Bo's deck: 3 cards played by hand"), never the names, because names would reveal the deck. A card's name, badge and the clause to resolve appear when it becomes public (cast or in play).
3. The sideboard counts toward the marker: one manual card there keeps the deck unmarked. The report still shows it under its zone.
4. A precon is checked as `commander` when it has a commander and as `freeform` otherwise, since it is a product and not an entry in a tournament format. Its state is the report of the moment, kept in an index until the cards, the precons, the rules or the engine change; nothing is stored, so it is `verified`, `manual_play` or `blocked` and never `unchecked`. Whether a game takes the deck is a separate flag (`playable`) with its own issues: the report's legality is clean, no card is `unplayable` and the engine accepts the deck in the format it is for (a Commander precon in a Commander game, any other in a Normal game). A card played by hand (`manual`, `partial`) never keeps a deck from being playable, so a Commander precon with manual cards is `manual_play` and playable.
