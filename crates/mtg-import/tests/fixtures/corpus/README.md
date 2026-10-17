# Export corpus

Deck exports from public Moxfield and Archidekt decks, for parser tests. Hand-made fixtures live one level up and the owner's own exports in `../real`. Each deck folder holds one text file per export variant and a `truth.json` with what the site itself says the deck contains, so a test can compare `parse(export)` with the site's own boards.

## Provenance

- Fetched 2026-10-17: 16 Moxfield and 13 Archidekt public decks, at one request per 1.3 s and per 3 s with an honest user agent. Only card lists are kept (no authors, comments or prices); they are used only as parser fixtures.
- **Moxfield**: every `.txt` is the byte-exact body of the site's own export endpoint (`format=full`, `format=mtgo`, `/export/arena`, plus option variants), so the quirks are real. A variant identical to another is not stored.
- **Archidekt** exports client-side, so the files are rendered from the public deck JSON by a port of the site's exporter, read from its JS, with the option sets a user can pick. All 1406 lines of the `default` files and of the owner's two real exports fit one line grammar. Assumed, not observed: the export includes sideboard and maybeboard cards (the `{noDeck}` flag implies it), and card-type headers use our own type precedence (matches the site's default categories on 184 cards).

## truth.json

- `source`: `site`, `id`, `url`, `format`, `name`, `why`, `fetched`.
- `boards`: `commanders`, `signatureSpells`, `mainboard`, `sideboard`, `maybeboard`, `stickers`, `tokens` (only the non-empty ones). Rows are `{name, qty, set, cn, finish}` with the oracle name (`A // B` for faces), a lower-case set code and `finish` of `nonFoil`, `foil` or `etched`. On Archidekt the first category decides the board; later categories never change it.
- `variants.<name>`: `boards` lists what the text carries (Moxfield exports never carry maybeboard or tokens); `commanders` says how they are marked: `marked` (Archidekt `{top}` flag or `Commander` header), `first-lines` (Moxfield `full`: unmarked, then the oathbreaker's signature spell), `last-block` (Moxfield `mtgo`: trailing block without header), `section` (Arena `Commander` section) or `none` (expect every card in the main list); `lossy` means cards without an Arena id are missing, so expect a subset.

## Quirks the corpus exercises

- Moxfield: `A / B` for double-faced cards; flavor names instead of oracle names (`full` vs `full-noflavor`; "The Ball" is Fellwar Stone, SLD 1040); collector numbers `TMP-315`, `2026-2`, `271p`, `242s`, `121★`, `2016`; `_____ Goblin`; a `STICKERS:` section; `*F*` and `*E*` flags; `#tag` suffixes in `full-tags` (`#!Lands`, `#Mana (LOCKED)`); Arena `About`/`Name` lines.
- Archidekt: several categories in one bracket (`[Ramp,Maybeboard{noDeck}{noPrice}]`), `{top}`, `^Label,#hex^`, `1` against `1x`, section headers by role, category or card type, front-name-only faces, cards without categories exported under their type, collector numbers `STH-134`, `73s`, `we313`, `tvdl345`, `107m`, `mb283`, `226p`, `251a`.
- Not covered: companions and custom cards (no public deck found), MTGO `.dek` and client exports (hand-made fixtures exist).

## Decks

### Moxfield

| Deck | Format | Cards (cmd / main / side / maybe) | Variants | Why it is here |
|---|---|---|---|---|
| `blue-farm` | commander | 2 / 98 / 0 / 36 | 4 | two commanders; emoji in the deck name |
| `brawl-looters` | brawl | 1 / 59 / 1 / 147 | 4 | 60-card Brawl deck with a commander and one sideboard card |
| `clockside` | commander | 1 / 99 / 27 / 70 | 5 | accented names (Fili, Kili, Oin with accents); 27 sideboard cards |
| `duel-commander` | duelCommander | 1 / 99 / 0 / 0 | 4 | Duel Commander list |
| `krark-sakashima` | commander | 2 / 98 / 0 / 12 | 6 | two commanders; transform and modal double-faced cards |
| `krrik-storm` | commander | 1 / 99 / 0 / 113 | 6 | deck name with brackets, quotes and emoji (shows in the Arena Name line) |
| `legacy-tes` | legacy | 0 / 60 / 15 / 532 | 5 | Legacy deck: old cards, a 15-card sideboard, a 532-card maybeboard |
| `medium-midrange` | commander | 12 / 588 / 0 / 18 | 4 | twelve cards in the commander board, a messy real list |
| `modern-goryo` | modern | 0 / 60 / 15 / 0 | 3 | 60-card Modern deck with 15 sideboard cards |
| `oathbreaker` | oathbreaker | 1 / 59 / 0 / 0 | 4 | Oathbreaker: oathbreaker plus signature spell |
| `pauper-cycle-storm` | pauper | 0 / 60 / 15 / 107 | 3 | Pauper deck with a 107-card maybeboard that exports must skip |
| `pauper-staples` | pauper | 0 / 792 / 0 / 27 | 3 | 792-card list, a size stress test |
| `rograkh-silas` | commander | 2 / 98 / 0 / 138 | 7 | two commanders (Rograkh and Silas) |
| `standard-gruul` | standard | 0 / 60 / 15 / 0 | 3 | Standard deck: recent sets, adventures and modal cards |
| `winota-snowball` | commander | 1 / 99 / 17 / 197 | 7 | commander with 17 sideboard cards; double-faced cards written 'A / B'; The List collector numbers |
| `yuriko-tempo` | commander | 1 / 99 / 10 / 20 | 6 | Lim-Dul's Vault with accents; seven double-faced cards; 10 sideboard cards |

### Archidekt

| Deck | Format | Cards (cmd / main / side / maybe) | Variants | Why it is here |
|---|---|---|---|---|
| `black-burn-modern` | modern | 0 / 60 / 15 / 0 | 9 | modern 60 plus 15 with double-faced cards |
| `burakos-background` | commander | 2 / 98 / 0 / 0 | 8 | commander plus a Background as second commander, basic-land stacks, foils, 'STH-134' style collector numbers |
| `dakkon-oathbreaker` | oathbreaker | 2 / 58 / 3 / 0 | 8 | oathbreaker with a split card as signature spell, cards without categories, numbers like 226p, 251a and mb283 |
| `erinis-six-commanders` | commander | 6 / 246 / 0 / 0 | 9 | six cards in the Commander category, 252 cards of which 95 are lands, etched and foil finishes |
| `huatli-oathbreaker` | oathbreaker | 2 / 58 / 0 / 48 | 9 | oathbreaker: planeswalker and signature spell both in the Commander category, 48-card maybeboard, many excluded categories |
| `izzet-pauper-edh` | paupercommander | 1 / 99 / 0 / 65 | 9 | pauper commander, 65 maybeboard cards, an excluded 'Alt. Commanders' category, accent in a card name, 107m number |
| `korvold-brawl` | brawl | 1 / 59 / 0 / 0 | 8 | brawl precon: 24 cards without categories (exported with their type as category), one commander |
| `modern-artifact-burn` | modern | 0 / 60 / 15 / 0 | 8 | 60 plus 15 modern list with no commander, numbers like we313 and tvdl345 |
| `rakdos-burn-pauper` | pauper | 0 / 60 / 15 / 0 | 8 | pauper 60 plus 15, many older printings |
| `riders-of-rohan` | commander | 1 / 99 / 0 / 0 | 7 | precon-style commander deck with accents in four card names |
| `tymna-kraum-partners` | commander | 2 / 98 / 1 / 29 | 9 | partner pair, one sideboard card and a 29-card maybeboard, etched finish, split/MDFC/transform/adventure faces |
| `tymna-thrasios-cedh` | commander | 2 / 98 / 0 / 99 | 9 | partner pair, 12 categories of which 7 are excluded from the deck, 154 multi-category cards, a very long title, 73s-style numbers |
| `wyll-gruul-background` | commander | 2 / 98 / 0 / 7 | 8 | commander plus a Background, seven maybeboard cards, multi-category cards |
