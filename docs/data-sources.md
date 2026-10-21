# Data sources

Reference for the external data `mtg-import` reads and how `mtg-cli` fetches it: Scryfall and MTGJSON bulk files, mana cost strings, and the deck export formats of Moxfield, Archidekt, Arena and MTGO. Everything here was checked against live data or real exports.

## Bulk data

- **Scryfall** (`GET https://api.scryfall.com/bulk-data`; send `User-Agent` and `Accept`): the files are gzipped JSONL (`jsonl_download_uri`). `oracle_cards` is the card catalogue and `rulings` the rulings. `mtg_import::scryfall::{cards_jsonl, rulings_jsonl}` parse them behind a gzip decoder; `cards` and `rulings` read the JSON-array form of older dumps.
- **MTGJSON**: `https://mtgjson.com/api/v5/DeckList.json` indexes about 3,100 decks of many types (`Commander Deck`, `Starter Deck`, `Planeswalker Deck`, ...). A deck file is `https://mtgjson.com/api/v5/decks/<fileName>.json`.
- Quirks the importer handles: MTGJSON lists reversible cards as `A // A` where Scryfall names them `A`, and memorabilia `front_card` placeholders share names with real cards but are not deck cards.

## Using mtg-cli

`mtg-cli` downloads the bulk files into a cache and loads them into Postgres. `DATABASE_URL` selects the database (`cargo run` applies the default of `.cargo/config.toml`); `--cache <dir>` or `MTG_CACHE_DIR` selects the cache, which defaults to `$XDG_CACHE_HOME/mtg-cli` or `~/.cache/mtg-cli`. Progress and warnings go to stderr, one summary line to stdout.

```sh
docker compose up -d db
cargo run -p mtg-cli -- fetch scryfall   # Oracle Cards and rulings, about 30 MiB
cargo run -p mtg-cli -- fetch decks      # the 198 Commander precons, about 100 MiB
cargo run -p mtg-cli -- ingest           # applies the migrations, then loads the cache
cargo run -p mtg-cli -- status           # cache, row counts, latest import runs
cargo run -p mtg-cli -- coverage         # compiler and engine coverage of the precons, ranked blockers (--deck, --top)
```

- **`fetch scryfall`** reads the `bulk-data` listing and downloads `oracle_cards` and `rulings` (`jsonl_download_uri`, else `download_uri`). A file is skipped while the manifest holds it under the listing's `updated_at`; `--force` downloads anyway.
  - The file is written to `<name>.part`, resumed there with a `Range` request, checked (gzip integrity and the announced `compressed_size`), hashed with SHA-256 and renamed. The manifest (`manifest.json` in the cache) records `updated_at`, size, checksum and address of each complete file.
- **`fetch decks`** saves `DeckList.json` and the decks it lists. The default is the `Commander Deck` type; `--type <TYPE>` (repeatable, `all` for every type), `--only <NAME>` (a part of the deck name, or its file name; repeatable) and `--limit <N>` (the newest N) narrow or widen the choice.
  - A cached deck is asked about only when the index build differs from the one that last confirmed it, and then with `If-None-Match`. A deck the server lacks is reported and skipped; any other failure stops the run and keeps what was fetched.
- **Politeness:** every request sends `User-Agent: mtg-cli/<version>` and an `Accept` header, and starts at least 100 ms after the previous one ended. Connection failures, timeouts, 408, 425, 429 and 5xx are tried three times with waits of 2 s and 4 s, or the server's `Retry-After` (at most 60 s); other statuses end the request.
- **`ingest`** streams the cached files through `mtg-import` and upserts them in batches (`--batch-size`, default 500) while a reader thread parses ahead. Each import run stores the SHA-256 of what it read in `import_runs.input`.
  - The latest successful run of a source with the same input makes the next ingest skip it; `--force` loads it anyway and changes no row. Rulings and decks include the cards input in their identity, so new cards bring them back in.
  - A failed run keeps its finished batches and the rerun upserts the rest. A record Postgres cannot store (a bad id or date, a NUL byte in any text) is skipped, counted and warned about; a run left `running` for over an hour is marked failed when the next import starts.
  - `--dry-run` reads and checks the cached files with the importers' record rules, connects to no database and writes nothing.

Real-data tests are ignored by default; they need downloaded files:

```sh
mkdir -p /tmp/mtgdata && cd /tmp/mtgdata
# download oracle-cards-*.jsonl.gz, rulings-*.jsonl.gz, DeckList.json and some decks/<fileName>.json
MTG_REAL_DATA_DIR=/tmp/mtgdata cargo test -p mtg-import real_data:: -- --ignored
```

## Mana cost strings

Figures come from the oracle-cards file (38,753 cards).

- `mana_cost` is written per face (`{2}{W/P}{G/U/P}`). The card-level text of a split or adventure card joins its faces as `{2}{B} // {B}` (`ManaCost::parse_faces`); a transform or modal card has none there, so `CardRecord.mana_cost` falls back to the front face.
- Scryfall prints symbols in no fixed order (`{1}{G}{G/U}{U}`, `{1}{R}{W}`), so `ManaCost` sorts them.
- The file uses 61 distinct symbols. `ManaCost::parse` reads 54:
  - the numbers 0 to 16 and 1000000 (Gleemax), `{W}{U}{B}{R}{G}` and `{C}`;
  - the monocolored hybrids `{2/W}` to `{2/G}` and the ten colour pairs in the orientation cards print (`{W/U}`, `{U/B}`, `{B/R}`, `{R/G}`, `{G/W}`, `{W/B}`, `{U/R}`, `{B/G}`, `{R/W}`, `{G/U}`), which `ManaCost` keeps;
  - the Phyrexian symbols (`{W/P}` to `{G/P}`, and `{C/P}` on Kozilek, Compleated and That Which Was Compleated) and four hybrid Phyrexian ones (`{G/U/P}`, `{G/W/P}`, `{R/G/P}`, `{R/W/P}`);
  - the colorless hybrids `{C/W}` to `{C/G}` (MH3).
- The other seven (`{X}`, `{Y}`, `{Z}`, `{S}`, `{HW}`, `{D}`, `{L}`) are refused by name as `Unsupported`, never skipped. X is a roadmap item; the rest are silver-border or playtest symbols.
- The importer keeps the text. `CardFace::cost()` is the step from it to a cost: `None` for a face with no cost, `Some` for the rest (`{0}` included), or an error naming an unsupported symbol.
- `crates/mtg-import/tests/fixtures/scryfall_mana_costs.tsv` holds every distinct string of the file: 864 single-faced costs with Scryfall's `cmc` and colours, 211 face costs and 297 `//` texts. It takes the majority value per string because a few joke cards disagree with their own cost.
- `tests/it/mana_costs.rs` checks that the mana value and colours of every cost that parses equal Scryfall's, and that a refusal names one of the seven symbols. The 290 `manaCost` strings of 56 MTGJSON precon decks are all among the fixture's strings.

## Deck export fixtures

- `crates/mtg-import/tests/fixtures/real/<site>/` holds verbatim exports of two Commander decks (cEDH Najeela, Warhammer 40K Tyranids); never reformat them. `tests/it/real_exports.rs` pins detection, section counts and exact entries. The Arena and MTGO files are Moxfield's export-menu output; the Archidekt lists were edited separately.
- `crates/mtg-import/tests/fixtures/corpus/` holds 29 public decks (16 Moxfield, 13 Archidekt) as 183 export files, each deck with the card lists its own site reports; its README gives the provenance.
  - `tests/it/corpus_parse.rs` parses every export with the format a user would name and compares the cards with the site's lists.
  - `corpus_detect.rs` pins what `detect` can and cannot know.
  - `corpus_export.rs` writes every parsed deck out and reads it back, as text, as an Arena export and as MTGO text.
- The ignored test in `real_exports.rs` checks that every name of the eight real files, except the flavor name `The Ball`, resolves against Scryfall (needs `MTG_REAL_DATA_DIR`).

## Deck export formats

- **Moxfield text** (`Moxfield`): `N Name (SET) num *F*|*E* #tag`, a blank line, `SIDEBOARD:`, the sideboard, then `STICKERS:` when the deck has stickers.
  - Commanders (and an oathbreaker's signature spell) are the first lines, unmarked and out of alphabetical order, so `parse` keeps them in Main.
  - `#tag` suffixes may hold spaces, parentheses and `!` (`#Card Advantage #!Wheel`, `#Lands (LOCKED)`) and are dropped. Multi-face cards are `A / B`.
  - Detected by `*F*`/`*E*`, or an all-caps header together with printings.
- **Archidekt text** (`Archidekt`): `Nx Name (set) num *F*|*E* [Category{flags},Category] ^Label,#color^`, lowercase sets; `N` without the `x` reads the same.
  - **The first category decides the board** and later ones never change it: the `{top}` flag is the command zone, a category named Sideboard the sideboard (even with `{noDeck}`), any other `{noDeck}` category the maybeboard, the rest the main deck (`[Ramp,Maybeboard{noDeck}{noPrice}]` is Main).
  - Tag labels hold spaces and commas. An oathbreaker's planeswalker and signature spell both sit in the Commander category. Some exports write only the front face of multi-face cards.
  - Optional headers are generic roles (`Commander`, `Mainboard`, `Sideboard`, `Maybeboard`), category names or card types (`5: Lands`, `🟢 Main Deck`). A line that cannot be an entry, alone above a block of entries, is a header and never an error. Without brackets on the lines the generic role headers decide the board; with brackets the first category does.
- **Arena** (`Arena`, as Moxfield writes it; no export from the Arena client itself has been checked): an `About`/`Name` preamble (emoji and brackets allowed), then `Commander`, `Deck` and `Sideboard` headers and no printings, optionally with `#tag` suffixes.
  - The commander is under its header. Multi-face cards are written by their front face, split cards `A /// B` (`A // B` in one deck).
  - **The export is lossy**: it drops cards that are not on Arena (Najeela 75 of 109 cards, Tyranids 72 of 131 and its commander). That is the site, not a parser bug; the tests pin the counts and require the parsed cards to be a subset of the deck.
- **MTGO text** (`MtgoText`, Moxfield's MTGO menu; not the XML `.dek`): `N Name` lines without printings, a blank line, `SIDEBOARD:`, the sideboard, a blank line, then a headerless block that is the commander.
  - **That trailing block is the command zone with or without a sideboard**, for one, two or more commanders, so MTGO text needs no promotion. `STICKERS:` may come before it. Split cards are `A/B`, other multi-face cards the front face.
  - Detected by an all-caps header with no printing anywhere. A text without such a header is plain `N Name` lines, and plain text reads the trailing block as the sideboard.
- Collector numbers are free text (`1492★`, `187p`, `2023-8`, `NPH-123`, `TMP-315`, `2026-2`, `2016`, `we313`, `tvdl345`) and set codes may start with a digit or hold five characters (`40K`, `PAL00`). Flags, categories and tags never reach names; `_____ Goblin` is a card.
- **Stickers are not deck cards.** Moxfield lists its sticker sheets (set `SUNF`) in a `STICKERS:` section. The parser keeps them out of every section and reports `ParsedDeck::ignored`, one `IgnoredSection { name, count }` per section name (as written, count = copies), never a line error. `export` does not write them. Only `STICKERS` is known: other all-caps lines such as `LANDS:` stay what they were, because plain lists use them as group labels.

## Writers

`decklist::export_as(format, deck)` writes plain text (`export`), the Arena export or MTGO text, and returns text only after reading it back through the parser of that dialect as the deck the format can hold.

- **Arena**: `Commander`, `Companion`, `Deck` and `Sideboard` sections in that order, printings kept, no maybeboard.
- **MTGO text**: names only, the main deck, a `SIDEBOARD:` block (a companion goes there, since MTGO has no companion zone) and the commanders in a last block with no header. A list of commanders alone cannot be written, because the block after the main deck is what marks the command zone; the writer refuses it. Plain-text detection reads a list without the `SIDEBOARD:` header as plain text, so MTGO text is read back with the format named.

## Commander promotion and detection

- Commander imports call `decklist::promote_implied_commander(format, deck)`, which only moves Moxfield's first line (a single copy, with no commander marked).
  - Moxfield's text does not say how many lines are commanders: for a partner pair or a Background it moves the first and leaves the second in Main, so the deck-builder UI has to fix it. The tests pin both outcomes.
  - Signature spells are deck cards and stay in Main.
  - A Moxfield list with no foil or etched card and no sideboard or stickers reads as plain text and is not promoted, so the endpoint should let the user name the site.
- `detect` is decisive for Moxfield text with a foil or etched flag or an all-caps header, MTGO text with an all-caps header, every Arena export with an `About` block and Archidekt text with brackets.
  - It cannot know, and falls back to plain text, for Moxfield lists with neither flag nor header (a `full-nofinish` export without sideboard), MTGO text without header, and Archidekt lists without brackets (`names-only`, `sets-only`; with a foil flag the latter reads as Moxfield, whose flag syntax it shares).
  - A bracketless Archidekt list that opens with a header word (`headers-generic-plain`) reads as Arena.
  - Every fallback reads the same cards in the same sections except an MTGO commander block, which plain text takes for the sideboard.

## Names the resolver must map

The parser leaves these as written:

- Moxfield's flavor names: `The Ball` is Fellwar Stone (SLD 1040), `Rick, Steadfast Leader` is Greymond, Avacyn's Stalwart (SLD 143), `Skittering Kitten` is Masked Meower (OM1 96).
- The digital names Arena and MTGO exports give Universes Beyond cards (`Phenomena Recorder` is Peter Parker's Camera).
- The face notations above, and front faces alone.

The tests learn these from each deck's own data (printings shared with the site's list, leftovers by elimination), not from a list.
