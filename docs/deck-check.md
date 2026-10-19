# Deck check and the Verified marker

Design of the deck check: when a deck is chosen or built, a check finds the cards the game cannot play without manual steps and highlights them, and a saved deck that passes shows a Verified marker until it changes.

The per-card support lookup (`Library::support` in `mtg-cards`), the report (`mtg_cards::deck::deck_report`), the deck endpoints and the derived marker are implemented (`server.md`, "Cards and decks"); the highlighting and the interface below are design. A card without an implementation counts as `Manual` unless it has no rules text, so the report always says how much of the deck the table resolves by hand.

## Terms

- Card support, per oracle id, is computed from the card's rules file, its imported oracle text and the primitives the engine runs ([cards-dsl.md](cards-dsl.md) owns the definitions):
  - `Verified`: the rules cover the whole text, the text is unchanged, and the engine runs every primitive used. A card with no file and no rules text once reminder text (parentheses) is removed, such as a vanilla creature or a basic land, is `Verified` too.
  - `Partial`: implemented except the listed clauses; each carries a plain reason and the span of oracle text it covers.
  - `Manual`: no implementation; the table resolves the whole text by hand.
  - `Stale`: the oracle text changed since the rules were written (errata). Counts as `Manual` until reviewed.
  - `Unplayable`: the engine cannot host the card at all (digital-only actions, ante, joke-set mechanics).
- Deck state:
  - `Verified`: legal, and every card of every zone is `Verified`, the sideboard included.
  - `Manual play`: legal, with some cards not `Verified`.
  - `Blocked`: illegal, or holds an `Unplayable` card.
  - `Unchecked`: never checked, or changed since.

## The marker

- The marker is derived, never a flag set by hand. A check stores `{deck_hash, coverage_hash, checked_at, summary}` with the deck. The marker shows only while the stored check is clean, the stored `deck_hash` equals the current one, the stored `coverage_hash` equals the one computed from the cards' current support records, and the check of the moment is clean too, so a ban that changes no hash still removes it. A stored check that was not clean never gives the marker; `mtg_cards::deck::marker` derives the state (`verified`, `manual_play`, `blocked`, `unchecked`) and `changed` ("changed since the last check").
- Editing the deck changes the hash, so the marker disappears at once with the note "changed since the last check"; nothing can leave it behind by mistake.
- `deck_hash` is SHA-256 over the sorted `(zone, oracle id, count)` of main, sideboard, commander and companion: the text `mtg-deck-check/deck/v1` and one line `<zone> <oracle id> <count>` per card and zone, ordered by zone name and id, copies in one zone summed, as 64 lower-case hex digits. Printings, the deck's name, notes and format are not in it, so swapping art keeps the marker.
- `coverage_hash` is SHA-256 over the sorted `(oracle id, support fingerprint)` of the deck's distinct cards: the text `mtg-deck-check/coverage/v1` and one line `<oracle id> <fingerprint>` per card. An engine or card-data release that changes one of them drops the marker with the reason ("Sheoldred's implementation changed").
- The check is cheap, so the server re-runs it each time a deck is opened and answers the report next to the stored check. Opening writes nothing: a clean result of the moment brings the marker back when the deck is saved again, and a worse one shows as `blocked` or `manual_play` at once.
- Saving runs the check, so a clean deck is marked at once. `POST /api/decks/check` runs the same function without saving, on a stored deck or an unsaved list (a signed-in account's; the check without an account is an extension).

## The report

`DeckReport { deck_hash, coverage_hash, format, legality: [Issue], cards: [{oracle_id, name, zone, count, support, reasons: [{face, span, text}]}], summary }`; `summary` is `{format, state, counts, zones, issues}` with copies by support state (`total`, `verified`, `partial`, `manual`, `stale`, `unplayable`) overall and per zone, and `state` one of `verified`, `manual_play`, `blocked` (a report is never `unchecked`).

- One pure function in `mtg-cards` (`deck_report(format, lines, cards, library, caps)` in the `deck` module) over the deck, the facts of its cards and the rules library serves the deck endpoints, the CLI and tests.
- Legality is its own section (`legality`, a list of `Issue {code, message, zone, oracle_id, name}`), so "illegal" and "not implemented" never blur. The rules:
  - `freeform`: at least one card, nothing else.
  - Constructed formats (`standard`, `pioneer`, `modern`, `legacy`, `vintage`, `pauper`, `historic`, `timeless`, `alchemy`, `penny`, `premodern`, `oldschool`): 60 cards or more in the main deck, at most 15 in the sideboard, no command zone, at most one companion, at most 4 copies of a card across main, sideboard and companion (1 where the format restricts it), every card legal or restricted in the format.
  - `commander`: one commander, or two that can be partnered (Partner, Partner with, Friends forever, Choose a Background, Doctor's companion); a commander is a legendary creature (or a Vehicle or Spacecraft with a power) or says it can be the commander; the main deck and the command zone make exactly 100 cards, one copy of each; every card of the main deck and the companion lies within the commanders' colour identity; every card of every zone is legal in the format; at most one companion. The sideboard is checked for legality only.
  - Basic lands are exempt from copy limits; a card that says "a deck can have any number" or "up to N" follows its text.
  - A card the database lacks is an `unknown_card` issue and counts as `unplayable`; a format without rules is an `unknown_format` issue.
- Cards are grouped by zone (main, sideboard, commander, companion), each by name, so the owner sees where an issue sits; the sideboard and the companion count toward the state like any zone.

## Highlighting

- Deck list and builder: every card that is not `Verified` gets an icon, a word (Manual, Partial, Stale, Unplayable) and a coloured edge, never colour alone. The header counts them ("96 of 100 verified, 3 manual, 1 partial") with an "issues only" filter and a sort that puts them first.
- Card preview: the oracle text with the unsupported clauses underlined and the reason beside it (needs spans from the card data).
- Game: a card in play with manual text carries a small "manual" badge for every seat, and its prompt quotes only the unsupported clause.
- Lobby and precon browser: each seat's deck and each precon shows its state, so the coverage target (100% of the chosen precons) is a visible list.

## Extensions

In order of value:

1. Live counter while building: the check is a pure lookup, so "N issues" updates as cards are added or removed.
2. Clause-level highlighting with plain reasons (above).
3. Smoke test: run the deck through bot games in the engine (the testkit harness) and cache the result by `deck_hash`. "Tested in 20 bot games" backs the static label and catches crashes in card combinations.
4. Swap suggestions: for a flagged card, verified cards of the same type, mana value and colour identity, one click to swap.
5. Demand-ranked backlog: every flagged card in every saved deck counts toward the coverage dashboard, so implementation follows what the table plays.
6. Table setting "verified decks only" for friends who want no manual steps; the default allows manual play with a visible note to all seats.
7. History: the last few checks per deck (date, engine version, result), which explains why a marker went away.
8. Paste-a-list check without an account, and `mtg-cli check <file>` for the same report.

## Rollout

- Decks table and REST, the report function and endpoint, the builder panel with highlighting, the marker and precon badges.
- The coverage dashboard ranks cards by deck demand; Stale detection comes from import drift (a changed oracle fingerprint).
- Then the smoke test, swap suggestions and the table setting.

## Decisions

1. Saving runs the check automatically; the button stays for unsaved decks and pasted lists.
2. A deck with manual cards may be played, with a note to everyone at the table. The note is a count ("Bo's deck has 3 manual cards"), never the names, because names would reveal the deck. A card's name, badge and the clause to resolve appear when it becomes public (cast or in play).
3. The sideboard counts toward the marker: one manual card there keeps the deck unmarked. The report still shows it under its zone.
