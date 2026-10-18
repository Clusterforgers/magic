# Deck check and the Verified marker

Design of the deck check: when a deck is chosen or built, a check finds the cards the game cannot play without manual steps and highlights them, and a saved deck that passes shows a Verified marker until it changes. Status: design only.

It builds on `impl_status` in the card data, stored decks and the builder, and card coverage. A card without an implementation counts as `Manual`, so the report always says how much of the deck the table resolves by hand.

## Terms

- Card support, per oracle id:
  - `Verified`: an implementation exists, the fingerprint of the oracle text it was written for equals the current text, and its tests pass.
  - `Partial`: implemented except the listed clauses; each carries a plain reason and the span of oracle text it covers.
  - `Manual`: no implementation; the table resolves the whole text by hand.
  - `Stale`: implemented, but the oracle text has changed (errata). Counts as `Manual` until reviewed.
  - `Unplayable`: the engine cannot host the card at all (digital-only actions, ante, joke-set mechanics).
- Deck state:
  - `Verified`: legal, and every card of every zone is `Verified`, the sideboard included.
  - `Manual play`: legal, with some cards not `Verified`.
  - `Blocked`: illegal, or holds an `Unplayable` card.
  - `Unchecked`: never checked, or changed since.

## The marker

- The marker is derived, never a flag set by hand. A check stores `{deck_hash, coverage_hash, checked_at, summary}` with the deck. The marker shows only while the stored `deck_hash` equals the current one and the stored `coverage_hash` equals the one computed from the cards' current support records.
- Editing the deck changes the hash, so the marker disappears at once with the note "changed since the last check"; nothing can leave it behind by mistake.
- `deck_hash` is SHA-256 over the sorted `(zone, oracle id, count)` of main, sideboard, commander and companion. Printings, the deck's name and notes are not in it, so swapping art keeps the marker.
- `coverage_hash` is SHA-256 over the sorted `(oracle id, support fingerprint)` of the deck's cards. An engine or card-data release that changes one of them drops the marker with the reason ("Sheoldred's implementation changed").
- The check is cheap, so the server re-runs it when the deck is next opened. A clean result brings the marker back; a worse one tells the owner what changed.
- Saving runs the check, so a clean deck is marked at once. The "Check deck" button runs the same function without saving, on a stored deck, an unsaved one or a pasted list.

## The report

`DeckReport { deck_hash, coverage_hash, format, legality: [Issue], cards: [{oracle_id, name, count, zone, support, reasons: [{text, span}]}], summary }`.

- One pure function in `mtg-cards` (`support` module) over the deck and the support records serves the endpoint `POST /api/decks/check`, the CLI and tests.
- Legality (size, singleton, colour identity, banned list, commander and companion rules) is its own section, so "illegal" and "not implemented" never blur.
- Cards are grouped by zone (main, sideboard, commander, companion), so the owner sees where an issue sits.

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

- Card data carries `impl_status` with reasons, text spans and the oracle fingerprint (the DSL design).
- Decks table and REST, the report function and endpoint, the builder panel with highlighting, the marker and precon badges.
- The coverage run writes `card_support` rows (status, reasons, spans, fingerprint); Stale detection comes from import drift; the dashboard ranks by deck demand.
- Then the smoke test, swap suggestions and the table setting.

## Decisions

1. Saving runs the check automatically; the button stays for unsaved decks and pasted lists.
2. A deck with manual cards may be played, with a note to everyone at the table. The note is a count ("Bo's deck has 3 manual cards"), never the names, because names would reveal the deck. A card's name, badge and the clause to resolve appear when it becomes public (cast or in play).
3. The sideboard counts toward the marker: one manual card there keeps the deck unmarked. The report still shows it under its zone.
