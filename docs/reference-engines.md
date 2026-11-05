# Forge reference

How the project uses Forge's card scripts: a cross-check of the compiler's readings (`forge-check`) and the source of the translator (`translate-forge`). Other open-source engines are not used.

## Licence and storage

- Forge is GPLv3. Its scripts are read at run time from an owner-provided checkout (`--forge <checkout>`, or the `cardsfolder` directory); nothing of a script is copied or stored in the repository, and the clone stays outside it. The test scripts are invented in Forge's format.
- `forge-check` keeps only card names, Forge's API, mode and key names, and counts. The proposals `translate-forge` writes are data derived from the scripts, used by the owner and friends only and not redistributed. Rules rows reach browsers (`cards-dsl.md`), so a bulk conversion of scripts into rules files would be conveying a work based on Forge; the translator writes proposals only, and the checker judges them against the printed text like any proposal ([cards-dsl.md](cards-dsl.md)).

## Script format

A Forge card is one text file per card in `forge-gui/res/cardsfolder/<letter>/`. A line is `Tag:body`: `A:` abilities, `T:` triggers, `S:` statics, `R:` replacements, `K:` keywords, `SVar:` named sub-parts, `Oracle:` text; faces are joined by an `ALTERNATE` line. A body is `Key$ Value` pairs split at pipes. The effect verb is the value of `SP$`, `AB$` or `DB$` (an API such as `ChangeZone`, `Pump`, `Draw`), the trigger or static kind is `Mode$`, a replacement's is `Event$`. All behaviour is in Forge's engine, so a script is data: verb and parameters. A small number of the most used APIs and trigger modes covers most uses, which is why the tail is grouped by API.

## Translator

`mtg-cli translate-forge <checkout> [--cards NAME..] [--stored] [--precons] [--out DIR]` (`crates/mtg-cli/src/translate`) translates scripts with mappers and no model into proposals in the format `propose` takes. A construct without a mapper leaves its line out, so the checker sees the clause unwritten and marks the card `Partial`; every ability written parses as the DSL's `Ability`.

## Cross-check report

`mtg-cli forge-check --forge <checkout> [--deck CODE_OR_NAME] [--top N] [--keys N] [--out FILE]` sets every stored precon card against the Forge script of its name (`crates/mtg-cli/src/forge`).

| Part | Rule |
|---|---|
| Reader (`script.rs`) | Reads `A:`, `T:`, `S:`, `R:`, `K:` and `SVar:` lines, faces split at `ALTERNATE`. The verb of an item is the effect API, the mode or event, the keyword name, or a part of a `Cost$` (a spell's mana is skipped). A name in a value (`SubAbility$`, `Execute$`, `Choices$`, `Triggers$`) is followed once to its `SVar`; a part that no line names is read too. `Count$` expressions and hints for the computer player are not items; keys that only carry text, targeting, references or remembered objects are left out. A keyword the DSL has whole (`Backup`, `Gift`) holds its ability, so the parts it names give no item. |
| Map (`map.txt`, `map.rs`) | A Forge item with optional key conditions maps to the verbs of ours that stand for it (any one is enough), named as `used_primitives` names them (`Effect.Move`, `Trigger.StepBegins`). The rule with the most conditions that hold wins. `-` is an item with no DSL verb, `~` bookkeeping. `also` lines name a key that asks for a verb of its own (`CopyPermanent` with `NonLegendary` asks for `Mod.RemoveSupertype`); `variant` lines name a key whose value tells uses apart (`Destination=Exile`). A keyword with no rule stands for the keyword of its name. A test checks that every verb of the map is a variant of the DSL and that the map names the most used APIs. |
| Comparison (`compare.rs`) | The verbs of a card's rules are the primitives of its modelled abilities (the hand-written file, else the compiler's reading). An item is read when one of its verbs is among them, missing when none is, `no verb` or `unmapped`. The comparison is by sets for the whole card, not by clause. |
| Report (`report.rs`) | A card is unread when a clause is left to the table or its rules are unusable. The unread tail is grouped by Forge API (triggers, statics, replacements and keywords by family and name) with the cards that carry each key and the cards no other group holds; a key that 90% of a group's cards carry is a parameter of the verb and is left out; costs are read with their ability. The cover is the share of the grouped cards whose groups all lie among the top groups. |

One known disagreement is correct: an optional trigger that Forge states with "you may" and the Oracle text states without stays mandatory (CR 603.5 makes a trigger optional only when its text says "may").
