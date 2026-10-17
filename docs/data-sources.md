# Data sources

Verified against live data on 2026-10-17.

- **Scryfall bulk** (`GET https://api.scryfall.com/bulk-data`, send `User-Agent` and `Accept`): files are now **gzipped JSONL** (`jsonl_download_uri`). Use `oracle_cards` for the card catalogue and `rulings`. Parse with `mtg_import::scryfall::{cards_jsonl, rulings_jsonl}` behind a gzip decoder; the JSON-array readers remain for older dumps.
- **MTGJSON** `https://mtgjson.com/api/v5/DeckList.json` indexes ~3,100 decks of many types (`Commander Deck`, `Starter Deck`, `Planeswalker Deck`, ...). A deck file is `https://mtgjson.com/api/v5/decks/<fileName>.json`.
- Real-data quirks handled: reversible cards are listed by MTGJSON as `A // A` but Scryfall names them `A`; memorabilia `front_card` placeholders share names with real cards and are not deck cards.

## Running the real-data tests

```sh
mkdir -p /tmp/mtgdata && cd /tmp/mtgdata
# download oracle-cards-*.jsonl.gz, rulings-*.jsonl.gz, DeckList.json and some decks/<fileName>.json
MTG_REAL_DATA_DIR=/tmp/mtgdata cargo test -p mtg-import --test real_data -- --ignored
```

## Deck export formats (checked against real exports)

Verbatim exports of two Commander decks (cEDH Najeela, Warhammer 40K Tyranids) live in `crates/mtg-import/tests/fixtures/real/<site>/`; never reformat them. `tests/real_exports.rs` pins detection, section counts and exact entries. The Arena and MTGO files are Moxfield's export-menu output, the Archidekt lists were edited separately.

- **Moxfield text** (`Moxfield`): `N Name (SET) num *F*`, blank line, `SIDEBOARD:`, sideboard. The commander is the first line, unmarked and out of alphabetical order, so `parse` keeps it in Main. Detected by `*F*`/`*E*`, or an all-caps header together with printings.
- **Archidekt text** (`Archidekt`): `Nx Name (set) num *F*|*E* [Category] ^Label,#color^`, lowercase sets, no sideboard in the samples. The commander is the category `[Commander{top}]`.
- **Arena** (`Arena`, as Moxfield writes it; no real Arena client export yet): `About`/`Name` preamble (emoji allowed), then `Commander`, `Deck`, `Sideboard` headers and no printings. The commander is under its header.
- **MTGO text** (`MtgoText`, Moxfield's MTGO menu; not the XML `.dek`): `N Name` lines without printings, blank line, `SIDEBOARD:`, sideboard, blank line, then a headerless block that is the commander. Detected by an all-caps header with no printing anywhere.
- Collector numbers are free text (`1492★`, `187p`, `2023-8`, `NPH-123`) and set codes may start with a digit or hold five characters (`40K`, `PAL00`). Flags, categories and tags never reach names.
- **Arena's export is lossy**: it drops cards that are not on Arena (Najeela 75 of 109 cards, Tyranids 72 of 131 and its commander). That is the site, not a parser bug; the tests pin the counts.
- Commander imports call `decklist::promote_implied_commander(format, deck)`, which only moves Moxfield's first line; partner pairs are left to the deck-builder UI. A Moxfield list with no foil or etched card and no sideboard reads as plain text and is not promoted, so the endpoint should let the user name the site.
- Moxfield writes Secret Lair cards by flavor name (`The Ball` is Fellwar Stone, SLD 1040), which the name index cannot resolve; every other name of the eight files does (ignored test in `real_exports.rs`, same `MTG_REAL_DATA_DIR`).
