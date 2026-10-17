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
