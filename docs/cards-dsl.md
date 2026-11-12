# Card DSL and effect pipeline

Rules data for cards (`mtg-cards`), how it is checked and produced, and the rules of the pipeline that runs it (`mtg-engine`). The types are documented in `crates/mtg-cards/src` (`ability.rs`, `effect.rs`, `trigger.rs`, `modify.rs`, `cost.rs`, `value.rs`, `keyword.rs`); this file holds what the code does not say. Where the engine keeps each rule is in `engine.md`; see also `protocol.md`, `deck-check.md`.

## Summary

| Area | Rule |
|---|---|
| Printed side | Name, cost, types, power, toughness and oracle text come from the imported Scryfall record. |
| Rules file | One JSON file per oracle card: abilities per face, keyed by oracle id, with the fingerprint of the oracle text they match. |
| Vocabulary | Verbs (`Effect`, `TriggerPattern`, `CostItem`, `Modification`, `Keyword`) and values (`Ref`, `Selector`, `Amount`, `Condition`). A primitive is one enum variant. |
| Execution | An effect list runs as a journal: each step is a pure function of state that yields events or asks one `Decision`; answers are logged as events. |
| Stack | Abilities are stack objects. Targets are chosen on announcement and rechecked on resolution (CR 608.2b). |
| Triggers | Detected from events (look-back ones against the state before the event), queued in state, put on the stack in APNAP order. |
| Replacement | A step in front of the fold; the affected player orders several (CR 616.1). |
| Static abilities | Derived when characteristics are derived (CR 613); never stored as effects. |
| Keywords | Enum variants with parameters; behaviour in one engine file each. |
| Support | `Verified`, `Read`, `Forge`, `Partial`, `Manual`, `Stale` and `Unplayable` are computed from the file, the imported text, the engine's capabilities and the stored read verdicts. |

## Decisions

| # | Decision | Reason | Rejected |
|---|---|---|---|
| D1 | Format: JSON, one file per oracle card; serde's externally tagged enums, PascalCase variants, snake_case fields, unknown fields are errors, defaults omitted. Short forms: a mana cost or mana-only `Cost` is a string (`"{X}{R}"`), a fixed `Amount` a number | The database stores JSON, the compiler emits data, the web reads it, errors carry a path (`serde_path_to_error`) | RON (no jsonb or TypeScript reader), YAML and TOML (typing traps), Rust builders (not data), a text script (own parser) |
| D2 | Identity: the key is the Scryfall `oracle_id`; the file is `cards/<slug>.json` at any depth; a predefined token is `cards/tokens/<id>.json` | `cards.oracle_id` is the database key; names change and collide with face names | A slug as key (a rename breaks joins), per-printing ids (rules belong to the oracle card) |
| D3 | Multi-face: one file per oracle card, `faces` in Scryfall order; a whole-card property (Fuse) is a keyword on face 0 | Decks and imports name the card; faces share identity and status | One file per face, a `layout` field (a copy of import data) |
| D4 | Characteristics: the printed side always comes from the import record; the file holds abilities and a per-face `oracle_fingerprint`; a token carries its own characteristics | Cards without a file need the record anyway, so there is one source; errata surface as drift | Copying cost, types and P/T into each file (a second source, silent typos) |
| D5 | Shape: verbs and values are separate; sequences are lists; results are named (`as`, `Bound`) | Names are validated; a new mechanic is one verb over existing values | Implicit anaphora (`last created`), one flat enum, a class per card |
| D6 | Support is computed, never stored | `Verified` cannot go stale or ignore an engine that lacks a primitive; no engine run | A stored flag, running tests per lookup |
| D7 | Execution is a journal of pure steps | State stays the fold of the log; a restart asks the same question; each step is testable | Blocking callbacks per choice, unlogged resolution flushed at the end, replay from the start on every answer |
| D8 | Triggers: detection from events, a queue in state | Detection is a function of events and data; the log shows why an ability appeared; a resume keeps the queue | Detection inside the fold, a queue outside state |
| D9 | Replacement: a step in front of the fold | The fold stays dumb; the log holds what happened | A `replaced` flag on events, replacement inside each primitive, a fixed global order |
| D10 | Static abilities are derived | Characteristics are already derived; layer dependencies come from the data | Registering statics as `ContinuousEffect`s, a per-card layer table |
| D11 | Keywords: one enum, one engine file each | An exhaustive `match` flags a forgotten keyword; parallel work touches separate files | Keywords as strings, a shared registry file |
| D12 | Validation in three levels; all errors are collected | A file is judged where the information is | Checking when the engine uses a card, stopping at the first error |
| D13 | The compiler writes the same files; an unreadable clause becomes a `Manual` ability | One reader and one validator; a deck is never blocked | A second format, refusing decks with unimplemented cards |
| D14 | Read tier: a whole draft the checker cannot prove runs as `Unverified` until a signed-in player reads it beside the engine's English: `Read` once found correct, by hand after "reads wrong"; verdicts are stored per rules hash, the state is still computed (D6) | The checker has no writer for many valid rules; a human read is fast when the engine's reading is shown; `Verified` keeps its proof | A stored per-card flag, drafts in the corpus (the ratchet and `NOT_VERIFIED` stay about `Verified`) |

## Files

| Path | Content |
|---|---|
| `crates/mtg-cards/cards/<slug>.json` | Rules of one oracle card |
| `crates/mtg-cards/cards/tokens/<id>.json` | A predefined token (Treasure, Clue, Food, Gold, Blood, Junk, Powerstone, Vibranium, Map, Mutagen, Lander) |
| `crates/mtg-cards/tests/fixtures/corpus/<slug>.json` | The imported Scryfall object of one corpus card |
| `crates/mtg-cards/tests/fixtures/reproduced.txt` | The ratchet list: the corpus cards the compiler reproduces, one name per line, sorted |
| `crates/mtg-cli/drafts/<slug>.json` | A draft (Read tier); its card record is `drafts/fixtures/<slug>.json`, its meta `drafts/meta/<slug>.json` |

- `mtg_cards::builtin::library()` is the library of the corpus files, embedded at build time (`build.rs`), so a server needs no directory and a deploy carries the rules it was tested with.
- Canonical text: compact JSON in serde's field order, laid out by `mtg_cards::pretty` (a value stays on one line up to 100 columns, otherwise one entry per line).
- `MTG_UPDATE_CARDS=1 cargo test -p mtg-cards corpus::` rewrites the files and their fingerprints; the test fails on a file that is not canonical.
- `cargo test -p mtg-cards hand_written::` checks each rules file alone (`Library::check_generated`) against its fixture and the newest `oracle-cards-*.jsonl.gz` (`MTG_REAL_DATA_DIR` or the `mtg-cli` cache; without one, the fixture only). An erratum fails the card: fix the rules, or refresh the fingerprint when they still fit.
- `cargo test -p mtg-cards fixture_real::` compares the fixture records with the same download; without one it says so and passes.

## Adding a corpus card

No test holds a count of cards, sources, compiled sources, reproduced cards or fingerprints; each is derived from the card directory and the fixture, and a test asserts what the number stood for. A new card needs:

1. Its rules file `crates/mtg-cards/cards/<slug>.json` (the slug is the card's name lowercased with every run of non-alphanumerics as `_`, `mtg_cards::load::slug`). A `Full` or `Partial` face starts with an empty `oracle_fingerprint`.
2. Its Scryfall object as the new file `crates/mtg-cards/tests/fixtures/corpus/<slug>.json`: one pretty-printed object with a final newline (a multi-face card under its whole name), sharing the `oracle_id` with the rules file. No other file changes, so branches adding different cards never conflict here.
3. `MTG_UPDATE_CARDS=1 cargo test -p mtg-cards corpus::` writes the rules file in canonical form with the fixture's fingerprints and adds every card the compiler reproduces to `crates/mtg-cards/tests/fixtures/reproduced.txt`. It never removes a name: a listed card that stops being reproduced fails.
4. `cargo test -p mtg-cards -p mtg-testkit`. A Manual or Partial card is a row of `NOT_VERIFIED` in `crates/mtg-testkit/tests/fuzz/corpus_cards.rs` with its state and ability count; a card played whole needs none. A Verified card carries its file's abilities plus a mana ability per basic land type.

`reproduced.txt` is sorted and the update inserts names in place; two branches conflict only on adjacent names, resolved by keeping both lines (the update restores the order).

The tests name the card, the file to add and this section for: a card without record or file, a fixture misnamed or without final newline, two files with one oracle id, a stale or missing fingerprint, an unlisted reproduced card and an unexpected state. `tests/it/adding_a_card.rs` adds a card to a copy of both directories and shows that no count or table needs an edit.

## Printed side and rules

The printed side of every card comes from the import record (`mtg_import::CardRecord`, stored in the `cards` table). The rules file adds abilities. A deck of oracle ids becomes `CardDefinition`s in the server (`CardSource`, `DeckRef::Oracle`):

| Step | Action |
|---|---|
| 1 | `mtg-db` reads one `cards` row per distinct oracle id of the deck (`CardRepo::get_many`); every id without a row is named in the error. |
| 2 | The server adapts each row to `mtg_cards::PrintedCard` (`mtg_server::printed_card`; strings only, so `mtg-cards` does not depend on `mtg-import`); the testkit does the same from the record. |
| 3 | `assemble(&PrintedCard, Option<&Card>, &Caps) -> Assembled { definition, support }` reads the front face (cost without `{X}` plus `x_in_cost`, type line, stats, colours), gives a land its basic types' mana abilities (CR 305.6) and attaches the rules the engine runs. A card with no file is `Verified` with no rules text outside reminder text, `Manual` otherwise. |
| 4 | `PlayerSetup.library` gets one shared `Arc<CardDefinition>` per deck line. `AbilityDef::Mana` is the lowered plain `{T}: Add {X}`; every other ability the engine runs is an `Arc<Ability>` in `CardDefinition.rules`, named by its index (`lower`). |
| 5 | The deck fingerprint (`recipe/fingerprint.rs`) hashes the serde form of the definitions, so a corrected record, changed rules or changed engine coverage fails a resumed game as a changed deck does. A new `CardDefinition` field stays out of that form while unused, so `FINGERPRINT_VERSION` stays 1. |

A printed gap makes the card `Manual` and attaches no rules; `support` computes it, so the deck check and the game agree, and a `Caps` name lifts it once the engine plays the value:

| Printed value | In the definition | Lifted by |
|---|---|---|
| `*`, `1+*` or `X` as power, toughness, loyalty or defense | `None` | `Card.VariableStats` |
| `{X}` in the cost | `x_in_cost`; the engine asks X at the cast, and the spell, its permanent, its enters trigger and its delayed triggers remember it | `Card.XCost` |
| A second face | Split, Adventure, modal and transforming double-faced cards play with their faces, each assembled like a card (`CardDefinition.layout`, `faces`); a `prepare` card's spell face is a copy cast by the prepared permanent; any other second face (meld) is only its front | `Card.Faces`, `Card.Prepare` |
| A mana symbol the engine does not know, an unreadable cost, a type word that is no card type | No cost, or the word dropped | Nothing |

## Vocabulary

- A node is a variant of a type in `crates/mtg-cards/src`; the rustdoc of each variant says what it means. `mtg-cli dsl-reference` prints the type tree as the model prompt of `propose` sees it, and `mtg-cli dsl-schema` its JSON schema.
- Defaults: `who` is `You`; `source` is `This`; an `Amount` is 1; a `Bounds` is 1..1 and `max: null` is any number.
- `Target(i)` is the set chosen for the i-th `TargetSpec`: one object for a plain spec, several for "any number". An effect on a set applies to each member.
- A `Selector` is a list of `Pred`s that must all hold; the zone is the battlefield unless `In(zone)` says otherwise.
- Values (`Ref`, `Selector`, `Amount`, `Condition`) are evaluated when an effect runs or a pattern is matched and produce no events.
- One reader and one writer turn every object phrase into a `Selector` and back (`selectors::phrase`, `selectors::phrase_text`); every effect, trigger, condition and static reader uses this pair, so a new atom works in all of them.
- Machinery that counts "this way" (`Destroyed`, `Exiled`, `LifeLostThisWay`) counts what the resolving spell or ability has done so far, as the objects last existed.
- A keyword action that abilities wait for ("whenever you investigate", "discover", "manifest dread", "give a gift") is logged by `Effect::Performed` after its steps: the reader adds it after an investigation's Clue tokens, `effect::discover` and the steps of manifest dread end with it, and gift's steps end with it in the engine. The engine logs `Investigated`, `Discovered` (value and card), `DreadManifested` (the card put into the graveyard) and `GiftGiven` (to whom), read as `TriggerPattern::Did`. The writers word the steps; an investigation's log is dropped before its Clue tokens are worded.
- "Sacrifice any number of ..." is a `SacrificeUpTo` of every match and "discard any number of ..." an optional `Discard` of the hand, each bound by name; `Bound("<name>~moved")` is how many it took ("that many", "for each permanent sacrificed this way").
- "Spend only black mana on X." is `CostItem::XColor`: the X of the cost is paid with mana of that color; a spell holds it as a required `AdditionalCost` that pays nothing.
- "For each card type, you may put a card of that type ..." and "for each color pair, choose a card ..." are one `Choose` per kind, each from the cards no earlier choice took (`Not(Is(Union[...]))`), then one move of the `Union` of the bound sets; a `Union` of bound sets reads anywhere objects are read.
- Loyalty activations change by `GameRule`: `LoyaltyTwice` (twice each turn, not cumulative), `LoyaltyOnceMore` (one more each turn per rule, The Chain Veil as an `Effect::Rule`), `LoyaltyAsInstant` and `LoyaltyCostMore` (an additional `[+N]`, one change of counters with the printed cost).
- `GameRule::MustTarget` is the Flagbearer rule: the last target specification that could take such a permanent takes only those when no earlier target is one.
- `Modification::KeywordsOf` gives the named keywords of other objects (landwalk, protection and hexproof in every variant); `Amount::DealtUpToHealth` is damage dealt so far capped per dealing by the recipient's life, loyalty or toughness before it; `Produce::Unspent` doubles the pool by type and `Produce::ManaCostOf` adds a mana cost, a hybrid symbol chosen.
- A new primitive adds its name to `Caps` (below) only when the engine runs it.

## Support records

| State | Meaning | From |
|---|---|---|
| `Verified` | Rules cover the whole text, the text is unchanged, the engine runs every primitive used; or there is no text to cover | `impl_status: Full`, equal fingerprints, `Caps` cover the primitives; or no file and no rules text on any face once reminder text is removed, and no printed gap |
| `Read` | A whole draft the checker could not prove, read and found correct by a player; runs like `Verified` | A draft with no `Manual` ability, equal fingerprints, `Caps` covering every primitive, and at least one "correct" and no "wrong" verdict on its `read_hash` (Read tier) |
| `Forge` | A trusted whole translation of a Forge script (Forge source); runs like `Read`, counted and shown apart | As `Read`, with the draft `approved` by its Forge source instead of a player |
| `Unverified` | A whole draft that waits for its first read; runs like `Read`, counted and shown apart, never as `Verified` or `Read` | As `Read`, with the draft `pending` |
| `Partial` | Rules exist except the listed clauses | `impl_status: Partial`, or `Full` with primitives the engine lacks |
| `Manual` | No rules, or the engine runs none of them | No file and rules text, `impl_status: Manual`, or no primitive covered; or a compiled row or approved draft not marked `differs` with a clause that reads back as other text (`proposal::differing`, named `readback:Differs`; the corpus is held to it by `fix21::no_verified_corpus_card_reads_back_as_other_text`) |
| `Stale` | The oracle text or the face list changed since the rules were written; counts as `Manual` | `Full` or `Partial` with a different fingerprint |
| `Unplayable` | The engine cannot host the card | `impl_status: Unplayable` |

| Item | Rule |
|---|---|
| Declared and computed | The file declares `impl_status` (`Full` is the database enum `full`); `support(Option<&Card>, &PrintedCard, &Caps) -> SupportRecord` computes the state. CI holds that a `Full` file has a scenario test or compiler golden. |
| Clause | An `Ability::Manual { reason, span }`. `reason` is plain language; `span` is `{start, end}` in Unicode scalar values of the face's imported oracle text, end exclusive. |
| Status and abilities | `Full` has no clause; `Partial` has at least one clause and one modelled ability; `Manual` and `Unplayable` have no abilities. Clauses do not overlap and lie inside the text. |
| `Caps` | The primitives the engine implements, by enum and variant (`Effect.DealDamage`, `Keyword.Flying`, `Cost.Tap`), plus names for shapes only some engine paths run (`Effect.Discard.Random`, `Amount.X.Elsewhere`) and printed gaps (`Card.XCost`). `mtg_engine::caps()` supplies it from `RUNS` in `caps.rs`; `lower` carries every covered ability into the definition. Value nodes are gated only where the evaluator cannot read them yet. |
| Engine gap | `used_primitives(ability)` lists what an ability needs. A face with an uncovered primitive reports the clause "the engine does not run X yet" over the whole face text. A card moves from `Manual` to `Partial` to `Verified` as primitives land, with no file edit. |
| Declared ahead of the engine | A node the engine does not run is declared with a stub (an effect file in `resolve/prim/` refusing with `ResolveError::Unsupported("Effect.<Name>")`, a cost part named in `price.rs`, a pattern that matches nothing, a keyword that changes nothing); the support walker names it, so its card stays `Partial` until `caps.rs` lists it. |
| `has_rules_text(text)` | True when the text has anything outside parentheses (an unclosed parenthesis counts as text). |
| `oracle_fingerprint(text)` | Lines trimmed, whitespace collapsed, blank lines dropped, joined by `\n`; SHA-256; first 16 bytes as 32 hex characters. |
| `implementation_hash(card)` | The same digest of the compact JSON of every face's abilities. |
| `SupportRecord.fingerprint` | SHA-256 (first 16 bytes) over `oracle_id`, state, clauses, implementation hash, stored fingerprints and current fingerprints. It changes with the data, the imported text and the engine's coverage of the card. The deck's `coverage_hash` is built from `(oracle_id, fingerprint)` (`deck-check.md`). |
| `Library::support(&PrintedCard, &Caps)` | The pure per-card lookup the deck check calls. |

### Read tier

A card the checker cannot prove runs automatically (D14); `Read` and `Unverified` are counted on their own wherever states are counted.

| Item | Rule |
|---|---|
| Draft | A whole rules file (no `Manual` ability) with verdict `Unchecked`: valid, nothing contradicts the text, a construct has no writer. A matched Forge draft may also be `Mismatch` through an ability without a clause, or through a `Differs` clause when its meta marks it `differs` (then `Read` after a player's read, never `Forge`; the deck check shows each such clause beside the engine's words, `ReportCard.differs`). Other `Invalid` and `Mismatch` proposals never become drafts; translator drafts are checked without repairs; `Accepted` ones join the corpus. Drafts live in `crates/mtg-cli/drafts/`. |
| `read_hash` | SHA-256 (first 16 bytes as 32 hex characters) over the oracle id, each face's current `oracle_fingerprint` and the draft's `implementation_hash` without its keyword-action logs (`Effect::Performed`, which change nothing a player reads). A changed draft or an erratum gives a new hash; `Caps` do not enter it. |
| Verdict | `correct`, or `wrong` with a note, by one account on one `read_hash`; the account's newest verdict on that hash counts ([server.md](server.md), Reads). The server's own `wrong` (a resolution that failed in a game) has no account. |
| Read state | Per `read_hash`: `wrong` when a counting verdict is `wrong`, else `approved` when one is `correct`, else `pending`. A verdict on another hash counts for nothing, so a changed draft or text is read again. |
| State | `Read` when the draft is `approved`, `Unverified` when a whole draft is `pending`, both only while its fingerprints are current and `Caps` cover it; otherwise the card keeps the state its file or compiled row gives. |
| Precedence | A rules file, then a `Verified` compiled row, then a draft in state `Read`, `Forge` or `Unverified`, then a `Partial` compiled row, then nothing. A wrong draft changes no game. |
| Library | `Library::with_drafts(&[DraftRules {oracle_id, read_hash, rules, read}])` keeps each card's current draft (later rows win) and returns the rows it left out (unparsable, invalid, not whole); `support` and `assemble` follow the precedence, `Library::draft(oracle_id)` serves the review and `Library::draft_state(printed, caps)` the deck check. It stays pure. |
| Definition | `assemble` sets `CardDefinition.read` for a card in state `Read`, `Forge` or `Unverified` (left out of the JSON while false, so no other fingerprint changes); the table shows it and fail-soft resolution reads it ([engine.md](engine.md), Fail-soft resolution). |
| Sheet | `mtg_cards::read::sheet(&Card, &PrintedCard) -> ReadSheet` gives one `ReadRow` per printed clause in text order, from the checker's pairing: the clause, its `state` (`Written`, `Unwritten`, `Differs`, `ByHand`), its abilities and `steps` (depth-marked lines of what the game asks, checks and does). Every node has words; a Saga line of several chapters ("I, II —") shows the abilities of the chapters it names; a clause without an ability says it is played by hand. |
| Smoke gate | `mtg_testkit::smoke::smoke_card(&CardDefinition, seeds)` plays the card with `read` cleared, so a rules error fails: per seed a bot game at two and four seats under every invariant and the watchdog, bots favouring the card. It passes when no game fails and the card was used; otherwise `NeverSeen`, `Invariant`, `Engine`, `Stuck`, `BadEnding` or `Panicked`. |

#### Forge source

A draft has a source (`forge` for a Forge translation) and `oracle_matches` (Forge's Oracle text equals the printed text after normalising). A `forge` draft with `oracle_matches` and no `differs` mark is `approved` without a human verdict (a `wrong` verdict still wins); its card is `Forge`, run like `Read` but counted and shown apart. Any other Forge draft needs a read. Source, match and mark live in `drafts/meta/<slug>.json`, are stored by `compile` in `card_drafts` and reach the library as `DraftRules.forge` and `DraftRules.differs`.

#### Bulk reads

Many `differs` drafts differ from the card only in wording. `mtg_cards::wording` decides, per `Differs` clause and with no model, whether the printed words and the engine's say the same thing under a fixed list of named equivalences (`Rule`); a card whose every such clause matches is a bulk candidate.

- Each rule rewrites both wordings to one canonical form; a clause matches when the forms are equal and lists the fewest rules it needs. The rules are the variants of `Rule` (e.g. `self_name`: "this creature" as ~; `back_reference`: "that creature" as it). None drops a number, target, controller, duration, "may" or condition, and each has near-miss tests.
- A match is a likely equivalence, not a proof: the card stays `Differs` and still needs one human read, given in bulk (`GET` and `PUT /api/reads/bulk`, [server.md](server.md), Reads). A plainly exact equivalence belongs in the checker or renderer, so the card reads `Accepted`.
- `mtg-cli read-candidates [--out FILE] [--misses FILE]` judges the embedded `differs` drafts against the local database, read-only: counts per rule, a TSV of slug, name, both clauses and rule, and optionally the clauses that matched nothing.

## Execution

State is the fold of events; the engine is a pure function from state to `Progress` (events or a decision).

| Topic | Rule |
|---|---|
| Ability objects | `GameObject` has a kind: `Card`, `Token`, `Ability`. An ability lives in `Zone::Stack` and holds its source, a `Snapshot` of it, the controller, an `Arc` to its rules (so it resolves without its source, CR 113.7a), its `Choices` and a trigger's cause. A spell is the card object. |
| Identity | Every object has a zone-change counter bumped by the fold (CR 400.7). A stored reference is `(ObjectId, counter)`. |
| Announcement | One procedure in the order of CR 601.2b to 601.2h (`engine.md`, Announcement): a specification that allows no target is skipped, one that cannot be met rewinds the cast. A trigger chooses its targets as it goes on the stack (CR 603.3d), and those answers are logged. |
| Mana abilities | A mana ability of the shape `mana_shape` reads is not put on the stack: `ActivateMana`, or the tap of a payment, does the cost, adds the mana and runs the effects that follow in one batch. |
| Target recheck | On resolution each target is tested again with its spec (same object and zone, selector matches, not protected). If every target is illegal the object does nothing (`ResolutionEnded { Fizzled }`, CR 608.2b); otherwise it ignores the illegal ones, except one an earlier step of the same resolution moved, which stays named as it was (CR 608.2h). |
| Journal | `GameState.resolving` holds the object, a `position` in the effect tree, bindings, snapshots and the journal. `step(&GameState)` never mutates and returns `Done(events)` (ending with `StepDone`) or `Ask(decision)`; an answer is logged as `ChoiceMade` and journaled, and the step runs again to consume it. Random numbers come from the seeded substream, whose counter moves only when events fold. |
| Steps | One effect is one step. `If`, `May`, `Pay` and `Modal` are entered by position (`resolve/tree.rs`). The condition of an `If` is read in the state the earlier steps left. |
| Events | Each primitive without an event of its own logs one, with a fold arm and a redaction arm in the protocol's `redact`: library peeks, searches, scry and surveil are private to their seat, reveals public, and `ZoneChanged`, `ChoiceMade` and `StepDone` reach a seat whole only when it sees every card they name. |
| Group effects | Destroy, exile, tap, untap, counters and similar effects act on all objects a `Ref` names together, in its order, in one step; a name of an object that changed zones since (CR 400.7) or repeats is passed over. |
| Values | `Scope` in `resolve/value.rs` reads `Ref`, `Selector`, `Amount` and `Condition` from the state, bindings, choices and snapshots, changes nothing and refuses by name what it cannot read. `X` is the announced value, 0 where none was chosen (CR 107.3). |
| Let | `Let { as, amount }` fixes the amount when its step runs; the name is readable only after its `Let` in the same ability, not in a reflexive or delayed trigger. |
| Nothing to choose | An attack or block with no legal option is not asked (`NothingToDo`), nor a choice of cards with no more candidates than must be taken. Another decision with one legal answer and no "may" is answered by the engine unless the seat has Full Control; an unpayable payment is still asked with No alone, so the pace shows nothing. |
| New primitive | `resolve/prim/<effect>.rs` holds one file per `Effect` variant. A task replaces its stub and adds its name to `RUNS` in `caps.rs` and to the list of `the_capabilities_name_exactly_the_effects_that_run` in `resolve/prim/mod.rs` (skipped by the fast check: run `cargo test -p mtg-engine --lib`). |

### Decisions the effects need

`Decision.player` names who answers, so a decision may go to any seat. Every effect decision carries `source` (the object that asks). Decisions for a seat that is not the active player have a response clock (`server.md`). The kinds are in `mtg-engine/src/choose/`; a new decision comes with its web prompt in the same piece.

### Snapshots (last known information)

`Snapshot` (`snapshot(state, id)`) is kept only in a stack ability (its source and a trigger's cause objects, at detection, CR 603.10a), a resolution (each object a step moves off the battlefield or stack, in `StepDone`), `Choices` (X, which outlives the spell) and the turn's logs (judged by a `Tally` filter as it was, CR 608.2h). It holds `characteristics` with every continuous effect and the `keywords` after layer 6. `Amount::Stat`, `Amount::Counters`, `ControllerOf`, `Matches`, `HasKeyword` and `HasCounter` read the live object while its counter matches, else the snapshot. A spell leaving the stack is kept for the turn as a `KeptSpell`, so `CopySpell` of a spell returned to hand copies it as it last was (CR 608.2h, 707.10).

## Triggers, replacement and layers

| Topic | Rule |
|---|---|
| Detection | After a batch is folded, `triggers::detect(pre, events, post)` returns the triggered abilities (`AbilityTriggered`). A move off the battlefield is judged against `pre`, the abilities and objects as they were (CR 603.10a), so permanents leaving together see each other go; every other event against `post`. An ability works from the zone its `from` names (the battlefield by default). |
| State triggers | `TriggerPattern::State { condition }` (CR 603.8) is looked for where a player would receive priority, after state-based actions; it triggers when its source is in its `from` zone, the condition holds and no instance of that ability of that object is queued or on the stack. |
| `Batch` | `Batch(pattern)` triggers once for the events of one `detect` call that match (once per player they are about; counters on a single subject, `CountersPut.each`, once per permanent; damage only when `to` names players). "Whenever ~ is dealt damage" and "Whenever ~ deals damage" are a `Batch`; plain `DealsDamage` to one object is "Whenever a source deals damage to ~". |
| `Nth` | `Nth { event, nth }` triggers on the nth matching event of the turn, counted by the fold; over a damage `Batch` it is the first batch of the turn to the object, over a discard `Batch` the batch that holds the player's first discards of the turn of its kind (`TallyEvent::Discarded`; `nth` 1 only). |
| Clash result | `Did { action: Clashed }` triggers on every clash of the player; its `Amount::Triggering` is 1 after a win and 0 after a loss, so "If you won, S." is `If { Compare { Triggering, Ge, 1 } }`, written only in a trigger on a clash. |
| `Caused` | `Caused { inner, by, source }` wraps an event or `Batch` and matches only when a spell or ability caused the batch (`cause_of`: the resolving one, else the one being cast or activated, else the damage source), by a player `by` names and a source `source` selects; turn and state-based actions have no cause. |
| Turn log | The fold records damage dealt, attacks, blocks, activations and counters put (`turn_log.rs`) for one turn, derived from events. `Pred::DidThisTurn { event, by, to, combat }` selects the objects that did the event; `Amount::Tally` over `Damaged`, `Attacked`, `Blocked`, `Activated` and `CountersPut` counts the records (damage and counters by amount). A record names its object only while it is public; a hidden one counts but no filter selects it. |
| This turn's casts, after combat | `Pred::NthCastThisTurn(n)` is the nth spell cast this turn by any player, read from the log of casts; `Condition::AfterCombat` holds from the postcombat main phase on. |
| Queue and order | `GameState.pending_triggers` fills at detection. Before a player would receive priority (CR 117.5, 603.3) the active player's triggers go on the stack, then each other player's in turn order (CR 101.4); a controller of two or more asks `OrderTriggers` once unless all share source and ability. Each chooses targets as it goes on or is removed. The sweep repeats until none is pending. |
| Intervening if | `intervening_if` is checked when the event happens, against the pattern's state, and again as the ability begins to resolve; if false then, it ends `Lapsed` (CR 603.4). |
| Delayed and reflexive | `Effect::DelayedTrigger` adds a `Delayed` to `GameState.delayed` with the resolution's bound names; it triggers on the next matching event (`once`) or lasts until end of turn, and with `targets` of its own chooses them each time. `Effect::Reflexive` ("when you do", CR 603.12) queues its trigger, targets chosen then, when the step before it was done. |
| Paid any number of times | `PayAnyTimes` asks how many times (0 to the most the player can pay) and pays the cost that many times at once; a `Reflexive` right after it triggers only after a payment, with the number of payments as its `Triggering` ("that many", "up to that many target ...", Adversaries). |
| Still resolves | `StillResolves` among a trigger's steps keeps CR 608.2b from stopping it when every target became illegal; the steps then find no target (Gilded Drake). |
| Reselected attacks | `Reattack` lets the controller choose again what each named attacker attacks among what its controller could attack, ignoring attack requirements and costs; `AttackRedirected` logs it and nothing triggers on attacking (Portal Mage). |
| Replacement step | Every batch passes through `replace::process(state, proposed, journal)` in `Engine::commit` before the fold, which returns the events that happen or a question. A replacement applies at most once to an event and its replacements (CR 614.5). When several apply, the affected player picks (`ChooseReplacement`, CR 616.1); one about its own object applies first unasked (CR 614.15). A batch's answers are its journal, so `process` is pure. |
| Countered spells | A shield over a spell countered this way sends it to exile (`MoveTo`) or to the top or bottom of its owner's library (`IntoLibrary`), only when it is countered (CR 701.6). |
| Divided shields | A `PreventCounted(Divided)` shield over damage to targets begins one shield for each target with its announced share (CR 601.2d, 611.2c); a target given nothing gets none. |
| Counted prevention with effects | `PreventCountedAnd { most, effects }` prevents up to `most` in all, counted as it begins, and runs `effects` after each event it lessens with the damage prevented as `Triggering` (so "that creature" is `Triggering`, not a target). |
| Token replacements | `Multiply` and `Add` scale the run of tokens one effect makes for a player; `Instead` replaces it ("that many Angels", "one of each" over a kind named by `AnyOf`); `Plus` keeps it and its `CreateToken` steps join it as one event, so later replacements see every token ("those tokens plus that many Squirrels"). |
| Forms as it enters | `ReplaceBy::BecomesChoice` ("it becomes your choice of a 3/3 creature, a 2/2 creature with flying, or ..."): the controller picks a form as the permanent enters and the form joins its copiable values as a copy effect of its own values (CR 707.2), so a copy of it is that form; a form holds only changes a copy effect makes (`r303_forms.rs`). |
| Milling as it enters | `ReplaceBy::Mill(amount)` ("As this creature enters, mill three cards for each time it was kicked"): the milled moves join the enter event's batch, each a move of its own (`r303_mills.rs`). |
| Rules of the game | `Effect::Rule` (with a duration) and `Ability::Rule` (while its source has it) change a rule through a `GameRule`. A `SpellCost` rule with `from: Command` also works from its owner's command zone (eminence). |
| Derived statics | `layers.rs` applies the resolved effects and the permanents' `Static` abilities layer by layer to every permanent at once, so a static's `affected`, `condition` and amounts read the others as the earlier layers left them. A static applies in the layer of each modification while its source is in the zone its `from` names and its `condition` holds. |
| Locked or live | A resolving spell or ability that modifies characteristics or control fixes its object set when it begins (CR 611.2c): `Effect::Continuous` evaluates `affected` once and works out its amounts then. A `Static` evaluates its selector at every derivation. |
| Timestamps | A static ability takes its permanent's entry time, or the time of the effect that gave it when that is later (CR 613.7a); a resolved effect takes its creation time (CR 613.7). Both are counters assigned by the fold. |
| Durations | `UntilEndOfTurn` ends in the cleanup step in the batch that removes damage (CR 514.2), so a creature a pump kept alive survives it; `UntilEndOfCombat` after the end of combat step; the others follow their named source or counter (`Duration` in `modify.rs`). |
| Dependencies | Within a layer an effect follows those it depends on, otherwise timestamp order after the layer's characteristic-defining abilities (CR 613.3). A depends on B when B changes whether A exists, what A applies to or what A does (CR 613.8a), found by trying B on copies; a loop goes by timestamp (CR 613.8b). |
| Characteristic-defining | A `Static` with `defining: true` (CR 604.3) has `affected` `This`, no `condition`, no `from`, and works in every zone; on the battlefield it applies first in its layer. |

### Copies

A copy effect gives an object the copiable values of another (`CreateToken` with `CopyOf`, `Replacement` `AsCopy`, `BecomeCopy`, `CopySpell`, `CopyAbility`). A copy is a new object; a spell copy has the original's modes, X, optional costs, targets and current face. Storm and Demonstrate are keyword triggers that copy; the engine side is in [engine.md](engine.md) (copies).

## Keywords

| Item | Rule |
|---|---|
| Data | `{"Keyword": {"Ward": "{2}"}}`. `Keyword` is an enum in `keyword.rs`. |
| Engine | `keywords/mod.rs` maps each variant to a `KeywordRules` value in `keywords/<name>.rs`; an exhaustive `match` flags a forgotten keyword. A keyword returns abilities (`adds_abilities`) or answers a hook (`restricts_blocking`, `silences_players`, `answers_evasion`, ...); one with no rules yet changes nothing. |
| Payment keywords | Convoke, improvise and delve have no hook: the payment reads the keywords. |
| Keyword macros | `mtg_cards::macros` holds two tables of JSON templates with `$N` (the number) and `$ON` (the object): `KEYWORDS` expand into abilities, `ACTIONS` into effects. `Keyword::Macro { name, amount }` adds its entry's abilities through `keywords/macro_keyword.rs`, and the validator walks the expansion. A keyword or action that templates existing nodes is one `Entry`, not an engine file; the renderer writes an action back by matching the entry. `Keyword::MacroX { name, x }` puts an `Amount` in place of `$N`. |
| Dungeons | A dungeon is a file under `crates/mtg-cards/dungeons/`; the check refuses a dungeon with no room, a doubled name, an arrow up or to no room, an unreached room or a last room with an arrow, and validates each room ability. |
| Storied | `Keyword::Storied` gives its controller an enduring story for the rest of the game once they control three or more artifacts, legendaries and/or Sagas, checked with the state-based actions like ascend (`rmt6_storied.rs`); `Condition::EnduringStory` reads it. |
| Draft rules | `GameRule::Draft(DraftRule)` holds a rule that works only during a booster draft ("Draft ~ face up."); it changes nothing in a game and is written without `from`, as `AnyNumberInDeck` is. |
| Siege | A Battle with the subtype Siege has two rules by that subtype (`assemble::siege_rules`, CR 310.11), which no rules file repeats. |
| Cumulative upkeep with an action | `CumulativeUpkeepActs { acts, payable }`: each upkeep adds an age counter, then the controller does `acts` once per age counter (each pass binds its own names) or sacrifices the permanent; the actions are offered only while `payable` holds (CR 118.3, `r282_cumulative_acts.rs`). |
| More Than Meets the Eye | `MoreThanMeetsTheEye(cost)` on a transforming double-faced card's front: the back face is cast from hand for the cost as an alternative cost (`CastWay::Alternative`), as disturb does from a graveyard, and enters converted (CR 702.162, `r282_more_than_meets.rs`). "Convert" is `Effect::Transform`. |
| Compleated | The cast keeps the life paid for Phyrexian symbols (`CostTaken.phyrexian_life`); a planeswalker with `Compleated` enters with two fewer loyalty counters per such symbol (CR 702.150, `r282_compleated.rs`). |
| Suspend X | `Suspend { time: 0, cost }` with `{X}` in the cost (`XAtLeast` for the least X): suspending asks X (`ChooseNumber`), costs X more per `{X}` and puts X time counters on the card (`ways::suspend_x_of`). |
| Plotting by an effect | `Effect::Plot { what }` exiles cards from a hand, graveyard, library or the stack face up as plotted (CR 702.170d): the owner casts each free as a sorcery on a later turn. |
| Cleave | `Cleave { cost, spell }`: the keyword carries the `Spell` ability without the bracketed words, which a spell cast for the cleave cost announces and resolves instead of the printed one (CR 702.148); an `{X}` of the cleave cost is announced for that cast only. The compiler and the read back check each version against its text. |

## Validation

Errors name the file and the path inside it; all are collected.

| Level | Checks | Runs in |
|---|---|---|
| 1 Parse | Syntax, types, unknown fields, unknown variants | `Library::from_sources`, `load_dir` |
| 2 Structure | File name is the slug; oracle id is a UUID; ids and names unique; face names unique and joined by ` // ` to the name; `impl_status` fits the abilities; a fingerprint on every face of a `Full` or `Partial` card; clause spans non-empty and disjoint; `Target(i)` in range and used; `Bound` bound earlier on the path; value nodes only where their context supplies them | `validate.rs` |
| 3 Printed | Oracle id, name, face count and names; fingerprints (drift); Scryfall keywords present on some face as an ability, a grant or inside a `Manual` clause; `X` against the printed cost; ability kinds against the type line | `Library::check_generated` and the file tests |

## Compiler and Manual fallback

- The oracle-text compiler (`compile/`) writes `Card`s with `mtg_cards::load::canonical`, validated by the same functions, with the fingerprint of the text it read.
- `compile::compile(&PrintedCard) -> Compiled { card, clauses }` is pure: reminder text and the card's own name (`~`) are stripped with a map back to the original text, and the text splits into clauses. Templates live in `compile/read/`, one module per family of wording, each with goldens and near-misses in `compile/read/tests/`.
- Every reader has a writer (`compile/read/render/`). The checker accepts a proposal only when the writer reproduces the printed clause; wordings only the checker needs live in `render/oracle_only/`.
- A last-resort pass (`written_memo::or_last_resort`) writes shapes whose wordings would hide another writer's only for an ability nothing else writes, so a known wording never turns into `Differs`.
- The object of the event as the object of a step ("destroy it", "counter that spell") is written from the twin acting on the card, its "~" renamed after the trigger's head (`wf25_writers`); "it" stands only where nothing else names the card, and "that <noun>" only for a noun the head says.
- Many shapes are written from a twin another writer words, with one part changed (`wf26_writers`, `wf27_writers`, `r280_steps`, `oracle_only/r280_wordings`; e.g. a card chosen from a revealed hand, never a "permanent"). Two steps joined by "and" read the same in either order, and an ability name of up to forty letters is flavor to the checker as to the compiler.
- A stand-in head that names the recipient of the damage never keeps "it" for the card.
- A payment of any amount (`l31b_any_amount`) is a `Pay` whose cost is `{X}`, X life or X energy; the payer announces X as it resolves and the paid steps read it ("that much", "that many", "for each {E} paid this way"). `RememberValue.per` and `Amount::Remembered.per` keep the amount for each player apart ("the amount of mana they paid this way").
- `tests/it/compile_corpus.rs` is the ratchet: every ability the compiler reads equals one of the hand-written file (mode labels aside); `reproduced.txt` lists the cards it holds for.
- Hand-written files win over generated `card_rules` rows (oracle id, JSON, status, fingerprint, compiler version) for the same oracle id; `Library::from_sources` loads files, `Library::with_generated` rows. `generated::generate` compiles a card and stores it only if `Library::check_generated` passes; `mtg-cli compile [--all | --card <name>]` writes the rows ([data-sources.md](data-sources.md)).
- A line the compiler cannot read becomes `Ability::Manual { reason, span }` and the card `Partial`; if nothing is read the card is `Manual`. "That much" and "that many" name the amount of an event (`Amount::Triggering`), so only a triggered ability reads them.
- A card the engine does not run whole is cast and resolved without the clauses it leaves out, which its controller does by hand (`engine.md`, Manual actions).
- `coverage::analyze(cards, caps) -> Report` judges each distinct card of a deck list with the game's support function; `mtg-cli coverage [--deck <code or name part>] [--top <n>] [--all]` runs it over the stored precons (or every Commander-legal card) and ranks the blockers. `mtg-cli audit [--count N] [--seed N] [FILE]` lists a seeded sample of compiler readings beside the oracle text.

## Proposals and drafts

A proposal is a rules file for a card from outside the corpus: the Forge translator, `propose` or a person. A deterministic checker accepts it only when it is valid, reads back as the printed text and has a support state. The printed text is the only ground truth.

| Check | How | Result |
|---|---|---|
| Repair | `proposal::repair` fixes the mechanical slips of the JSON before it is parsed | `Report::repairs`, one entry for each fix |
| Valid | `Library::check_generated`: levels 1 to 3 (Validation) | The problems, each with its path |
| Reads back | The clauses of the printed text (`compile::compile`) are paired with the abilities by kind; a wording of `written_variants` equal to the clause accepts it | The pairing, with the clauses that differ or lack an ability |
| Support | `support` with `mtg_engine::caps()` | `Verified`, `Partial` or `Manual`, with the blockers |

The verdict is `Invalid` (a problem of the first check, or an ability no writer reads back that uses a primitive the engine lacks, so a draft never does nothing), `Mismatch` (a clause differs or an ability has no clause), `Unchecked` (valid and nothing contradicts the text, but a construct has no writer) or `Accepted`.

- `mtg_cards::proposal::check(json, &PrintedCard, &Library, &names, &Caps) -> Report`; `mtg-cli check-rules [--card NAME] [--bundle] [--verbose] [--write DIR] [--gaps FILE [--marks]] FILE...` runs it over files.
- A proposal may leave out the oracle id, the names, the status and the fingerprints: they come from the card, and the status is derived from what is modelled.
- Repairs fix a slip only when the file and the printed card leave one reading; a guess is never repaired (which face of a multi-face card, which token size), the clause goes to the table instead.
- `mtg-cli promote [--dry-run] [--partial] [--out DIR] FILE...` makes corpus cards (rules file plus cached record as fixture) of accepted proposals, a `Partial` one only when each `Manual` clause is a whole sentence (`Span::is_whole_clause`). `promote --draft [--source forge] [--differs] FILE...` drafts an `Unchecked` proposal (or a matched Forge `Mismatch`, `--differs` marking it) behind the smoke gate and the validator.
- `check-rules --gaps FILE` writes the ledger of everything that holds the checked proposals back as TSV (kind, reason, blocked, freed, weight, examples), ranked by the popularity of the cards each gap alone holds back; `--marks` adds the players' fix marks. The ledger ranks the items of [cards-backlog.md](cards-backlog.md).
- `mtg-cli propose` asks a model through the Anthropic API for the rules of cards and keeps the proposals the checker accepts; it needs `ANTHROPIC_API_KEY` and costs money (see Paid runs; `--dry-run` sends nothing).

### Paid runs

A `propose` run is capped, priced and accounted before it is cheap.

- `--max-spend USD` is required: a request is not sent when the ledger plus the worst cases of outstanding requests and its own (all input as cache writes, all of `max_tokens` out) would pass the cap. `api-ledger.tsv` in the cache directory gets a line per billed request from the response's `usage` and counts toward every later run; a timeout counts at its worst case. Prices are in `propose/price.rs`; an unpriced model is refused. `--model opus|sonnet|haiku` names the current model of that family.
- The instructions and DSL reference are one cached system block; a run's first request goes alone so the others read the cache. `--reasoning low|medium|high` sets `output_config.effort` (thinking bills as answer tokens). `--dry-run` prints the expected cost per card and, with `--max-spend`, how far the cap goes.
- `--batch` uses the Message Batches API at half price: pieces sized to the cap (`--batch-size`), polling (`--poll-every`), results settled into the ledger when read (an outstanding batch counts at its worst case), a repair batch after the first, and `batch-state.json` so `--resume` collects it and nothing is billed twice.
- The key comes from `ANTHROPIC_API_KEY` only and is never printed, logged or written; errors redact it. An optional `ANTHROPIC_WORKSPACE_ID` is sent as `anthropic-workspace-id`.
- A card ends as an accepted proposal (`<slug>.json`), a draft (`drafts/<slug>.json`; `promote --draft --source sweep` or `--promote` makes it play Unverified) or a failure (`rejected/`). The model may list needed engine pieces in `missing`, which the checker never sees. `records/` holds a record per card and `summary.tsv` a line each (card, outcome, missing_engine_piece, requests, usd, detail).
- `--cards FILE` takes names one a line (`#` comments). `mtg-cli propose-order [--out FILE] [--limit N] [--lines DIR]` writes a run order: saved-deck cards first, most used first, then EDHREC popularity; only Commander-legal paper cards without rules in the repository. `--lines DIR` keeps cards whose translation in `DIR` has one or two gapped lines.

### Line fill

`propose --lines --names-file FILE --translations DIR --forge-hint FORGE` has the model write only the gapped lines of a Forge translation; the translator's proven lines stay as they are.

- A card qualifies with one or two gapped lines, no partly translated line, Commander legality and a script. Each line is one request under 8k tokens: the oracle text, the line and its script line with reached SVars, the gap key, the proven abilities, the 4 nearest solved lines of its kind and a focused DSL reference of at most 15 kB. The system prompt is cached per kind.
- The model answers one ability, or `{"missing": "..."}` when the DSL cannot say the line exactly, which fails the card with that note. The parts are merged and the checker judges the whole card; a rejected card gets up to two repair rounds with the checker's words, asking again only a line that is not an ability.
- `--samples N` asks each line N times in the first round; the best is kept and `summary.tsv` says in `agree` whether all agreed. `--schema` sends a kind's schema as structured output when its types are not recursive (every kind nests effects, so the checker alone reads the answer).
- Paid runs take `--max-spend`, the ledger, `--reasoning`, the workspace header and `--batch` (`lines-batch-state.json` for `--resume`) as above; `--dry-run` prints eligible cards, skip reasons, request sizes and the cost per card per priced model. A local server takes `--provider openai --base-url`.

### Drafts

A whole card the checker cannot prove goes to the players instead of waiting for a writer (Read tier).

- `cargo test -p mtg-cli drafts::` checks every draft against its record: valid, verdict `Unchecked` (or a matched Forge `Mismatch`, through a `Differs` clause exactly when marked `differs`), whole, fingerprints current, and the smoke gate.
- `mtg-cli` embeds the directory at build time. `compile` makes each draft its card's current one in `card_drafts`, rewrites a stored one whose meta (source, match, `differs`, proof) changed, and retires those it no longer holds.
- `mtg-cli coverage --deck CODE` lists the cards that are neither `Verified` nor `Read`; `mtg-cli draft-suspects` finds drafts likely to be wrong by setting the engine's English against the Oracle text, read-only.
- `mtg-cli semantic-audit` runs structural checks (`semantic_audit/checks.rs`) between each clause and its abilities over the corpus, the drafts and the newest translator run's offered drafts ("if", "for each", "may", targets, durations, filters, numbers, "instead", `unclaused`) to a TSV ranked by check and EDHREC rank, read-only. `--min-precision` keeps the checks whose precision, measured on a hand-judged sample, reaches it.
- `mtg-cli mutation-score` measures those checks and the checker's read back on single-fault mutants of `Full` corpus cards (`mtg_testkit::mutate`: a number, a condition, "may", "another", a filter, you and opponents, a duration, an ability): the share of mutants each catches, per kind.
- A "reads wrong" verdict adds a fix mark with its note (`mtg-cli fix-marks`, `reads`); the fix is a new draft (a new `read_hash`), which is read again.

### Forge translator

`mtg-cli translate-forge DIR [--cards NAME...] [--stored] [--out DIR]` (`crates/mtg-cli/src/translate/`) writes proposals with no model from the scripts of a read-only Forge checkout (nothing is copied into the repository); `promote --draft --source forge` drafts them. `mtg-cli forge-check --forge DIR` sets the stored precon cards against the scripts and lists the verbs and wordings the rules lack.

- A construct the translator cannot express makes the script a gap and is never approximated; a partial translation records its gaps in its meta.
- Filters (`translate/filter.rs`): a valid-string splits into alternatives, a base and properties (`alt ("," alt)*`, `base ("." prop ("+" prop)*)?`; ", " inside a `named` word stays in the name), read by `valid.rs`, `valid_grammar.rs`, then the atom table (rows with legal scopes and a probe card); an atom of `valid_grammar::UNMAPPED` is named by its row and unknown words are reported. `translate-cards.tsv` ends with `offered` (`--stored`, `whole` cards): whether the exact check accepts the draft as `Verified` with no `Differs` clause (else the gap `offer:readback:Differs`, clauses in `translate-differs.tsv`).
- A key a mapper lists as understood must be read: an unread listed key makes the line a gap `key:<api>.<Key>-unread`.
- Memory plumbing is two tables: `translate/remember.rs` gives per Forge API what each `Remember*`/`Imprint*` key writes; `translate/refs.rs` maps each `Defined$`/`RememberObjects$` word to a reference with legal scopes and a probe card: `Now`, `Fixed` (an `Effect`'s statics, CR 611.2c), `Lasting` (its triggers and shields: only words naming the same objects throughout), `Reflexive` (never a target an earlier step may have moved) and `Imprint`. A word outside its scopes stays a gap.
- A `Triggered*` word names what the engine holds for the trigger's mode (`translate/triggered.rs`): `Triggering` (the event's subjects), `Dealer` (the doer), `Against` (what an attacker attacks or a blocker blocks), `Arrived { of, zone }` (the card `of` became one zone change on) and `Departed(of)` (the object before its last move, so `ControllerOf(Departed(x))` is its last known controller); each names nobody once its record is gone. `ZoneKind::Outside` finds nothing without a sideboard.
- `Amount::CastLastTurn` reads the seat summaries kept as each turn ends (`turn_summary.rs`): the most spells one named player cast in the turn just ended (the werewolf upkeep words).
- `Permanent` as a card filter outside the battlefield is a card of a permanent type, never an instant or sorcery. `NoResolvingCheck` with a condition is a gap, because the engine checks the condition.
- `proposal::merge` joins a trusted proposal that leaves clauses by hand with the compiler: per clause the compiler's ability wins, else the translator's; a clause neither reads stays `Manual`, so no clause is read twice or lost.

### Partial translations

A Forge translation with gaps runs the lines it proves and leaves the others to the players, and no printed clause is lost. The card plays in state `Partial`: its automatic lines run as a `Forge` card, the others are `Manual` clauses.

- `mtg_cards::proposal::partial::prove(&Report, &[LedgerLine])` is pure and gives each printed clause one row, `Auto` or `ByHand`, or refuses the card by name (`InvalidRules`, `OracleDiffers`, `Stray`, `GapInsideLine`, `UncoveredLine`, and others in the promote report).
- The ledger: `translate-forge` writes `lines` in the meta, one `LedgerLine {face, tag, text, abilities, gap}` for each `K:`, `A:`, `T:`, `S:` and `R:` line of the script.
- A card with no line proven is still `partial` (an empty proposal and its ledger, for line fill) when every gap stops one script line; a whole-card gap (`keyword:<word>-unused`) keeps it `skipped`. A card whose every clause is by hand compiles `Manual` and is never drafted.
- Totality: a property over every stored card says the spans of `compile(printed).clauses` and the reminder text cover the whole oracle text, so a clause cannot fall between rows.
- `ByHand`: a `Manual` ability whose span equals the clause's span. `Auto`: the clause is `Reads` (the renderer writes it back whole), or `Unwritten` or `Differs` with a witness: `oracle_matches` and every content word of the clause in the text of the translated line.
- `promote --draft` takes a proposal with gaps only with `oracle_matches`, a proof and the smoke gate, and stores the proof and ledger in `drafts/meta/<slug>.json`.
- A by-hand line is a `ManualClause` the engine asks nothing about, so it cannot hold a game; when the compiler reads a trigger line's event, `ManualClause.when` holds its pattern (outside the recipe fingerprint). A failed automatic line is `AbilityLeftToTable` and the server's `wrong`. The card preview marks each line "Runs automatically" or "You do this by hand" ([web.md](web.md)).
- A partial draft is an ordinary draft with a `read_hash`, so the Read tier's loop holds; the fix mark follows the draft a card plays.

A whole by-hand line is safe only when the automatic lines behave the same with or without it; anything not listed as safe is refused:

| By-hand line | Safe | Why |
|---|---|---|
| Activated or loyalty ability | Yes | The player uses it when they choose; nothing automatic waits on it |
| Triggered ability, whole | Yes | Its effect is one act of the players, its own "otherwise" and "if you do" included |
| The whole text of a spell, with only keywords or other faces automatic | Yes | The spell resolves with nothing automatic in it, then the by-hand window |
| A spell line beside a spell line that runs | No | One resolution: order and "if" cross the line |
| Static ability, anthems included | No | The engine's combat, targeting and costs run without it |
| Restriction: "can't", "must", "as though", "unless", "only" | No | The engine enforces the opposite |
| Replacement or self-replacement: "instead", "would", "prevent", "skip" | No | The event has already happened when players could act |
| Cost: alternative, additional, "without paying", cost changes | No | A cast by hand does not run the spell's automatic lines |
| Keyword | No | A keyword is a rule of combat or casting that the engine would need to run |
| Entering: "as ~ enters", "enters with", "enters tapped" | No | The rest of the card reads what it sets |
| Crossing clauses: "chosen", "named", "noted", "exiled with", "the exiled", "imprint", "remembered", "secret" in a by-hand line, or in an automatic line beside one | No | Lines share a memory the players cannot see |

## Worked example: Archon of Cruelty

```json
{
  "oracle_id": "aa1a6646-c1e6-4bff-9092-43ee3e137914",
  "name": "Archon of Cruelty",
  "impl_status": "Full",
  "faces": [
    {
      "name": "Archon of Cruelty",
      "oracle_fingerprint": "b68d063ee3b4ff033c607c00f4b59695",
      "abilities": [
        { "Keyword": "Flying" },
        {
          "Triggered": {
            "when": {
              "AnyOf": [
                { "ZoneChange": { "what": ["This"], "to": "Battlefield" } },
                { "Attacks": { "what": ["This"] } }
              ]
            },
            "targets": [{ "what": "Opponent" }],
            "effects": [
              {
                "Sacrifice": {
                  "who": { "Target": 0 },
                  "filter": [{ "AnyOf": [{ "Type": "Creature" }, { "Type": "Planeswalker" }] }]
                }
              },
              { "Discard": { "who": { "Target": 0 } } },
              { "LoseLife": { "who": { "Target": 0 }, "amount": 3 } },
              { "Draw": {} },
              { "GainLife": { "amount": 3 } }
            ]
          }
        }
      ]
    }
  ]
}
```

This is `crates/mtg-cards/cards/archon_of_cruelty.json`; a test keeps the two equal. Oracle text: "Flying. Whenever this creature enters or attacks, target opponent sacrifices a creature or planeswalker of their choice, discards a card, and loses 3 life. You draw a card and gain 3 life."

| Feature | In the file |
|---|---|
| One ability, two events | `when: AnyOf [ZoneChange to Battlefield, Attacks]` |
| Target opponent | `targets: [{ what: Opponent }]` |
| Actors differ | `who: Target(0)` for the first three effects; the default `You` for the last two |
| Choice by the target player | `Sacrifice` filters the target's own creatures and planeswalkers; the target player chooses |

At run time the trigger is detected after the event, goes on the stack with its target chosen by the controller and is rechecked on resolution. `Sacrifice` and `Discard` ask `ChooseCards` of the target player; the other steps ask nothing. Triggers caused by the sacrifice, discard and life loss go on the stack at the next sweep (CR 603.3b).

## Corpus

`crates/mtg-cards/cards/` holds the corpus cards and the predefined tokens: the hand-written reference set, every ability covered by a scenario test or compiler golden. The cards exercising a mechanic are found by grepping the rules files for the node (`grep -l '"Mutate"' crates/mtg-cards/cards/*.json`) and in the tests of `mtg-cards` and `mtg-testkit`.
