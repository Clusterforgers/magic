# Card DSL and effect pipeline

Rules data for cards (`mtg-cards`), how it is checked and produced, and the rules of the pipeline that runs it (`mtg-engine`). The types are documented in `crates/mtg-cards/src` (`ability.rs`, `effect.rs`, `trigger.rs`, `modify.rs`, `cost.rs`, `value.rs`, `keyword.rs`); this file holds what the code does not say. Related: `engine.md`, `protocol.md`, `deck-check.md`.

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
| D1 | Format: JSON, one file per oracle card. Serde's externally tagged enums, PascalCase variants, snake_case fields; unknown fields are errors; defaults are omitted. Short forms: a mana cost is a string (`"{X}{R}"`), a fixed `Amount` a number, a mana-only `Cost` a string | The database stores JSON, the compiler emits data, the web reads it, errors carry a path (`serde_path_to_error`) | RON (no jsonb, no TypeScript reader), YAML and TOML (typing traps, awkward trees), Rust builders (not data), a text script (needs its own parser) |
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
| D14 | Read tier: a whole draft the checker cannot prove (`Unchecked`) runs automatically once one signed-in player read it beside the engine's own English and found it correct, and by hand after one "reads wrong"; verdicts are stored per rules hash, the state is still computed (D6) | The checker has no writer for many valid rules; a human read is fast when the engine's reading is shown, and `Verified` keeps its proof | A stored per-card flag, drafts in the corpus (the ratchet and `NOT_VERIFIED` stay about `Verified`) |

## Files

| Path | Content |
|---|---|
| `crates/mtg-cards/cards/<slug>.json` | Rules of one oracle card |
| `crates/mtg-cards/cards/tokens/<id>.json` | A predefined token (Treasure, Clue, Food, Gold, Blood, Junk, Powerstone, Vibranium, Map, Mutagen) |
| `crates/mtg-cards/tests/fixtures/corpus/<slug>.json` | The imported Scryfall object of one corpus card |
| `crates/mtg-cards/tests/fixtures/reproduced.txt` | The ratchet list: the corpus cards the compiler reproduces, one name per line, sorted |
| `crates/mtg-cli/drafts/<slug>.json` | A draft (Read tier); its card record is `drafts/fixtures/<slug>.json`, its meta `drafts/meta/<slug>.json` |

- `mtg_cards::builtin::library()` is the library of the corpus files, embedded at build time (`build.rs`), so a server needs no directory and a deploy carries the rules it was tested with.
- Canonical text: compact JSON in serde's field order, laid out by `mtg_cards::pretty` (a value stays on one line up to 100 columns, otherwise one entry per line).
- `MTG_UPDATE_CARDS=1 cargo test -p mtg-cards corpus::` rewrites the files and their fingerprints; the test fails on a file that is not canonical.
- `cargo test -p mtg-cards hand_written::` checks every rules file as a file of its own (`Library::check_generated`) against its fixture record and against the newest `oracle-cards-*.jsonl.gz` of `MTG_REAL_DATA_DIR` or the `mtg-cli` cache; without a download it checks the fixture only and says so. An erratum fails the card it changed: fix the rules, or refresh the fingerprint when they still fit.
- `cargo test -p mtg-cards fixture_real::` compares the fixture records with the same download; without one it says so and passes.

## Adding a corpus card

No test holds a count of cards, sources, compiled sources, reproduced cards or fingerprints; each is derived from the card directory and the fixture, and a test asserts what the number stood for. A new card needs:

1. Its rules file `crates/mtg-cards/cards/<slug>.json` (the slug is the card's name lowercased with every run of non-alphanumerics as `_`, `mtg_cards::load::slug`). A `Full` or `Partial` face starts with an empty `oracle_fingerprint`.
2. Its Scryfall object as the new file `crates/mtg-cards/tests/fixtures/corpus/<slug>.json`, with the same slug: pretty-printed JSON of one object and a final newline (a multi-face or split card is one file named after the whole card name). The rules file and the fixture file share the `oracle_id`, and each card has exactly one of each. No other file changes for this step, so branches adding different cards never conflict here.
3. `MTG_UPDATE_CARDS=1 cargo test -p mtg-cards corpus::` writes the rules file in canonical form with the fingerprints of the fixture's text and adds every card the compiler reproduces to `crates/mtg-cards/tests/fixtures/reproduced.txt`. It never removes a name: a listed card that stops being reproduced fails.
4. `cargo test -p mtg-cards -p mtg-testkit`. A card the engine plays whole needs no row. A card it does not (Manual or Partial) is a row of `NOT_VERIFIED` in `crates/mtg-testkit/tests/it/corpus_cards.rs` with its state and the number of abilities it carries; a Verified card carries the abilities of its file, and each basic land type adds its mana ability.

`reproduced.txt` is sorted, one name per line, and the update inserts a name at its sorted position. Two branches that add different names conflict only where the names are adjacent; the resolution keeps both lines, and the update command restores the order if it is off.

The tests report a card without its record or file, a fixture file that is not named after its card or lacks the final newline, two files with one oracle id, a stale or missing fingerprint, an unlisted reproduced card and an unexpected state with the card's name, the file to add and this section. `tests/it/adding_a_card.rs` adds a card to a copy of the directory and the fixture directory and shows that no count or table needs an edit.

## Printed side and rules

The printed side of every card comes from the import record (`mtg_import::CardRecord`, stored in the `cards` table). The rules file adds abilities. A deck of oracle ids becomes `CardDefinition`s in the server (`CardSource`, `DeckRef::Oracle`):

| Step | Action |
|---|---|
| 1 | `mtg-db` reads one `cards` row per distinct oracle id of the deck (`CardRepo::get_many`); every id without a row is named in the error. |
| 2 | The server adapts each stored row to `mtg_cards::PrintedCard` (`mtg_server::printed_card`; strings only, `mtg-cards` does not depend on `mtg-import`). The testkit does the same from the record itself. |
| 3 | `assemble(&PrintedCard, Option<&Card>, &Caps) -> Assembled { definition, support }` reads the front face (cost without its `{X}` symbols and `x_in_cost`, type line, power, toughness, loyalty, defense, colours), gives a land the mana abilities of its basic land types (CR 305.6) and attaches the rules the support record says the engine runs. A card with no file is `Verified` when it has no rules text once reminder text is removed, and `Manual` otherwise. |
| 4 | `PlayerSetup.library` gets one `Arc<CardDefinition>` per deck line; copies share it. `AbilityDef::Mana` is the lowered plain `{T}: Add {X}`; every other ability the engine runs is an `Arc<Ability>` in `CardDefinition.rules`, named by its index there (`lower`). |
| 5 | The recipe's deck fingerprint (`recipe/fingerprint.rs`) hashes the serde form of the definitions, so a corrected record, changed rules or a change in what the engine runs changes it and a resumed game fails as a changed deck does. A field added to `CardDefinition` is left out of that form while unused, so `FINGERPRINT_VERSION` stays 1. |

A printed gap makes the card `Manual` and attaches no rules; `support` computes it, so the deck check and the game agree, and a `Caps` name lifts it once the engine plays the value:

| Printed value | In the definition | Lifted by |
|---|---|---|
| `*`, `1+*` or `X` as power, toughness, loyalty or defense | `None` | `Card.VariableStats` |
| `{X}` in the cost | `x_in_cost`; the engine asks X as the spell is cast and the spell, its permanent, its enters trigger and the delayed triggers it sets up remember it | `Card.XCost` |
| A second face | A split card, an Adventure, a modal double-faced card and a transforming double-faced card are played with their faces, each face assembled like a card of its own (`CardDefinition.layout` and `faces`); a `prepare` card plays its spell face as a copy cast by the prepared permanent; any other card with a second face (meld) is only its front face | `Card.Faces`, `Card.Prepare` |
| A mana symbol the engine does not know, an unreadable cost, a type word that is no card type | No cost, or the word dropped | Nothing |

## Vocabulary

- A node is a variant of a type in `crates/mtg-cards/src`; the rustdoc of each variant says what it means. `mtg-cli dsl-reference` prints the type tree as the model prompt of `propose` sees it, and `mtg-cli dsl-schema` its JSON schema.
- Defaults: `who` is `You`; `source` is `This`; an `Amount` is 1; a `Bounds` is 1..1 and `max: null` is any number.
- `Target(i)` is the set chosen for the i-th `TargetSpec`: one object for a plain spec, several for "any number". An effect on a set applies to each member.
- A `Selector` is a list of `Pred`s that must all hold; the zone is the battlefield unless `In(zone)` says otherwise.
- Values (`Ref`, `Selector`, `Amount`, `Condition`) are evaluated when an effect runs or a pattern is matched and produce no events.
- One reader and one writer turn every object phrase into a `Selector` and back (`selectors::phrase` and `selectors::phrase_text`); every effect, trigger, condition and static reader calls this pair rather than a template of its own, so an atom added to it works in all of them at once.
- Machinery that counts "this way" (`Destroyed`, `Exiled`, `LifeLostThisWay`) counts what the resolving spell or ability has done so far, as the objects last existed.
- A new primitive adds its name to `Caps` (below) only when the engine runs it.

## Support records

| State | Meaning | From |
|---|---|---|
| `Verified` | Rules cover the whole text, the text is unchanged, the engine runs every primitive used; or there is no text to cover | `impl_status: Full`, equal fingerprints, `Caps` cover the primitives; or no file and no rules text on any face once reminder text is removed, and no printed gap |
| `Read` | A whole draft the checker could not prove, read and found correct by a player; runs like `Verified` | A draft with no `Manual` ability, equal fingerprints, `Caps` covering every primitive, and at least one "correct" and no "wrong" verdict on its `read_hash` (Read tier) |
| `Forge` | A trusted whole translation of a Forge script (Forge source); runs like `Read`, counted and shown apart | As `Read`, with the draft `approved` by its Forge source instead of a player |
| `Partial` | Rules exist except the listed clauses | `impl_status: Partial`, or `Full` with primitives the engine lacks |
| `Manual` | No rules, or the engine runs none of them | No file and rules text, `impl_status: Manual`, or no primitive covered |
| `Stale` | The oracle text or the face list changed since the rules were written; counts as `Manual` | `Full` or `Partial` with a different fingerprint |
| `Unplayable` | The engine cannot host the card | `impl_status: Unplayable` |

| Item | Rule |
|---|---|
| Declared and computed | The file declares `impl_status` (`Full` is the database enum `full`). `support(Option<&Card>, &PrintedCard, &Caps) -> SupportRecord` computes the state. A file says `Full` only with a scenario test or compiler golden; CI enforces it. |
| Clause | An `Ability::Manual { reason, span }`. `reason` is plain language; `span` is `{start, end}` in Unicode scalar values of the face's imported oracle text, end exclusive. |
| Status and abilities | `Full` has no clause; `Partial` has at least one clause and one modelled ability; `Manual` and `Unplayable` have no abilities. Clauses do not overlap and lie inside the text. |
| `Caps` | The primitives the running engine implements, named by enum and variant (`Effect.DealDamage`, `Keyword.Flying`, `Trigger.ZoneChange`, `Cost.Tap`, `Mod.ModifyPT`, `ReplaceBy.Multiply`, `Ability.Loyalty`), plus names for shapes inside a node that only some engine paths run (`Effect.Discard.Random`, `Ability.ManaAbility.Complex`, `Amount.X.Elsewhere`) and for the printed gaps (`Card.XCost`, `Card.Faces`, `Card.VariableStats`). `mtg_engine::caps()` supplies it from `RUNS` in `caps.rs`; `lower` carries every ability the `Caps` cover into the definition. Value nodes are not gated except those the evaluator cannot read yet, which count as primitives of their own. |
| Engine gap | `used_primitives(ability)` lists what an ability needs. A face with an uncovered primitive reports the clause "the engine does not run X yet" over the whole face text. A card moves from `Manual` to `Partial` to `Verified` as primitives land, with no file edit. |
| Declared ahead of the engine | A node the engine does not run is declared with a stub (an effect file in `resolve/prim/` that refuses with `ResolveError::Unsupported("Effect.<Name>")`, a cost part named in `price.rs`, a trigger pattern that matches no event, a keyword whose rules value changes nothing). The support walker names the node, so a card that uses it stays `Partial` until the entry is added to `caps.rs`. |
| `has_rules_text(text)` | True when the text has anything outside parentheses (an unclosed parenthesis counts as text). |
| `oracle_fingerprint(text)` | Lines trimmed, whitespace collapsed, blank lines dropped, joined by `\n`; SHA-256; first 16 bytes as 32 hex characters. |
| `implementation_hash(card)` | The same digest of the compact JSON of every face's abilities. |
| `SupportRecord.fingerprint` | SHA-256 (first 16 bytes) over `oracle_id`, state, clauses, implementation hash, stored fingerprints and current fingerprints. It changes with the data, the imported text and the engine's coverage of the card. The deck's `coverage_hash` is built from `(oracle_id, fingerprint)` (`deck-check.md`). |
| `Library::support(&PrintedCard, &Caps)` | The pure per-card lookup the deck check calls. |

### Read tier

A card the checker cannot prove runs automatically once a player has read it (D14). `Verified` keeps its proof; `Read` is counted on its own wherever states are counted.

| Item | Rule |
|---|---|
| Draft | A whole rules file (no `Manual` ability) whose checker verdict is `Unchecked`: valid, nothing contradicts the text, a construct has no writer. A trusted Forge draft may also be `Mismatch` (a wording difference). Other `Invalid` and `Mismatch` proposals never become drafts; a translator draft is checked without the checker's repairs, so a slip is refused and never rewritten; an `Accepted` one is promoted into the corpus as `Verified`. Drafts live in `crates/mtg-cli/drafts/`, outside the corpus. |
| `read_hash` | SHA-256 (first 16 bytes as 32 hex characters) over the oracle id, each face's current `oracle_fingerprint` and the draft's `implementation_hash`. A changed draft or an erratum gives a new hash; `Caps` do not enter it. |
| Verdict | `correct`, or `wrong` with a note, by one account on one `read_hash`; the account's newest verdict on that hash counts ([server.md](server.md), Reads). The server's own `wrong` (a resolution that failed in a game) has no account. |
| Read state | Per `read_hash`: `wrong` when a counting verdict is `wrong`, else `approved` when one is `correct`, else `pending`. A verdict on another hash counts for nothing, so a changed draft or text is read again. |
| State | `Read` when the draft is `approved`, its fingerprints equal the imported text's and `Caps` cover every primitive it uses; otherwise the draft is ignored and the card keeps the state its file or compiled row gives. |
| Precedence | A rules file, then a `Verified` compiled row, then a draft in state `Read` or `Forge`, then a `Partial` compiled row, then nothing. A pending or wrong draft changes no game. |
| Library | `Library::with_drafts(&[DraftRules {oracle_id, read_hash, rules, read}])` keeps each card's current draft (the later row wins) and its read state, and returns the rows it left out (unparsable, invalid, or not whole). `support` and `assemble` follow the precedence; `Library::draft(oracle_id)` serves the review and `Library::draft_state(printed, caps)` the deck check. It stays pure: the server reads the rows and the verdicts. |
| Definition | `assemble` sets `CardDefinition.read` for a card in state `Read` or `Forge` (left out of the JSON while false, so no other fingerprint changes); the table shows it and fail-soft resolution reads it ([engine.md](engine.md), Fail-soft resolution). |
| Sheet | `mtg_cards::read::sheet(&Card, &PrintedCard) -> ReadSheet` gives one `ReadRow` for each printed clause of every face in text order, from the checker's pairing: the clause as printed, its `state` (`Written`, `Unwritten`, `Differs`, `ByHand`), the abilities that stand for it and `steps`, depth-marked lines of what the game asks, checks and does. Every node has words; one without a phrase of its own is named by its variant and fields. A clause without an ability says it is played by hand. No row is blank. |
| Smoke gate | `mtg_testkit::smoke::smoke_card(&CardDefinition, seeds)` plays the card with `read` cleared, so a rules error fails the gate instead of being left to the table: per seed a bot game at two and at four seats under every invariant and the watchdog, with bots that favour playing the card. A draft passes when no game fails and the card was cast, played, activated or triggered at least once; otherwise it fails (`NeverSeen`, `Invariant`, `Engine`, `Stuck`, `BadEnding`, `Panicked`). |

#### Forge source

A draft has a source (`forge` for a Forge translation) and `oracle_matches`: Forge's Oracle text equals the printed text after normalising. A draft with source `forge` and `oracle_matches` is trusted: its read state is `approved` without a human verdict (a `wrong` verdict still wins), and its card has the state `Forge`: it runs like `Read` (fail-soft resolution included) but is counted apart and shown as "from Forge", never as `Verified`. A Forge draft whose text did not match needs a read like any other. Source and match are kept in `drafts/meta/<slug>.json`, embedded with the drafts and stored by `compile` in `card_drafts.source` and `card_drafts.oracle_matches`; `DraftRules.forge` carries the trust into the library.

## Execution

State is the fold of events; the engine is a pure function from state to `Progress` (events or a decision).

| Topic | Rule |
|---|---|
| Ability objects | `GameObject` has a kind: `Card`, `Token`, `Ability`. An ability lives in `Zone::Stack` with an id from the same counter and holds its source, a `Snapshot` of it, the controller, an `Arc` to its rules (it resolves without its source, CR 113.7a), its `Choices` and, for a trigger, the cause. A spell is the card object. |
| Identity | Every object has a zone-change counter bumped by the fold (CR 400.7). A stored reference is `(ObjectId, counter)`. |
| Announcement | One procedure (`announce/`, `engine.md`, Announcement) in the order of CR 601.2b to 601.2h. Each step is a decision asked while the cast or activation is pending outside state: nothing is logged until the cast is paid, cancel is free. A specification that allows no target is skipped, one that cannot be met rewinds the cast. A trigger chooses its targets when it goes on the stack (CR 603.3d); those answers are logged. |
| Mana abilities | A mana ability of the shape `mana_shape` reads is not put on the stack: `ActivateMana`, or the tap of a payment, does the cost, adds the mana and runs the effects that follow in one batch. |
| Target recheck | On resolution each target is tested again with the spec that chose it: same object, same zone, selector matches, not protected. If the object had targets and every one is illegal, it does nothing (`ResolutionEnded { Fizzled }`), including parts that do not use the target (CR 608.2b). Otherwise it resolves and ignores the illegal targets, except one that an earlier step of the same resolution moved: it stays named for the steps after it and is read as it was (CR 608.2h). One function, `legal_targets`, serves choosing and rechecking. |
| Journal | `GameState.resolving` holds the object, a `position` (a path of indices into the effect tree), bindings, snapshots and the journal. `step(&GameState)` reads state and journal, never mutates, and returns `Done(events)` (ending with `StepDone`) or `Ask(decision)`. An answer is validated, logged as `ChoiceMade` and appended to the journal by the fold; the same step runs again and consumes it, so a step with k decisions runs k+1 times. Random numbers come from the seeded substream, whose counter moves only when events fold. |
| Steps | One effect is one step. `If`, `May`, `Pay` and `Modal` are entered by position (`resolve/tree.rs`). The condition of an `If` is read in the state the earlier steps left. |
| Events | Each primitive without an event of its own logs one; each has a fold arm and a redaction arm in the protocol's `redact`: a library peek, a search, a scry and a surveil are private to the seat that did them, a reveal is public, and `ZoneChanged`, `ChoiceMade` and `StepDone` reach a seat whole only when every card they name is visible to it. |
| Group effects | Destroy, exile, tap, untap, counters and similar effects name their objects with a `Ref` and act on all of them together, in the order the reference lists them, in one step. A name of an object that has changed zones since (CR 400.7) or repeats is passed over. |
| Values | `Scope` in `resolve/value.rs` reads `Ref`, `Selector`, `Amount` and `Condition` from the state, the bindings, the choices and the snapshots, and changes nothing. It refuses by name what it cannot read. `X` is the value announced for the spell or ability, and 0 where none was chosen (CR 107.3). |
| Let | `Let { as, amount }` works out the amount when its step runs and keeps the number under the name, so later changes leave it as it was; a name is read only after its `Let` in the same ability, not inside a reflexive or delayed trigger. |
| Nothing to choose | An attack or block with no legal option is not asked and logs `NothingToDo`. A choice of cards with no more candidates than must be taken (`Discard`, `Sacrifice`) is not asked either. Any other decision with one legal answer and no "may" is answered by the engine unless the seat has Full Control; a payment that cannot be made is the exception, asked with No the only answer so the pace shows nothing. |
| New primitive | `resolve/prim/<effect>.rs` holds one file per `Effect` variant. A task replaces its stub, adds its name to `RUNS` in `caps.rs` and to the expected list of `the_capabilities_name_exactly_the_effects_that_run` in `resolve/prim/mod.rs` (the fast check skips that test: run `cargo test -p mtg-engine --lib`). |

### Decisions the effects need

`Decision.player` names who answers, so a decision may go to any seat. Every effect decision carries `source` (the object that asks). Decisions for a seat that is not the active player have a response clock (`server.md`). The kinds are in `mtg-engine/src/choose/`; a new decision comes with its web prompt in the same piece.

### Snapshots (last known information)

`Snapshot` is built by `snapshot(state, id)` and kept in three places only: a stack ability (its source and, for a trigger, the objects of its cause, taken at detection, CR 603.10a), a resolution (every object a step moves off the battlefield or stack, carried by `StepDone`), and `Choices` (the value of X, which outlives the spell). `characteristics` has every continuous effect applied and `keywords` are those it had after layer 6. `Amount::Stat`, `Amount::Counters`, `ControllerOf`, `Matches`, `HasKeyword` and `HasCounter` read the live object when its counter matches and the snapshot otherwise.

## Triggers, replacement and layers

| Topic | Rule |
|---|---|
| Detection | After a batch is folded, `triggers::detect(pre, events, post)` returns the triggered abilities, logged as `AbilityTriggered`. A move out of the battlefield (dies, leaves, sacrificed) is judged against `pre`: the abilities that worked then and the objects as they were (CR 603.10a); every other event against `post`. An ability works from the zone its `from` names (the battlefield unless it says otherwise). |
| State triggers | `TriggerPattern::State { condition }` (CR 603.8) is looked for where a player would receive priority, after state-based actions and before triggers go on the stack. It triggers when its source is in the zone its `from` names, the condition holds and no instance of that ability of that object waits in the queue or is on the stack. |
| `Batch` | `Batch(pattern)` is the whole pattern of a triggered ability: the events of one `detect` call that the inner pattern matches trigger it once (once for each player the events are about). |
| `Nth` | `Nth { event, nth }` triggers on the nth matching event of the turn, counted by the fold. |
| Queue and order | `GameState.pending_triggers` fills at detection. Before a player would receive priority (CR 117.5, 603.3) the active player's triggers go on the stack, then each other player's in turn order (CR 101.4). A controller with two or more asks `OrderTriggers` once, unless they are all of one source and ability. Each trigger chooses its targets as it goes on and is removed if it cannot. The sweep repeats until none is pending. |
| Intervening if | `intervening_if` is checked when the event happens, against the same state as the pattern, and again as the ability begins to resolve; if false then, the ability does nothing and ends `Lapsed` (CR 603.4). |
| Delayed and reflexive | `Effect::DelayedTrigger` adds a `Delayed` to `GameState.delayed` with the names the resolution had bound; it triggers on the next event its pattern describes (`once`) or lasts until end of turn. `Effect::Reflexive` ("when you do", CR 603.12) runs when the step before it was done and puts its trigger in the queue with targets chosen then. |
| Replacement step | Every batch of events passes through `replace::process(state, proposed, journal)` in `Engine::commit` before the fold. It returns the events that happen or the question it needs answered. A replacement applies at most once to an event and to the events that replace it (CR 614.5). When two or more apply, the affected player picks (`ChooseReplacement`, CR 616.1); an effect about its own object applies first without a question (CR 614.15). The answers of one batch are its journal, so `process` is a pure function of state, batch and journal. |
| Rules of the game | `Effect::Rule` (with a duration) and `Ability::Rule` (while its source has it) change a rule through a `GameRule`. |
| Derived statics | `layers.rs` works through the layers once for a state: the effects of resolved spells and abilities and the `Static` abilities the permanents have at that point apply layer by layer to every permanent at once, so a static's `affected`, `condition` and amounts read the other objects as the earlier layers left them. A static works while its source is in the zone its `from` names and its `condition` holds. |
| Locked or live | A resolving spell or ability that modifies characteristics or control fixes its object set when it begins (CR 611.2c): `Effect::Continuous` evaluates `affected` once and works out its amounts then. A `Static` evaluates its selector at every derivation. |
| Timestamps | A static ability takes its permanent's entry time, or the time of the effect that gave it when that is later (CR 613.7a); a resolved effect takes its creation time (CR 613.7). Both are counters assigned by the fold. |
| Durations | `UntilEndOfTurn` ends in the cleanup step with the damage (CR 514.2); `UntilEndOfCombat` after the end of combat step; the others follow their named source or counter (`Duration` in `modify.rs`). |
| Dependencies | Within a layer an effect follows those it depends on; otherwise effects apply by timestamp, after the characteristic-defining abilities of the layer (CR 613.3). A depends on B when applying B changes whether A exists, which objects A applies to or what A does to them (CR 613.8a); the engine finds it by trying B on copies and asking A again. A loop goes by timestamp (CR 613.8b). |
| Characteristic-defining | A `Static` with `defining: true` (CR 604.3) has `affected` `This`, no `condition`, no `from`, and works in every zone; on the battlefield it applies first in its layer. |

### Copies

A copy effect gives an object the copiable values of another (`CreateToken` with `CopyOf`, `Replacement` `AsCopy` for "enters as a copy", `BecomeCopy`, `CopySpell`, `CopyAbility`). A copy is a new object; a spell copy has the modes, X, optional costs and targets of the original and the values of the face the original is in. Storm and Demonstrate are keyword triggers that copy; the engine side is in [engine.md](engine.md) (copies).

## Keywords

| Item | Rule |
|---|---|
| Data | `{"Keyword": {"Ward": "{2}"}}`. `Keyword` is an enum in `keyword.rs`. |
| Engine | `keywords/mod.rs` maps each variant to a `KeywordRules` value defined in `keywords/<name>.rs`; an exhaustive `match` flags a forgotten keyword. A keyword either returns abilities (`adds_abilities`: triggers, replacements, static or activated abilities) or answers a hook the engine asks (`restricts_blocking`, `silences_players`, `answers_evasion`, and so on); one with no rules yet has a value that changes nothing. |
| Payment keywords | Convoke, improvise and delve have no hook: the payment reads the keywords. |
| Keyword macros | `mtg_cards::macros` holds two tables of JSON templates with `$N` (the number) and `$ON` (the object): `KEYWORDS` expand into abilities and `ACTIONS` into effects. `Keyword::Macro { name, amount }` adds the abilities of its entry through the one engine file `keywords/macro_keyword.rs`, and the validator walks the expansion, so a card is as supported as the nodes the entry uses. A new keyword or action that is a template of existing nodes is one `Entry`, not an engine file; the renderer writes an action back by matching its effects to the entry. |
| Dungeons | A dungeon is a file under `crates/mtg-cards/dungeons/`; the check refuses a dungeon with no room, a doubled name, an arrow up or to no room, an unreached room or a last room with an arrow, and validates each room ability. |
| Siege | A Battle with the subtype Siege has two rules by that subtype (`assemble::siege_rules`, CR 310.11), which no rules file repeats. |

## Validation

Errors name the file and the path inside it; all are collected.

| Level | Checks | Runs in |
|---|---|---|
| 1 Parse | Syntax, types, unknown fields, unknown variants | `Library::from_sources`, `load_dir` |
| 2 Structure | File name is the slug of the name; oracle id is a UUID; unique ids and names; face names unique and joined by ` // ` to the name; `impl_status` fits the abilities; a fingerprint on every face of a `Full` or `Partial` card; clause spans non-empty and disjoint; `Target(i)` in range and used; `Bound` bound earlier on the path; value nodes only where their context supplies them | `validate.rs` |
| 3 Printed | Oracle id, name, face count and names; fingerprints (drift); Scryfall keywords present on some face as an ability, a grant or inside a `Manual` clause; `X` against the printed cost; ability kinds against the type line | `Library::check_generated` and the file tests |

## Compiler and Manual fallback

- The oracle-text compiler (`compile/`) writes `Card`s with `mtg_cards::load::canonical`, validated by the same functions, with the fingerprint of the text it read.
- `compile::compile(&PrintedCard) -> Compiled { card, clauses }` is pure: reminder text and the card's own name (`~`) are stripped with a map back to the original text, and the text splits into clauses. Templates live in `compile/read/`, one module per family of wording, each with goldens and near-misses in `compile/read/tests/`.
- Every reader has a writer (`compile/read/render/`). The checker accepts a proposal only when the writer reproduces the printed clause; wordings only the checker needs live in `render/oracle_only/`.
- `tests/it/compile_corpus.rs` is the ratchet: every ability the compiler reads equals one of the hand-written file (mode labels aside); `reproduced.txt` lists the cards it holds for.
- Hand-written files win over generated ones for the same oracle id. Generated rules are `card_rules` rows (oracle id, JSON, status, fingerprint, compiler version). `Library::from_sources` loads the files and `Library::with_generated` the rows. `generated::generate` compiles a card and checks the result with `Library::check_generated`; a row that fails the checks is not stored. `mtg-cli compile [--all | --card <name>]` writes the rows ([data-sources.md](data-sources.md)).
- A line the compiler cannot read becomes `Ability::Manual { reason, span }` and the card `Partial`; if nothing is read the card is `Manual`. "That much" and "that many" name the amount of an event (`Amount::Triggering`), so only a triggered ability reads them.
- A card the engine does not run whole is cast and resolved without effects for the clauses it leaves out, and its controller does them by hand with the manual commands (`engine.md`, `manual.rs`).
- `coverage::analyze(cards, caps) -> Report` judges each distinct card of a deck list with the same support function the game uses. `mtg-cli coverage [--deck <code or name part>] [--top <n>] [--all]` runs it over the stored precons (or every Commander-legal card with `--all`) and ranks the blockers; `mtg-cli audit [--count N] [--seed N] [FILE]` lists a seeded sample of what the compiler reads beside the oracle text.

## Proposals and drafts

A proposal is a rules file for a card from outside the corpus: the Forge translator, `propose` or a person. A deterministic checker accepts it only when it is valid, reads back as the printed text and has a support state. The printed text is the only ground truth.

| Check | How | Result |
|---|---|---|
| Repair | `proposal::repair` fixes the mechanical slips of the JSON before it is parsed | `Report::repairs`, one entry for each fix |
| Valid | `Library::check_generated`: levels 1 to 3 (Validation) | The problems, each with its path |
| Reads back | The clauses of the printed text (`compile::compile`) are paired with the abilities by kind; a wording of `written_variants` equal to the clause accepts it | The pairing, with the clauses that differ or lack an ability |
| Support | `support` with `mtg_engine::caps()` | `Verified`, `Partial` or `Manual`, with the blockers |

The verdict is `Invalid` (a problem of the first check), `Mismatch` (a clause differs or an ability has no clause), `Unchecked` (valid and nothing contradicts the text, but a construct has no writer) or `Accepted`.

- `mtg_cards::proposal::check(json, &PrintedCard, &Library, &names, &Caps) -> Report`; `mtg-cli check-rules [--card NAME] [--bundle] [--verbose] [--write DIR] [--gaps FILE [--marks]] FILE...` runs it over files.
- A proposal may leave out the oracle id, the names, the status and the fingerprints: they come from the card, and the status is derived from what is modelled.
- Repairs fix a slip only when the file and the printed card leave one reading; a guess is never repaired (which face of a multi-face card, which token size), the clause goes to the table instead.
- `mtg-cli promote [--dry-run] [--partial] [--out DIR] FILE...` makes corpus cards of proposals the checker accepts: the rules file in `cards/` and the card's Scryfall record from the cache as its fixture. `promote --draft [--source forge] FILE...` also drafts an `Unchecked` proposal (and a trusted Forge `Mismatch`) behind the smoke gate and the DSL validator.
- `check-rules --gaps FILE` writes the ledger of everything that holds the checked proposals back as TSV (kind, reason, blocked, freed, weight, examples), ranked by the popularity of the cards each gap alone holds back; `--marks` adds the fix marks players made. The ledger ranks the items of [cards-backlog.md](cards-backlog.md).
- `mtg-cli propose` asks a model through the Anthropic API for the rules of cards and keeps the proposals the checker accepts; it needs `ANTHROPIC_API_KEY` and costs money (`--dry-run` sends nothing, and the key never appears in an error or log).

### Drafts

A whole card the checker cannot prove goes to the players instead of waiting for a writer (Read tier).

- `cargo test -p mtg-cli drafts::` checks every draft against its record: valid, verdict `Unchecked` (or a trusted Forge `Mismatch`), whole, fingerprints current, and the smoke gate.
- `mtg-cli` embeds the directory at build time. `compile` makes each draft its card's current draft in `card_drafts` and retires the stored ones it no longer holds.
- `mtg-cli coverage --deck CODE` lists the cards that are neither `Verified` nor `Read`; `mtg-cli draft-suspects` finds drafts likely to be wrong by setting the engine's English against the Oracle text, read-only.
- A "reads wrong" verdict adds a fix mark with its note (`mtg-cli fix-marks`, `reads`); the fix is a new draft (a new `read_hash`), which is read again.

### Forge translator

`mtg-cli translate-forge DIR [--cards NAME...] [--stored] [--out DIR]` (`crates/mtg-cli/src/translate/`) writes proposals with no model: it reads the scripts of a read-only Forge checkout at run time (nothing is copied into the repository) and maps each construct to the DSL. `promote --draft --source forge` turns the proposals into drafts, which run automatically under the Forge logo. `mtg-cli forge-check --forge DIR` sets the stored precon cards against the scripts and lists the verbs and wordings the rules lack.

- A construct the translator cannot express makes the script a gap and is never approximated; a partial translation records its gaps in its meta.
- Filters (`translate/filter.rs`): a valid-string splits into alternatives, a base and properties (`alt ("," alt)*`, `base ("." prop ("+" prop)*)?`); the readers of `valid.rs` come first, then the atom table, whose rows name their legal scopes and a probe card; a string reports every unknown word. `translate-cards.tsv` ends with `offered` (with `--stored`, for `whole` cards): the exact check accepts the draft as `Verified`, as `promote --draft` sees it before the smoke gate.
- A key a mapper lists as understood must be read: an unread listed key makes the line a gap `key:<api>.<Key>-unread`.
- A `Triggered*` word names what the engine holds for the trigger's mode (`translate/triggered.rs`): `Triggering` is the subjects of the event and `Dealer` the object that did it. `Against` is what the subject went against: the player or planeswalker an attacker attacks (also a group of attackers at one player), the attacker a creature blocks (its blocker is the `Dealer`). `Arrived { of, zone }` is the card `of` became one zone change on, for a trigger on a permanent leaving the battlefield; nobody once it moved again or elsewhere.
- `Amount::CastLastTurn` reads the seat summaries the engine keeps as each turn ends (`turn_summary.rs`): the most spells one of the named players cast in the turn that just ended; the werewolf upkeep words read it.
- `Permanent` as a card filter outside the battlefield is a card of a permanent type, never an instant or sorcery. `NoResolvingCheck` with a condition is a gap, because the engine checks the condition.
- `proposal::merge` joins a trusted proposal that leaves clauses by hand with the compiler: per clause the compiler's ability wins, else the translator's; a clause neither reads stays `Manual`, so no clause is read twice or lost.

### Partial translations

A Forge translation with gaps runs the lines it proves and leaves the others to the players, and no printed clause is lost. The card plays in state `Partial`: its automatic lines run as a `Forge` card, the others are `Manual` clauses.

- `mtg_cards::proposal::partial::prove(&Report, &[LedgerLine])` is pure and gives each printed clause one row, `Auto` or `ByHand`, or refuses the card by name (`InvalidRules`, `OracleDiffers`, `Stray`, `GapInsideLine`, `UncoveredLine`, and others in the promote report).
- The ledger: `translate-forge` writes `lines` in the meta, one `LedgerLine {face, tag, text, abilities, gap}` for each `K:`, `A:`, `T:`, `S:` and `R:` line of the script.
- Totality: a property over every stored card says the spans of `compile(printed).clauses` and the reminder text cover the whole oracle text, so a clause cannot fall between rows.
- `ByHand`: a `Manual` ability whose span equals the clause's span. `Auto`: the clause is `Reads` (the renderer writes it back whole), or `Unwritten` or `Differs` with a witness: `oracle_matches` and every content word of the clause in the text of the translated line.
- `promote --draft` takes a proposal with gaps only with `oracle_matches`, a proof and the smoke gate, and stores the proof and ledger in `drafts/meta/<slug>.json`.
- A by-hand line is a `ManualClause`; the engine asks nothing about it, so it cannot hold a game. When the compiler's event reader reads the event half of a trigger line, `ManualClause.when` holds its pattern (left out of the recipe fingerprint). A failed automatic line is `AbilityLeftToTable` and the server's `wrong`. The Commands panel is where the player acts; the card preview marks each line "Runs automatically" or "You do this by hand" ([web.md](web.md)).
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

At run time the trigger is detected against the state after the event, goes on the stack with its target chosen by the controller, and is rechecked on resolution (a target that left or gained hexproof fizzles it). The `Sacrifice` and `Discard` steps ask `ChooseCards` of the target player, not the controller; steps 3 and 4 ask nothing. Triggers caused by the sacrifice, discard and life loss go on the stack at the next sweep (CR 603.3b).

## Corpus

`crates/mtg-cards/cards/` holds the corpus cards and the predefined tokens. The corpus is the hand-written reference set: every ability is covered by a scenario test or compiler golden, and the compiler ratchet (`reproduced.txt`) holds for the cards it reproduces. The cards that exercise a mechanic are found by grepping the rules files for the node (`grep -l '"Mutate"' crates/mtg-cards/cards/*.json`) and by `tests/it/` in `mtg-cards` and `mtg-testkit`.
