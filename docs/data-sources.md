# Data sources

The external data `mtg-import` reads and how `mtg-cli` fetches and loads it: Scryfall and MTGJSON bulk files, mana cost strings, deck export formats and name resolution.

## Bulk data

- **Scryfall** (`GET https://api.scryfall.com/bulk-data`, with `User-Agent` and `Accept`): gzipped JSONL files (`jsonl_download_uri`). `oracle_cards` is the card catalogue, `rulings` the rulings, `default_cards` every printing (English, or the printed language when a card has no English printing). `mtg_import::scryfall::{cards_jsonl, rulings_jsonl}` parse them behind a gzip decoder; `cards` and `rulings` read the JSON-array form of older dumps.
- **MTGJSON**: `https://mtgjson.com/api/v5/DeckList.json` indexes the decks of many types (`Commander Deck`, `Starter Deck`, ...); a deck file is `https://mtgjson.com/api/v5/decks/<fileName>.json`.
- The importer handles two quirks: MTGJSON lists reversible cards as `A // A` where Scryfall names them `A`, and memorabilia `front_card` placeholders share names with real cards but are not deck cards.

## Using mtg-cli

`mtg-cli` downloads the bulk files into a cache and loads them into Postgres. `DATABASE_URL` selects the database (`.cargo/config.toml` defaults it to the local one); `--cache <dir>` or `MTG_CACHE_DIR` selects the cache, default `$XDG_CACHE_HOME/mtg-cli` or `~/.cache/mtg-cli`. Progress and warnings go to stderr, one summary line to stdout.

```sh
docker compose up -d db
cargo run -p mtg-cli -- fetch scryfall               # Oracle Cards and rulings
cargo run -p mtg-cli -- fetch scryfall --printings   # and Default Cards, every printing
cargo run -p mtg-cli -- fetch decks                  # the Commander precons
cargo run -p mtg-cli -- ingest                       # migrates, then loads the cache
cargo run -p mtg-cli -- compile --all                # stores the compiled rules as card_rules rows
cargo run -p mtg-cli -- status                       # cache, row counts, latest import runs
```

`migrate` applies the card-table migrations and loads nothing. `--help` lists every subcommand.

- **`fetch scryfall`** downloads `oracle_cards` and `rulings` (`jsonl_download_uri`, else `download_uri`), and `default_cards` with `--printings`. A file is skipped while the manifest holds it under the listing's `updated_at`; `--force` downloads anyway. Every requested entry is checked in the listing before the first download starts. A file is written to `<name>.part`, resumed there with a `Range` request, checked (gzip integrity, announced `compressed_size`), hashed with SHA-256 and renamed; `manifest.json` in the cache records `updated_at`, size, checksum and address of each complete file.
- **`fetch decks`** saves `DeckList.json` and the decks it lists (default type `Commander Deck`; `--type` repeatable, `all` for every type; `--only <NAME>`; `--limit <N>` the newest N). A cached deck is asked about only when the index build differs from the one that last confirmed it, then with `If-None-Match`. A deck the server lacks is reported and skipped; any other failure stops the run and keeps what was fetched.
- **Politeness:** every request sends `User-Agent: mtg-cli/<version>` and an `Accept` header and starts at least 100 ms after the previous one ended. Connection failures, timeouts, 408, 425, 429 and 5xx are tried three times with waits of 2 s and 4 s, or the server's `Retry-After` (at most 60 s); other statuses end the request.
- **`ingest`** streams the cached files through `mtg-import` and upserts them in batches (`--batch-size`) while a reader thread parses ahead. Each import run stores the SHA-256 of what it read in `import_runs.input`.
  - The latest successful run of a source with the same input makes the next ingest skip it; `--force` loads anyway and changes no row. Printings, rulings and decks include the cards input in their identity, so new cards bring them back in; decks also include the printings input.
  - Default Cards is its own import (`scryfall_printings`) after the cards. It writes printings only; the oracle-level data stays what Oracle Cards gave, and a printing whose card is not stored is counted as orphaned, so cards come first. Reversible cards (named `A // A`, no oracle id of their own) add a printing to the card on their faces instead of becoming a card.
  - `printings.flavor_name` and `printed_name` hold what the record carries for any language (faces joined with ` // `). `cards.melds_into` holds the name of the card a meld piece melds into (the `meld_result` among `all_parts`); the Oracle Cards import counts a change as an update, so `CardRepo::index_version` shows it.
  - A failed run keeps its finished batches and the rerun upserts the rest. A record Postgres cannot store (a bad id or date, a NUL byte in text) is skipped, counted and warned about; a run left `running` for over an hour is marked failed when the next import starts.
  - `--dry-run` reads and checks the cached files with the importers' record rules, connects to no database and writes nothing.
- **`compile`** compiles the stored deck cards (the stored precons' cards by default; `--all` for every deck card; `--card <name>` for one, by card or face name) and keeps the rules that pass the checks of a rules file in `card_rules` ([cards-dsl.md](cards-dsl.md)). It reads no cache and writes no card.
  - A row holds the oracle id, the rules as JSON, the status (`full` or `partial`), the fingerprint of the printed data the compiler read and the compiler version. A card whose row has the same fingerprint and version is left alone; any other is compiled again and its row replaced. A card the compiler reads nothing of, or whose rules fail a check, has no row (an old row is removed, and the card is listed as invalid with its first two problems). A card a hand-written file covers has none either; the file is checked as a file of its own against the stored card.
  - Rows of cards no longer stored as deck cards are removed whatever the selection; with a selection no other row is touched. Rows are written in batches and a rerun finishes what a failed run left. The summary counts rows written, replaced, unchanged and removed, and the cards left out.
  - It also stores the drafts the build embeds in `card_drafts` and retires the stored ones it no longer holds; the summary counts them.
- **`status`** shows the cache, row counts and the latest import runs, including how many `card_rules` rows are stored.
- **`coverage`** judges the stored precon cards (`--deck <code or name part>` picks some) and ranks the blockers that hold cards back (`--top <n>` rows per ranking). `--all` takes every stored card legal or restricted in Commander instead and adds the share of verified cards among the most played; `--out <file>` writes the unverified ones among the first 2,000 by rank. Popularity is the Scryfall `edhrec_rank`, read at run time from the cached Oracle Cards file (the database does not store it); a card weighs `1 / (rank + 100)`, a card without a rank weighs as the last rank of the file.
- **`fix-marks`** prints the cards accounts marked as important to fix, most marked first (`--limit N`; [server.md](server.md), Fix marks). **`reads`** prints the read verdicts, newest first (`--wrong`; `--quick` keeps the verdicts an account gave on different cards less than 4 seconds apart, with a count per account). **`draft-suspects`** ranks drafts by how far the engine's English differs from the Oracle text (no model, no database). Both `fix-marks` and `reads` read only the database.

Real-data tests are ignored by default and need downloaded files: `MTG_REAL_DATA_DIR=<dir> cargo test -p mtg-import real_data:: -- --ignored`. The directory holds `oracle-cards-*.jsonl.gz`, `rulings-*.jsonl.gz`, `DeckList.json` and some `decks/<fileName>.json`, or is the `mtg-cli` cache itself; then `cargo test -p mtg-import -- --ignored` runs every real-data test.

The records of `mtg-cards/tests/fixtures/corpus/` are copies of Oracle Cards records, never typed by hand: `cargo test -p mtg-cards fixture_real::` compares each with the cached download by oracle id and names the card and field that differ (numbers by value, so 2 equals 2.0). No test depends on the order of the directory; `mtg_testkit::corpus::scryfall_array` joins the files into the one JSON array the importer streams.

## Tokens and pictures

- Tokens are in Oracle Cards (layouts `token` and `double_faced_token`), each with an oracle id. `ingest` stores them as cards with `is_deck_card` false; Default Cards adds their other printings. The database holds no link from a token to the cards that make it, so the server finds one by name and the facts a client gives ([server.md](server.md), Pictures).
- A printing's `image_uris` (or one set per face for transform, modal and reversible cards) hold `small`, `normal`, `large`, `png`, `art_crop` and `border_crop` links of the form `https://cards.scryfall.io/<size>/<front|back>/<a>/<b>/<id>.<jpg|png>?<time of change>`; the query changes when Scryfall replaces the picture. A card Scryfall has no picture of has no `image_uris`.
- The server fetches pictures from `scryfall.io` hosts only, with the politeness rules above, and shows them as Scryfall serves them: never cropped, re-encoded, resized or marked.

## Mana cost strings

- `mana_cost` is written per face (`{2}{W/P}{G/U/P}`). The card-level text of a split or adventure card joins its faces as `{2}{B} // {B}` (`ManaCost::parse_faces`); a transform or modal card has none there, so `CardRecord.mana_cost` falls back to the front face.
- Scryfall prints symbols in no fixed order, so `ManaCost` sorts them.
- `ManaCost::parse` reads numbers (0 to 16 and 1000000), `{W}{U}{B}{R}{G}{C}`, monocolored hybrids (`{2/W}`), the ten colour-pair hybrids in the orientation cards print, Phyrexian and hybrid Phyrexian symbols, and colorless hybrids (`{C/W}`). `{X}`, `{Y}`, `{Z}`, `{S}`, `{HW}`, `{D}` and `{L}` are refused by name as `Unsupported`, never skipped.
- The importer keeps the text. `CardFace::cost()` is the step from it to a cost: `None` for a face with no cost, `Some` for the rest (`{0}` included), or an error naming an unsupported symbol.
- `crates/mtg-import/tests/fixtures/scryfall_mana_costs.tsv` holds every distinct cost string of the file with Scryfall's `cmc` and colours (the majority value per string, because a few joke cards disagree with their own cost); `tests/it/mana_costs.rs` checks that every cost that parses equals them and that a refusal names one of the unsupported symbols.

## Deck export formats

- **Moxfield text** (`Moxfield`): `N Name (SET) num *F*|*E* #tag`, a blank line, `SIDEBOARD:`, the sideboard, then `STICKERS:` when the deck has stickers. Commanders (and an oathbreaker's signature spell) are the first lines, unmarked and out of alphabetical order, so `parse` keeps them in Main. `#tag` suffixes may hold spaces, parentheses and `!` and are dropped. Multi-face cards are `A / B`. Detected by `*F*`/`*E*`, or an all-caps header together with printings.
- **Archidekt text** (`Archidekt`): `Nx Name (set) num *F*|*E* [Category{flags},Category] ^Label,#color^`, lowercase sets; `N` without the `x` reads the same.
  - **The first category decides the board** and later ones never change it: the `{top}` flag is the command zone, a category named Sideboard the sideboard (even with `{noDeck}`), any other `{noDeck}` category the maybeboard, the rest the main deck.
  - Tag labels hold spaces and commas. An oathbreaker's planeswalker and signature spell both sit in the Commander category. Some exports write only the front face of multi-face cards.
  - Optional headers are generic roles, category names or card types. A line that cannot be an entry, alone above a block of entries, is a header and never an error. Without brackets on the lines the generic role headers decide the board; with brackets the first category does.
- **Arena** (`Arena`, as Moxfield writes it): an `About`/`Name` preamble, then `Commander`, `Deck` and `Sideboard` headers and no printings, optionally with `#tag` suffixes. Multi-face cards are written by their front face, split cards `A /// B`. The export is lossy: it drops cards that are not on Arena, which is the site and not a parser bug; tests require the parsed cards to be a subset of the deck.
- **MTGO text** (`MtgoText`, Moxfield's MTGO menu; not the XML `.dek`): `N Name` lines without printings, a blank line, `SIDEBOARD:`, the sideboard, a blank line, then a headerless block that is the commander. **That trailing block is the command zone with or without a sideboard**, for one or more commanders. `STICKERS:` may come before it. Split cards are `A/B`, other multi-face cards the front face. Detected by an all-caps header with no printing anywhere; a text without one is plain `N Name` lines, and plain text reads the trailing block as the sideboard.
- Collector numbers are free text (`1492★`, `NPH-123`, `2023-8`) and set codes may start with a digit or hold five characters (`40K`, `PAL00`). Flags, categories and tags never reach names; `_____ Goblin` is a card.
- **Stickers are not deck cards.** The parser keeps `STICKERS:` out of every section and reports `ParsedDeck::ignored`, one `IgnoredSection { name, count }` per section name, never a line error. `export` does not write them. Only `STICKERS` is known: other all-caps lines such as `LANDS:` stay what they were, because plain lists use them as group labels.
- Fixtures: `crates/mtg-import/tests/fixtures/real/<site>/` holds verbatim exports of two Commander decks and must never be reformatted; `fixtures/corpus/` holds public decks as export files with a README on provenance. `tests/it/real_exports.rs`, `corpus_parse.rs`, `corpus_detect.rs`, `corpus_export.rs`, `resolve_corpus.rs` and `resolve_names.rs` pin detection, sections, entries, the round trip and name resolution over them; the `mtg-db` test `card_index_names.rs` requires the stored index and an index built from the records to resolve alike.

## Writers

`decklist::export_as(format, deck)` writes plain text (`export`), the Arena export or MTGO text, and returns text only after reading it back through the parser of that dialect as the deck the format can hold.

- **Arena**: `Commander`, `Companion`, `Deck` and `Sideboard` sections in that order, printings kept, no maybeboard.
- **MTGO text**: names only, the main deck, a `SIDEBOARD:` block (a companion goes there, since MTGO has no companion zone) and the commanders in a last block with no header. A list of commanders alone cannot be written, because the block after the main deck is what marks the command zone; the writer refuses it. Plain-text detection reads a list without `SIDEBOARD:` as plain text, so MTGO text is read back with the format named.

## Commander promotion and detection

- Commander imports call `decklist::promote_implied_commander(format, deck)`, which only moves Moxfield's first line (a single copy, no commander marked); the server also moves the second line when the two cards can be commanders together (`deck::can_pair`). Moxfield's text does not say how many lines are commanders, so for a partner pair or a Background the first moves and the second stays in Main until the deck builder fixes it. Signature spells are deck cards and stay in Main. A Moxfield list with no foil or etched card and no sideboard or stickers reads as plain text and is not promoted, so a caller should let the user name the site.
- Every importer keeps the command zone as `Section::Commander`: Moxfield's first line after promotion, Archidekt's `{top}` category, Arena's `Commander` header, MTGO text's trailing block and MTGJSON's `commander` board (`displayCommander` is cosmetic). A saved deck keeps it as its `commander` zone and a stored precon as its `commander` board; a Commander game starts each seat with them in its command zone ([server.md](server.md), Choosing a deck).
- `detect` is decisive for Moxfield text with a foil or etched flag or an all-caps header, MTGO text with an all-caps header, every Arena export with an `About` block and Archidekt text with brackets. It cannot know, and falls back to plain text, for Moxfield lists with neither flag nor header, MTGO text without header, and Archidekt lists without brackets (with a foil flag the latter reads as Moxfield, whose flag syntax it shares). A bracketless Archidekt list that opens with a header word reads as Arena. Every fallback reads the same cards in the same sections except an MTGO commander block, which plain text takes for the sideboard.

## Name resolution

The parser leaves names as written; `resolve::resolve_deck(index, deck)` ties each entry to one card, by these rules in this order.

- **Printing**: a set and a collector number (lower case, numbers as free text) name a printing, and its card is the entry's card unless the entry's name is another card's.
- **Card name**: compared after folding case, accents, ligatures, typographic punctuation and spacing. Faces are `A // B`, `A / B`, `A /// B` or `A/B`, and the front face alone matches too. A trailing `(SET)` that the parsers split off is retried as part of the name.
- **Other names of a printing**: flavor names, the names printed on English digital versions and `A // B` of a meld piece, in full or by the front face. They are tried last and never take a card name from its card. A flavor or digital name also selects its printing: an entry with no set and no number takes the set and number of the one printing of the card that carries the name (none when several do, and none for a meld name, which belongs to the card). Moxfield's Arena and MTGO exports write such a name with no set, so only this rule reads them.
- A printing and a name that belong to different cards leave the entry unresolved as `AmbiguousName`, the printing's card first. A name several cards share is ambiguous, a name no card has is unknown with up to five fuzzy suggestions, and a suggestion is never applied. `ResolvedEntry::matched` tells which rule tied the entry: `Printing`, `Name` or `OtherName`.
- `CardIndex::from_records` takes every record as a printing and, from English ones, `flavor_name`, `printed_name` and the meld name. `CardRepo::card_index` builds the same index from the stored deck cards, their `melds_into` and printings, so it resolves every line to the same card and printing; `CardRepo::index_version` fingerprints the cards and printings and tells whether an index built earlier is current. Oracle Cards has one printing per card; Default Cards brings every printing with its flavor and printed names.
