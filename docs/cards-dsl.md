# Card DSL and effect pipeline

Rules data for cards (`mtg-cards`) and the pipeline that runs it (`mtg-engine`).

Scope: the data model, loader, validation, support records, a 40-card corpus and card assembly are implemented in `crates/mtg-cards`, with the engine's capabilities in `mtg-engine` and the card source in `mtg-server`. The pipeline core ("Execution", "Decisions the effects need", "Snapshots") is implemented in `mtg-engine` for targets, modes, cards chosen by a player and put in an order, yes or no, payments, options, drawing, discarding, sacrificing, destroying, exiling, countering, scrying, looking at, revealing, searching for, moving (onto the battlefield too) and shuffling cards, life, damage, mana and tokens; "Triggers, replacement and layers" is implemented for triggers (detection, the queue, order, delayed triggers, the intervening "if" and the limit) and for the layer rows of continuous effects that pump power and toughness or grant a keyword (durations, timestamps, derivation); its replacement rows, the other layer rows and "Tasks" are the contract of the other M3 engine tasks. Related: `engine.md`, `protocol.md`, `deck-check.md`, `arena-flow.md`.

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
| Support | `Verified`, `Partial`, `Manual`, `Stale` and `Unplayable` are computed from the file, the imported text and the engine's capabilities. |

## Decisions

| # | Decision | Reason | Rejected |
|---|---|---|---|
| D1 | Format: JSON, one file per oracle card. Serde's externally tagged enums, PascalCase variants, snake_case fields; unknown fields are errors; defaults are omitted. Short forms: a mana cost is a string (`"{X}{R}"`), a fixed `Amount` a number, a mana-only `Cost` a string | The database stores JSON, the M6 compiler emits data, the web reads it, errors carry a path (`serde_path_to_error`) | RON (no jsonb, no TypeScript reader), YAML and TOML (typing traps, awkward trees), Rust builders like `vanilla.rs` (not data), a text script (needs its own parser) |
| D2 | Identity: the key is the Scryfall `oracle_id`; the file is `cards/<slug>.json` at any depth; a predefined token is `cards/tokens/<id>.json` | `cards.oracle_id` is the database key; names change and collide with face names | A slug as key (a rename breaks joins), per-printing ids (rules belong to the oracle card), a sharded layout now (sharding is a rename of files) |
| D3 | Multi-face: one file per oracle card, `faces` in Scryfall order; a whole-card property (Fuse) is a keyword on face 0 | Decks and imports name the card; faces share identity and status | One file per face, a `layout` field (a copy of import data) |
| D4 | Characteristics: the printed side always comes from the import record; the file holds abilities and a per-face `oracle_fingerprint`; a token carries its own characteristics | Cards without a file need the record anyway, so there is one source; errata surface as drift; M6 generates thousands of entries | Copying cost, types and P/T into each file (a second source, silent typos) |
| D5 | Shape: verbs and values are separate; sequences are lists; results are named (`as`, `Bound`) | Names are validated; a new mechanic is one verb over existing values | Implicit anaphora (`last created`), one flat enum, a class per card |
| D6 | Support is computed, never stored | `Verified` cannot go stale or ignore an engine that lacks a primitive; no engine run | A stored flag, running tests per lookup |
| D7 | Execution is a journal of pure steps | State stays the fold of the log; a restart asks the same question; each step is testable | Blocking callbacks per choice, unlogged resolution flushed at the end, replay from the start on every answer |
| D8 | Triggers: detection from events, a queue in state | Detection is a function of events and data; the log shows why an ability appeared; a resume keeps the queue | Detection inside the fold, a queue outside state |
| D9 | Replacement: a step in front of the fold | The fold stays dumb; the log holds what happened | A `replaced` flag on events, replacement inside each primitive, a fixed global order |
| D10 | Static abilities are derived | Characteristics are already derived; layer dependencies come from the data | Registering statics as `ContinuousEffect`s, a per-card layer table |
| D11 | Keywords: one enum, one engine file each | An exhaustive `match` flags a forgotten keyword; parallel tasks touch separate files | Keywords as strings, a shared registry file |
| D12 | Validation in three levels; all errors are collected | A file is judged where the information is | Checking when the engine uses a card, stopping at the first error |
| D13 | The compiler writes the same files; an unreadable clause becomes a `Manual` ability | One reader and one validator; a deck is never blocked | A second format, refusing decks with unimplemented cards |

## Files

| Path | Content |
|---|---|
| `crates/mtg-cards/cards/<slug>.json` | Rules of one oracle card |
| `crates/mtg-cards/cards/tokens/<id>.json` | A predefined token (Treasure, Clue, Food) |
| `crates/mtg-cards/tests/fixtures/corpus_scryfall.json` | Imported Scryfall objects of the corpus |

- `mtg_cards::builtin::library()` is the library of these files, embedded at build time (`build.rs`), so a server needs no directory and a deploy carries the rules it was tested with.
- Canonical text: compact JSON in serde's field order, laid out by `mtg_cards::pretty` (a value stays on one line up to 100 columns, otherwise one entry per line).
- `MTG_UPDATE_CARDS=1 cargo test -p mtg-cards --test corpus` rewrites the files and their fingerprints; the test fails on a file that is not canonical.
- `MTG_REAL_DATA_DIR=<dir> cargo test -p mtg-cards --test corpus -- --ignored` compares the corpus with an `oracle-cards-*.jsonl.gz` download.

## Printed side and rules

The printed side of every card comes from the import record (`mtg_import::CardRecord`, stored in the `cards` table). The rules file adds abilities. A deck of oracle ids becomes `CardDefinition`s in the server (`CardSource`, `DeckRef::Oracle`):

| Step | Action |
|---|---|
| 1 | `mtg-db` reads one `cards` row per distinct oracle id of the deck (`CardRepo::get_many`); every id without a row is named in the error. |
| 2 | The server adapts each stored row to `mtg_cards::PrintedCard` (`mtg_server::printed_card`; strings only, `mtg-cards` does not depend on `mtg-import`). The testkit does the same from the record itself. |
| 3 | `assemble(&PrintedCard, Option<&Card>, &Caps) -> Assembled { definition, support }` reads the front face: cost (`mana_cost` without its `{X}` symbols, and `x_in_cost`), type line, power, toughness, loyalty, defense and colours (`color_indicator` only where the record's colours differ from the cost's). It gives a land the mana abilities of its basic land types (CR 305.6) and attaches the rules the support record says the engine runs. A card with no file is `Verified` when it has no rules text once reminder text (parentheses) is removed, such as a vanilla creature or a basic land, and `Manual` otherwise. |
| 4 | `PlayerSetup.library` gets one `Arc<CardDefinition>` per deck line; copies share it. `CardDefinition`, `AbilityDef` and `ManaAbility` live in `mtg-cards`; `AbilityDef::Mana` is the lowered plain `{T}: Add {X}`; every other ability the engine runs is an `Arc<Ability>` in `CardDefinition.rules`, named by its index there (`lower` gives `Lowered::Mana` or `Lowered::Rules`). The field is left out of the serde form while it is empty. |
| 5 | The recipe's deck fingerprint (`recipe/fingerprint.rs`) hashes the serde form of the definitions, so a corrected record, changed rules or a change in what the engine runs changes it and a resumed game fails as a changed deck does. Fields added to `CardDefinition` are left out of that form while unused, so `FINGERPRINT_VERSION` stays 1. |

A printed gap makes the card `Manual` and attaches no rules. `support` computes it, so the deck check and the game agree; a `Caps` name lifts it once the engine plays the value.

| Printed value | In the definition | Lifted by |
|---|---|---|
| `*`, `1+*` or `X` as power, toughness, loyalty or defense | `None` | `Card.VariableStats` |
| `{X}` in the cost | `x_in_cost`; the engine offers no such spell | `Card.XCost` |
| A second face | Only the front face is the card | `Card.Faces` |
| A mana symbol the engine does not know, an unreadable cost, a type word that is no card type | No cost, or the word dropped | Nothing |

## The type tree

All types are in `crates/mtg-cards/src`. Notation: `Name { field: Type = default }`.

**Card and face**
```
Card      { oracle_id, name, impl_status: ImplStatus, faces: [Face], notes? }
Face      { name, oracle_fingerprint, abilities: [Ability] }
ImplStatus = Full | Partial | Manual | Unplayable          declared by the file
TokenFile { id, token: TokenSpec }
TokenSpec { name, colors, supertypes, types, subtypes, power?, toughness?, abilities }
```

**Abilities** (`Spell`, `Activated`, `Loyalty` and `Triggered` carry `targets` and `effects`)
```
Spell      { targets, effects }                                  instants and sorceries
Activated  { cost: Cost, timing = Instant|Sorcery, limit?, from = Battlefield, targets, effects }
Loyalty    { cost: Plus(n) | Minus(n) | Zero | MinusX, targets, effects }     planeswalkers only
Triggered  { when: TriggerPattern, intervening_if?: Condition, limit?, from = Battlefield, targets, effects }
Static     { affected: Ref, mods: [Modification], condition?, from = Battlefield }
Replacement{ when: ReplaceWhen, by: ReplaceBy, condition?, from = Battlefield }
CostModifier{ applies_to: Selector, change: Increase|Reduce, mana: ManaCostX, per = 1, condition? }
Keyword(Keyword)
Manual     { reason: String, span: Span }                        an unmodelled clause
```
An activated ability is a mana ability (CR 605.1a) when it has no targets and only `AddMana` effects. Loyalty timing (CR 606.3) is engine code.

**Verbs**
```
Effect (one variant per engine primitive)
  Draw{who,count} Discard{who,count,random} Mill{who,count} Scry{who,count}
  LookAtTop{who,count,as} Reveal{what} Search{who,zone,filter,count,reveal,as}
  Choose{who,from,count,as,rest_as?} Shuffle{who} Move{what,to: Destination}
  Destroy{what,regenerate} Exile{what} Sacrifice{who,filter,count} Counter{what}
  DealDamage{source?,to,amount} GainLife{who,amount} LoseLife{who,amount}
  AddMana{who,mana: Fixed([ManaType]) | AnyColor(n),times} PutCounters{on,counter,count}
  Tap{what} Untap{what} Transform{what} WinGame{who}
  CreateToken{who,token: Named(id) | Custom(TokenSpec) | CopyOf{of,except},count,tapped,attacking}
  Continuous{affected,mods,duration} Rule{rule: GameRule,duration}
  DelayedTrigger{when,once,effects}
  If{condition,then,otherwise} May{who,effects} Pay{who,cost,paid,unpaid}
  Modal{choose: Bounds, repeat, modes: [Mode{label?,targets,effects}]}
TriggerPattern
  ZoneChange{what,from?,to?} Attacks{what} BecomesTarget{what,by} SpellCast{what,by}
  Sacrificed{what,by} StepBegins{step,whose} AnyOf([TriggerPattern])
ReplaceWhen = TokenCreation{controller} | Damage{to,combat?} | ZoneChange{what,from?,to?}
ReplaceBy   = Multiply(n) | Add(n) | Prevent | Instead([Effect])
CostItem    = Mana(ManaCostX) | Tap | Untap | PayLife(Amount) | SacrificeSelf
            | Sacrifice{filter,count} | Discard{count,filter?}
Modification= ModifyPT{power,toughness} SetPT{..} SwitchPT AddType AddSubtype SetColors
            | AddKeyword RemoveKeyword SetController(Ref) CantAttack CantBlock
Duration    = UntilEndOfTurn | UntilEndOfCombat | UntilYourNextTurn | WhileSourceOnBattlefield | Forever
GameRule    = DamageCantBePrevented
Keyword     = Flying | Reach | FirstStrike | ... | Ward(Cost) | Equip(Cost) | Enchant(Selector) | Kicker(Cost) | ...
```

**Values** (evaluated when an effect runs or a pattern is matched; they produce no events)
```
Ref       = You | Opponents | EachPlayer | This | Target(i) | Triggering | Attached | Bound(name)
          | ControllerOf(Ref) | OwnerOf(Ref) | Each(Selector)
Selector  = [Pred]                       all must hold; the zone is Battlefield unless In(zone) says otherwise (Spell means the stack); [] matches everything there
Pred      = This | Other | Is(Ref) | Type | Supertype | Subtype | Color | Colorless | Token | Spell | Tapped | Untapped
          | Attacking | Blocking | ControlledBy(Ref) | OwnedBy(Ref) | In(zone) | HasKeyword | Named
          | Compare{stat,op,to} | Not(Pred) | AnyOf([Pred])
TargetSpec{ what: AnyTarget | Player | Opponent | Object(Selector) | Either([..]), bounds = 1..1, divide?: Amount }
Amount    = n | X | Count(Selector) | Stat{of,stat} | Life(Ref) | Counters{on,counter} | Sum([Amount])
          | Divided | EvenShare(Amount) | TargetCount(i)
Condition = Not | All | Any | Compare{left,op,right} | Matches{subject,selector} | Kicked | YourTurn
```

| Item | Rule |
|---|---|
| `Target(i)` | The set chosen for the i-th `TargetSpec`: one object for a plain spec, several for "any number". An effect on a set applies to each member. |
| Defaults | `who` is `You`; `source` is `This`; an `Amount` is 1; a `Bounds` is 1..1 and `max: null` is any number. |
| `Search` | The filter picks the cards of the searcher's own zone (`zone`, a library unless it says otherwise); the searcher alone is asked for between `count.min` and `count.max` of them. `count.min` does not bind in a library or a hand: a player may find fewer cards than match, or none. The cards found are bound to `as` and revealed to every seat only when `reveal` is set; a shuffle and the move to where they go are effects of their own. |
| Small enums | `ZoneKind` (Battlefield, Hand, Library, Graveyard, Stack, Exile, Command), `Step`, `Cmp`, `StatKind` (Power, Toughness, ManaValue, Loyalty), `Destination` (Hand, Graveyard, Battlefield{tapped, under?}, Library{place: Top, Bottom or Shuffled, order?: Chosen or Random}), `CounterKind`, `Timing`, `Limit`, `Produce`. |

## Support records

| State | Meaning | From |
|---|---|---|
| `Verified` | Rules cover the whole text, the text is unchanged, the engine runs every primitive used; or there is no text to cover | `impl_status: Full`, equal fingerprints, `Caps` cover the primitives; or no file and no rules text on any face once reminder text is removed, and no printed gap |
| `Partial` | Rules exist except the listed clauses | `impl_status: Partial`, or `Full` with primitives the engine lacks |
| `Manual` | No rules, or the engine runs none of them | No file and rules text, `impl_status: Manual`, or no primitive covered |
| `Stale` | The oracle text or the face list changed since the rules were written; counts as `Manual` | `Full` or `Partial` with a different fingerprint |
| `Unplayable` | The engine cannot host the card | `impl_status: Unplayable` |

| Item | Rule |
|---|---|
| Declared and computed | The file declares `impl_status` (`Full` is the database enum `full`). `support(Option<&Card>, &PrintedCard, &Caps) -> SupportRecord` computes the state. A file says `Full` only with a scenario test or compiler golden; CI enforces it. |
| Clause | An `Ability::Manual { reason, span }`. `reason` is plain language; `span` is `{start, end}` in Unicode scalar values of the face's imported oracle text, end exclusive. |
| Status and abilities | `Full` has no clause; `Partial` has at least one clause and one modelled ability; `Manual` and `Unplayable` have no abilities. Clauses do not overlap and lie inside the text. |
| `Caps` | The primitives the running engine implements, named by enum and variant: `Effect.DealDamage`, `Keyword.Flying`, `Trigger.ZoneChange`, `Cost.Tap`, `Mod.ModifyPT`, `ReplaceWhen.TokenCreation`, `ReplaceBy.Multiply`, `Ability.Loyalty`; an ability with a `limit` or a zone other than the battlefield also uses `Ability.Limit` or `Ability.From`, a discard at random also uses `Effect.Discard.Random`, a token that copies an object `Effect.CreateToken.Copy`, and a mana ability that is not the plain one and adds more than one mana or counts something `Ability.ManaAbility.Complex`. A plain `{T}: Add {X}` (cost exactly `{T}`, one fixed mana, added once) is the one primitive `Ability.ManaAbility`, since the engine runs that shape and not the general ability. The printed gaps have names too (`Card.XCost`, `Card.Faces`, `Card.VariableStats`). `mtg_engine::caps()` supplies it; each primitive task adds its entry there, and `lower` carries every ability the `Caps` cover into the definition. Value nodes (`Ref`, `Selector`, `Amount`, `Condition`) are not gated. |
| Engine gap | `used_primitives(ability)` lists what an ability needs. A face with an uncovered primitive reports the clause "the engine does not run X yet" over the whole face text. A card moves from `Manual` to `Partial` to `Verified` as primitives land, with no file edit. |
| No file | `has_rules_text(text)` is true when the text has anything outside parentheses (an unclosed parenthesis counts as text). A card with no file is `Verified` when no face has rules text and no printed gap applies (`Card.XCost`, `Card.Faces`, `Card.VariableStats`, unreadable printed values), `Manual` otherwise. |
| `oracle_fingerprint(text)` | Lines trimmed, whitespace collapsed, blank lines dropped, joined by `\n`; SHA-256; first 16 bytes as 32 hex characters. |
| `implementation_hash(card)` | The same digest of the compact JSON of every face's abilities. |
| `SupportRecord.fingerprint` | SHA-256 (first 16 bytes) over `oracle_id`, state, clauses, implementation hash, stored fingerprints and current fingerprints. It changes with the data, the imported text and the engine's coverage of the card. The deck's `coverage_hash` is built from `(oracle_id, fingerprint)` (`deck-check.md`). |
| `Library::support(&PrintedCard, &Caps)` | The pure per-card lookup the deck check calls. |

## Execution

State is the fold of events; the engine is a pure function from state to `Progress` (events or a decision).

| Topic | Rule |
|---|---|
| Ability objects | `GameObject` has a kind: `Card`, `Token`, `Ability`. An ability lives in `Zone::Stack` with an id from the same counter and holds its source, a `Snapshot` of it, the controller, an `Arc` to its rules (it resolves without its source, CR 113.7a), its `Choices` and, for a trigger, the cause. A spell is the card object. |
| Identity | Every object has a zone-change counter bumped by the fold (CR 400.7). A stored reference is `(ObjectId, counter)`. |
| Announcement | Mode, X, targets, division, optional and alternative costs are decisions asked in that order while the cast or activation is pending outside state: nothing is logged until the cast is paid, cancel is free. The engine asks the modes and then the targets of the modes chosen today, one specification at a time and before the payment; a specification that allows no target is skipped, one that cannot be met ends the cast. A mode with an unmeetable specification is not offered, and a modal ability with too few legal modes cannot be cast (a trigger is removed). The final batch carries `Choices { modes, x, targets per spec, division, optional costs }` (a field left out of the wire form while empty). A trigger chooses its targets when it goes on the stack (CR 603.3d); those answers are logged. |
| Activation | An ability whose cost is nothing, or any of `{T}`, sacrificing the source (`SacrificeSelf`) and mana, is activated from `PriorityOptions.abilities`: `PermanentTapped` (for `{T}`), `ZoneChanged` with `ZoneCause::Sacrificed`, `AbilityActivated`, then `AbilityPutOnStack`, with the source photographed before the costs; the mana is paid in the same step by the `PayMana` decision (`engine.md`, Spells and abilities), whose sources leave out the source of an ability that taps or sacrifices it. A token sacrificed this way has ceased to exist and its ability still resolves: Clue is `{2}` and sacrifice, Food `{2}`, `{T}` and sacrifice. A mana ability of one mana with `{T}` and/or the sacrifice (Treasure) is not put on the stack: `ActivateMana` pays it and adds the mana in one batch. Other costs wait for their primitives in `Caps`. |
| Target recheck | On resolution each target is tested again with the spec that chose it: same object, same zone, selector matches, not protected. If the object had targets and every one is illegal, it does nothing (`ResolutionEnded { Fizzled }`), including parts that do not use the target (CR 608.2b). Otherwise it resolves and ignores the illegal targets, except one that an earlier step of the same resolution moved: it stays named for the steps after it and is read as it was (CR 608.2h). One function, `legal_targets(state, spec, source, controller)`, serves choosing and rechecking. |
| Journal | `GameState.resolving: Option<Resolving { object, position, bindings, snapshots, journal }>`; `position` is a path of indices into the effect tree. `step(&GameState)` reads state and journal, never mutates, and returns `Done(events)` (ending with `StepDone { position, bound, snapshots }`) or `Ask(decision)`. An answer is validated, logged as `ChoiceMade { player, choice }` and appended to the journal by the fold; the same step runs again and consumes it. `StepDone` folds the next position, bindings and snapshots and clears the journal. A step with k decisions runs k+1 times. Random numbers come from the seeded substream, whose counter moves only when events fold. |
| Events | `AbilityTriggered`, `AbilityPutOnStack`, `AbilityActivated`, `ResolutionBegan`, `ChoiceMade`, `StepDone`, `ResolutionEnded { Resolved, Fizzled, Countered }`, and one per primitive without an event: `ZoneChanged { object, from, to, cause }`, `LifeGained`, `LifeLost`, `CountersChanged`, `TokenCreated`, `LibraryLooked`, `CardsFound`, `Scried`, `Revealed`, `LibraryBottomed`, `Entered { object, tapped, controller }`. Each has a fold arm and a redaction arm in the protocol's `redact`: a library peek, a search and a scry are private to the seat that did them (a scry also puts the cards back, and `LibraryBottomed` moves cards of a library to its bottom; other seats see counts), a reveal is public, `Entered` is public (it follows the `ZoneChanged` that put a card onto the battlefield tapped or under another player's control), and `ZoneChanged`, `ChoiceMade` and `StepDone` reach a seat whole only when every card they name is visible to it. The folds of `LibraryLooked`, `CardsFound`, `Scried` and `Revealed` also record the cards in the resolution (`Resolving.shown`: the seat that looked, found or scried, or every seat for a reveal), which the views read until the resolution ends or the card changes zones (`engine.md`, Library effects). |
| Spell resolution | A spell with a `Spell` ability or any ability runs `ResolutionBegan`, its steps and `ResolutionEnded`. For a card `SpellResolved` follows `Resolved` and moves it (a permanent to the battlefield, an instant or sorcery to the graveyard); a fizzled or countered spell goes to the graveyard with `ResolutionEnded`, and an ability ceases to exist with it. A permanent spell has no steps and is only `SpellResolved`. A resolution whose controller has left the game ends `Countered`. |
| Cast during resolution | The cast sub-procedure runs inside a step; its decisions are journal decisions; the spell goes on the stack above the resolving object. |
| Nothing to choose | An attack or block with no legal option is not asked and logs `NothingToDo`. A choice of cards with no more candidates than must be taken (`Discard`, `Sacrifice`) is not asked either: the engine takes them all, or nothing when there are none, and logs only the move. Any other decision with one legal answer and no "may" is answered by the engine unless the seat has Full Control (`arena-flow.md`). |

### Interpreter

| Topic | Rule |
|---|---|
| Primitives | `resolve/prim/<effect>.rs` holds one file per `Effect` variant. Draw, Discard, Scry, LookAtTop, Reveal, Search, Choose, Shuffle, Move, Destroy, Exile, Sacrifice, Counter, DealDamage, GainLife, LoseLife, AddMana, CreateToken, Continuous (with `Mod.ModifyPT` and `Mod.AddKeyword`), DelayedTrigger, May, Pay and Modal run and are in `Caps` (a `Move` onto the battlefield is the primitive `Effect.Move.Battlefield`, also in `Caps`; a `CreateToken` of a copy is the separate primitive `Effect.CreateToken.Copy`, a `Discard` with `random` is `Effect.Discard.Random` and a `Move` to the top or bottom of a library in random order `Effect.Move.Random`, none of the three in `Caps`); every other file is a stub that returns `ResolveError::Unsupported("Effect.<Name>")` and is not in `Caps`, so no game offers a card that uses it. A task replaces its stub and adds its name to `RUNS` in `caps.rs`. |
| Destroy, exile, counter | Each names its objects with a `Ref` (a target, `Each(selector)` for "all", a bound name) and acts on all of them together, in the order the reference lists them, in one step. A name of an object that has changed zones since (CR 400.7) or repeats is passed over. `Destroy` moves a permanent that is still on the battlefield and not indestructible (`keywords::is_indestructible`) to its owner's graveyard. `Exile` moves an object from whichever zone it is in to exile, whatever its keywords; abilities, the resolving spell and objects already in exile stay. `Counter` ends a spell or ability on the stack, other than the resolving one, as `Countered`: the spell goes to its owner's graveyard, an ability ceases to exist, nothing of either resolves. The moves carry `ZoneCause::Destroyed` and `Exiled`; a counter is `ResolutionEnded { Countered }`, which triggers read as a move from the stack with `ZoneCause::Countered`. Each step carries a snapshot of every object it moved. `Destroy.regenerate` is read once an effect can make a regeneration shield; none can yet. |
| Steps | One effect is one step. `If`, `May`, `Pay` and `Modal` are entered by position: `[2, 1, 0]` is the first effect of the second list of the third effect (`resolve/tree.rs`). The lists of a `Modal` are the modes chosen, in printed order, a mode chosen twice twice. |
| Values | `Scope` in `resolve/value.rs` reads `Ref`, `Selector`, `Amount` and `Condition` from the state, the bindings, the choices and the snapshots, and changes nothing. It refuses by name what it cannot read yet: loyalty, counters, X, divided amounts and kicker (`ResolveError::Unsupported`). |
| Replacement | The events of a step pass through `replace::process(state, proposed)` before they are logged; it returns them unchanged until the replacement task. |
| Damage and drawing | `DealDamage` is `DamageDealt` for each recipient that is still a player in the game, or a creature, planeswalker or battle on the battlefield (CR 120.1); other recipients are passed over, and damage of 0 is no event. `Draw` uses the turn code's `draw`, so an empty library is logged as before. |

### Decisions the effects need

`Decision.player` names who answers, so a decision may go to any seat. Every effect decision carries `source` (the object that asks). Decisions for a seat that is not the active player have a response clock (`server.md`). `ChooseTargets` and `OrderTriggers` are asked and have their prompts in the web; `ChooseCards` is asked by `Discard`, `Sacrifice`, `Choose` over objects, `Scry` and `Search` (its question carries `text`, "Discard a card." or "Search your library for a basic land card."; a search offers the matching cards, between 0 and the most that may be found), `OrderCards` by `Scry` and `Move` (the controller puts cards in one library in an order, top first), `YesNo` by `May` ("Draw a card?"; the players `who` names are asked in turn order until one says yes) and by `Pay` ("Pay {2}?"; the same order until one pays, those who cannot pay are not asked), `PayMana` by `Pay` after a yes (the payment cannot be backed out of), `ChooseOption` by `Choose` over players ("Choose an opponent."; a player with one option is not asked) and `ChooseMode` at announcement by `Modal`; `YesNo`, `ChooseOption`, `ChooseMode`, `ChooseCards` and `OrderCards` have their prompts and response clocks; the other kinds in the table, `PayMana` aside, exist as options, answer and wire types in `choose/<kind>.rs` and the four of them without a prompt (`ChooseNumber`, `DivideAmount`, `ChooseCosts`, `ChooseReplacement`) have a placeholder in the web, which their task replaces. The questions of `YesNo`, `ChooseOption`, `ChooseCards` and `OrderCards` carry `text` in the words of a card, and `ChooseMode` says whether the cast can be backed out of (`can_cancel`).

| Decision | Who answers | When | Corpus cards that need it |
|---|---|---|---|
| `ChooseTargets` | controller | announcement; trigger put on stack | Lightning Bolt, Doom Blade, Swords, Giant Growth, Counterspell, Burst Lightning, Eternal Witness, Izzet Charm, Jace, Pacifism (Enchant), Bonesplitter (equip), Archon of Cruelty, Fireball, Fire // Ice |
| `ChooseMode` | controller | announcement | Izzet Charm |
| `ChooseNumber` | controller | announcement (X, multikicker); resolution ("any number") | Fireball |
| `DivideAmount` | controller | announcement | Fire // Ice |
| `ChooseCards` | any seat | resolution; cost payment | Rampant Growth, Cultivate, Flooded Strand, Brainstorm, Jace 0, Faithless Looting, Archon (sacrifice and discard, by the target player), Sneak Attack |
| `OrderCards` | any seat | resolution | Opt, Brainstorm, Jace 0, cascade (bottom) |
| `YesNo` | any seat | resolution ("may", pay or not) | Eternal Witness, Jace +2, Rhystic Study (both players), Delver |
| `ChooseOption` | any seat | resolution | Hero of Bladehold in multiplayer (which defender), Treasure (colour) |
| `ChooseCosts` | controller | announcement (optional costs; alternative costs are separate `castable` entries) | Burst Lightning, Mulldrifter, Faithless Looting |
| `OrderTriggers` | controller | two or more triggers of one controller that are not all of one source and ability | a card with two triggers on one event |
| `ChooseReplacement` | affected player | two or more applicable replacement effects | Anointed Procession with a second token effect |
| `PayMana` | any seat | announcement of a spell or an ability with a mana cost; resolution after a yes to "Pay {N}?" | all spells; Rhystic Study, Izzet Charm |

### Snapshots (last known information)

`Snapshot { id, counter, controller, owner, zone, characteristics, counters, attachments }` is built by `snapshot(state, id)` and kept in three places only. `counter` names the object it describes, `characteristics` has every continuous effect applied, and `counters` and `attachments` are the permanent's own (empty off the battlefield).

| Place | Content |
|---|---|
| Stack ability | Its source and, for a trigger, the objects of its cause, taken at detection (CR 603.10a: look-back uses the state before the event). |
| Resolution | Every object a step moves off the battlefield or stack, carried by `StepDone`, so "its power" and "its controller" in Swords to Plowshares read the creature as it was when exiled (CR 608.2h). `Target(i)` still names a target that an earlier step moved, so those later steps find it. |
| `Choices` | The value of X, which outlives the spell. |

`Amount::Stat`, `ControllerOf` and `Matches` read the live object when its counter matches and the snapshot otherwise.

## Triggers, replacement and layers

| Topic | Rule |
|---|---|
| Detection | After a batch is folded, `triggers::detect(pre, events, post)` returns `Triggered { source, ability, controller, cause, snapshot, delayed }`, logged as `AbilityTriggered`. A move out of the battlefield (dies, leaves, sacrificed) is judged against `pre`: the abilities that worked then and the objects as they were (CR 603.10a); every other event against `post`. An ability works from the battlefield (a `from` other than that is not in `Caps`). |
| Pattern events | `ZoneChange`: `LandPlayed`, `SpellResolved`, `CreatureDied`, `CardDiscarded`, `CardDrawn`, `ZoneChanged` and the end of a spell that did not resolve. `Attacks`: each attacker of `AttackersDeclared`. `SpellCast`: `SpellCast`, with the caster and the spell as subjects. `Sacrificed`: `ZoneChanged` with cause sacrifice, with the permanent and who sacrificed it. `StepBegins`: `StepBegan`, with the active player. `BecomesTarget`: each object among the targets of `SpellCast` and `AbilityPutOnStack`, with the object and what targets it. |
| Queue and order | `GameState.pending_triggers` fills at detection. Before a player would receive priority (CR 117.5, 603.3) the active player's triggers go on the stack, then each other player's in turn order (CR 101.4). A controller with two or more asks `OrderTriggers` once, unless they are all of one source and ability; otherwise they go on in the order they triggered. Each trigger chooses its targets as it goes on (CR 603.3d, logged) and is removed if it cannot. State-based actions run before each sweep; the sweep repeats until none is pending, and a trigger that triggers during a sweep waits for the next round. |
| Intervening if | `intervening_if` is checked when the event happens, against the same state as the pattern, and again as the ability begins to resolve; if false then, the ability does nothing and ends `Lapsed` (CR 603.4). |
| `AnyOf` | Joins patterns ("enters or attacks"); one event triggers the ability once. |
| `limit` | `OncePerTurn` is kept per object in state, reset at the turn, and holds for triggered and activated abilities. |
| Delayed triggers | `Effect::DelayedTrigger` logs `DelayedTriggerCreated` and adds a `Delayed` to `GameState.delayed`: its pattern, effects, controller, the names the resolution had bound (the ability starts with them) and a duration. It triggers on the next event its pattern describes and is then gone (`once`), else it lasts until the end of the turn it was set up in. A duration that ran out, or a controller who left the game, drops it. |
| Replacement step | Effect steps and rules code produce proposed events; `replace::process(state, proposed)` returns the final events or needs a choice. Only "would" events pass through it: damage, life, draw, discard, mill, zone change, counters, tokens, tap. A replacement is data (a `Replacement` ability or a registered shield with a duration) and applies at most once per event (CR 614.5). |
| Replacement choice | The affected player picks among several (`ChooseReplacement`, CR 616.1), self-replacement first (CR 614.15). The choice is a journal entry, so `process` is a pure function of state and journal. |
| Prevention | `ReplaceBy::Prevent`. `GameRule::DamageCantBePrevented` wins over any shield. |
| Derived statics | `characteristics(state, id)` gathers the `Static` abilities of everything that functions in its zone plus the registered `ContinuousEffect`s, layer by layer. Today it gathers the registered effects only (`engine.md`, Continuous effects). |
| Layers | `SetController` 2; `AddType`, `AddSubtype` 4; `SetColors` 5; `AddKeyword`, `RemoveKeyword` 6; `SetPT` 7b; `ModifyPT` 7c; `SwitchPT` 7d; `CantAttack`, `CantBlock` are restrictions without a layer. `Modification::layer()` returns it. `ModifyPT` and `AddKeyword` run from `Effect::Continuous`; the other modifications refuse by name (`Mod.SetPT`, ...) and are not in `Caps`. |
| Locked or live | A resolving spell or ability that modifies characteristics or control fixes its object set when it begins (CR 611.2c): `Effect::Continuous` evaluates `affected` once, to the permanents it names then, each as the incarnation named (a permanent that leaves and returns is a new object, CR 400.7), and works out its amounts then. A `Static` evaluates its selector at every derivation. |
| Timestamps | A static ability takes its permanent's entry time (an Aura's or Equipment's effects, the time it became attached); a resolved effect takes its creation time (CR 613.7), one for each modification in the order of the card's text. Both are counters assigned by the fold. |
| Durations | `UntilEndOfTurn` ends in the cleanup step with the damage (CR 514.2); `UntilEndOfCombat` after the end of combat step; `UntilYourNextTurn` when the controller's next turn begins; `WhileSourceOnBattlefield` follows the source named on the effect; `Forever` ends only when every object it names is gone. All five run. |
| Dependencies | A modification names the characteristics it writes; a selector and condition name those they read (`Pred::Type` types, `Pred::Color` colours, `HasKeyword` abilities, `Compare` power or toughness). A depends on B when B writes what A reads or applies to. Within a layer, effects apply by timestamp except that an effect follows those it depends on; a cycle falls back to timestamps (CR 613.8). |

## Keywords

| Item | Rule |
|---|---|
| Data | `{"Keyword": {"Ward": "{2}"}}`. `Keyword` is an enum in `keyword.rs` with `name()` in Scryfall's spelling (compared with the record's `keywords`). It declares every keyword the roadmap lists. |
| Engine | `keywords/mod.rs` maps each variant to a `KeywordRules` value defined in `keywords/<name>.rs`; a keyword without its task is a stub that changes nothing. A keyword task replaces its stub and adds its `Caps` entry in `caps.rs`. `keywords_of(state, id)` is the one reader of the keywords an object has: the printed ones, with layer 6 joining there: the keywords continuous effects grant (`AddKeyword`) follow in timestamp order, never twice; `RemoveKeyword` joins with its task. The twelve keywords of combat and hexproof are built (`engine.md`, Combat and keywords). `player_keywords(state, player)` is the same reader for a player (hexproof, CR 702.11d). |
| Hooks | `restricts_blocking`, `answers_evasion`, `min_blockers`, `forbids_attacking`, `ignores_summoning_sickness`, `attacks_untapped`, `is_indestructible`, `modifies_damage`, `restricts_targeting`, `restricts_player_targeting`, `grants_timing`, `adds_abilities`. Combat, damage, state-based actions, targeting (`protects` and `protects_player`, part of `legal_targets`) and casting at instant speed (`grants_instant_timing`) call theirs; a stub answers "no". A keyword expressible in the DSL (equip, cycling, prowess, battle cry) returns abilities from `adds_abilities` and is tested like any card; one that bends rules machinery (flying, first strike, trample) is a hook in combat or damage code. |

## Validation

Errors name the file and the path inside it; all are collected.

| Level | Checks | Runs in |
|---|---|---|
| 1 Parse | Syntax, types, unknown fields, unknown variants (a keyword typo reads `cards/serra_angel.json: faces[0].abilities[1].Keyword: unknown variant ...`) | `Library::from_sources`, `load_dir` |
| 2 Structure | File name is the slug of the name; oracle id is a UUID; unique ids and names; token id is its file name; face names unique and joined by ` // ` to the name; `impl_status` fits the abilities; a fingerprint on every face of a `Full` or `Partial` card; clause spans non-empty and disjoint; `Target(i)` in range and used; `Bound` bound earlier on the path (not visible after its branch, no duplicates); `Triggering` only in triggers; `Divided` and `EvenShare` only with a divided or multi-target spec; `TargetCount` in range; `Kicked` only with a kicker keyword; `Modal` the only effect of its ability; `CreateToken::Named` exists; reference kinds fit (`Draw.who` is a player); bounds minimum not above maximum | `Library::from_sources`, `load_dir` |
| 3 Printed | Oracle id, name, face count and names; fingerprints (drift); Scryfall keywords of ours present on some face, as an ability or in the text a `Manual` clause covers (a granted keyword, `Plainscycling` for `Cycling`); `X` against the printed cost (an ability whose own cost has X is exempt); `Spell` only on instants and sorceries and required on a `Full` one; `Loyalty` only on planeswalkers; `Enchant` only on Auras; spans inside the text; card names in `Pred::Named` and `PartnerWith` exist | `Library::check_printed(&[PrintedCard])` |

## Compiler and Manual fallback

- The oracle-text compiler writes `Card`s with `mtg_cards::load::canonical`, validated by the same functions, with the fingerprint of the text it read.
- `compile::compile(&PrintedCard) -> Compiled { card, clauses }` is pure: reminder text and the card's own name (`~`) are stripped with a map back to the original text, the text splits into clauses (one per line; a keyword list one per keyword; modes, level blocks and ability words stay whole), each clause has a `ClauseKind` (keyword, spell, triggered, activated, loyalty, modal, static), and `clauses` lists every clause, read or not, for the coverage report.
- Keyword lines are read into `Ability::Keyword` when the DSL declares the keyword and its parameter reads (`Enchant` on an Aura only); the card is `Full` when every clause is read, `Partial` when some are, `Manual` with no abilities when none is, and `card` is `None` when no face has rules text or the data cannot hold the card (over four faces, two faces with one name).
- Hand-written files win over generated ones for the same oracle id. Generated rules are `card_rules` rows (oracle id, JSON, status, fingerprint); hand-written ones are repository files; `Library::from_sources` takes both.
- A line the compiler cannot read becomes `Ability::Manual { reason, span }` and the card `Partial`; if nothing is read the card is `Manual`. Each compiled card is rendered back to English and compared with its oracle text in the golden tests.
- A `Manual` card is cast and resolved without automatic effects; the table sees the oracle text with the unsupported spans marked and uses the generic logged actions. Every seat sees the card's support (`deck-check.md`).
- Prior art for the templates: phase.rs's oracle parser (`crates/engine/src/parser` in its repository; MIT or Apache-2.0) and XMage (MIT). Forge and Magarena are GPL-3.0: study only, copy nothing.

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

This is `crates/mtg-cards/cards/archon_of_cruelty.json`; a test keeps the two equal. Oracle text: `Flying` / `Whenever this creature enters or attacks, target opponent sacrifices a creature or planeswalker of their choice, discards a card, and loses 3 life. You draw a card and gain 3 life.`

| Feature | In the file |
|---|---|
| One ability, two events | `when: AnyOf [ZoneChange to Battlefield, Attacks]` |
| Target opponent | `targets: [{ what: Opponent }]` |
| Actors differ | `who: Target(0)` for the first three effects; the default `You` for the last two |
| Choice by the target player | `Sacrifice` filters the target's own creatures and planeswalkers; the target player chooses |

| Phase | Engine behaviour |
|---|---|
| Detection | `SpellResolved` or `ZoneChanged { to: Battlefield }` matches the `ZoneChange` arm against the state after the event; `AttackersDeclared` matches `Attacks`. `AbilityTriggered` names the Archon, ability 1, its controller and a snapshot of it. One event triggers once. |
| On the stack | At the next sweep, before a player receives priority: `ChooseTargets` goes to the controller (`triggered`, with no cancel); `AbilityPutOnStack` creates the ability object with the chosen player. |
| Recheck | If the opponent is no longer a legal target (left the game, gained hexproof) the ability fizzles and nothing happens, including the controller's draw and life gain (CR 608.2b). |
| Step 0 `Sacrifice` | `ChooseCards` to the **target** player (not the controller) over their creatures and planeswalkers ("Sacrifice a creature or planeswalker."); with none, or one, nothing is asked. Then `ChoiceMade`, `ZoneChanged { cause: Sacrificed }`, `StepDone`. |
| Step 1 `Discard` | `ChooseCards` from the target's hand ("Discard a card."), addressed and private to them; with an empty hand, or one card, nothing is asked. Then `ChoiceMade` (shown to the other seats without the card), `CardDiscarded`, `StepDone`. |
| Step 2 `LoseLife` | `LifeLost` through the replacement step; `StepDone`. |
| Steps 3, 4 | `Draw` and `GainLife` for the controller; no decisions. `ResolutionEnded { Resolved }`. |
| Afterwards | Triggers caused by the sacrifice, discard and life loss go on the stack at the next sweep (CR 603.3b). |

| The engine needs | Item |
|---|---|
| `Triggered` kind, `AnyOf`, `ZoneChange` and `Attacks` with their events; detection with snapshots | (i) |
| Ability stack object, `AbilityPutOnStack`, journal events, `ChooseTargets` with `Opponent` legality | (c) |
| `ChooseCards` addressed to a non-controller with a response clock | (d) |
| `Sacrifice`, `Discard`, `LoseLife`, `Draw`, `GainLife` with `LifeGained` and `LifeLost` | (f) |
| `Flying` (blocking restriction) | (e) |

## Corpus

`crates/mtg-cards/cards/` holds 40 cards and 3 tokens.

| Feature | Cards |
|---|---|
| Targets, damage | Lightning Bolt, Burst Lightning (kicker), Fireball (X, even split, cost per extra target), Fire // Ice (split, divided damage, Fuse) |
| Counter, destroy, exile, mass removal | Counterspell, Doom Blade, Swords to Plowshares, Wrath of God (Verified) |
| Pump with duration | Giant Growth (Verified) |
| Library | Rampant Growth, Cultivate, Opt, Brainstorm, Flooded Strand (fetchland), Jace, the Mind Sculptor (planeswalker) |
| Modes | Izzet Charm |
| Triggers | Mulldrifter (evoke), Eternal Witness, Dockside Extortionist (Treasure), Tireless Tracker (Clue), Hero of Bladehold (attack, tokens, battle cry), Archon of Cruelty, Delver of Secrets // Insectile Aberration (transform), Felidar Sovereign (intervening if), Rhystic Study, Bonecrusher Giant // Stomp (adventure), Sneak Attack (delayed trigger) |
| Replacement | Anointed Procession |
| Static, Aura, Equipment | Pacifism, Bonesplitter |
| Mana abilities | Llanowar Elves, Sol Ring, Gaea's Cradle |
| Keywords | Serra Angel, Bloodbraid Elf (cascade), Faithless Looting (flashback) |
| No abilities | Forest, Grizzly Bears |
| `Partial`, `Manual` | Questing Beast (three clauses), Panharmonicon |
| Tokens | Treasure (a mana ability, made by Dockside Extortionist), Clue (made by Tireless Tracker) and Food (`{2}` and a sacrifice, paid as any ability with mana) |

## Cross-check

Vocabulary of other engines against this DSL. Sources: P = phase.rs (MIT or Apache-2.0), X = XMage (MIT), F = Forge scripting vocabulary (GPL-3.0; names only, no code or scripts). Status: `DSL` is in the vocabulary; a letter is the M3 item that adds it; `M6` is added by demand from the coverage dashboard.

| Primitive | In | Status |
|---|---|---|
| `AnyOf` compound triggers (enters or attacks) | P, X | DSL |
| `Pay` ("unless pays", "may pay, if you do") | P, X, F | DSL |
| `DelayedTrigger`, `Rule`, `CostModifier`, `If`, `May`, `Modal` with per-mode targets | P, X, F | DSL |
| `limit` and `from` zone on abilities | P, X, F | DSL |
| A `Modification` for every layer, `CopyOf` token, `WinGame` | P, X | DSL |
| Keyword actions: Fight, Explore, Proliferate, Populate, Amass, Connive, Learn, Goad, Detain, Monstrosity, Adapt, Bolster, Support, Venture, Manifest, Cloak, Incubate, Suspect | P, X, F | (h); Proliferate (l), Populate (o), Manifest and Cloak (n) |
| `RevealUntil`, `ExileTop`, `Dig` (look at N and take some is `LookAtTop` + `Choose` + `Move`) | P, X, F | (h) |
| `Exile` with a duration (Oblivion Ring), `Duration` until host leaves, until a player's next turn, until an event | P | (h) |
| `CastFromZone`, free cast, `AsThough` (flash, play from graveyard or library), cast permissions | P, X | (g) |
| `CopySpell`, `BecomeCopy` | P, X, F | (o) |
| Costs: Exile, TapUntapped (crew), RemoveCounters (also X), PayEnergy, ReturnToHand, Mill, Reveal, Exert, Unattach, OneOf | P, X, F | (g) |
| `ChooseOption` effect (colour, type, name, player) with a remembered choice | P, X, F | (j) |
| `Condition::Performed` ("if you do") | P, F | (c) |
| Trigger families: damage, life, draw, discard, mill, cycle, exile, reveal, shuffle, search, tap, untap, mana, counters, attach, blocks, unblocked, copy, counter, activate, transform, token created, monarch, initiative | P, X, F | (i4) |
| Batch ("one or more"), first time each turn, nth card drawn; state triggers (CR 603.8) | P, X, F | (i4); state triggers (i3) |
| `Pred` and `Amount` over turn history (entered, attacked, spells cast this turn, life gained), event amount ("that much"), size of a bound set ("that many") | P, X | (i4) |
| `Amount`: kicker count, spells cast this turn (Storm) | P, X | (g), (o) |
| Statics: must attack or block, can't be countered, can't gain life or draw, trigger doubling, can't search, reveal top, characteristic-defining flag, remove abilities or types, exchange control | P, X, F | (k) |
| Shields (`PreventDamage`, damage and draw replacement), redirection; replaced events: draw, life, destroy, discard, mill, counters, tap, untap, enters tapped or with counters | P, X, F | (j) |
| Emblems, extra turns, skip turn, additional phase, set or exchange life totals, `LoseGame`, dice, coin, clash, vote, separate into piles, devotion | P, X, F | M6 |
| Saga chapters, class levels, level up, opening-hand actions, special actions | P, X | M6 |
| Implicit memory (`RememberCard`, `TrackedSet`, `LastCreated`, `ParentTarget`, `Imprint`) | P, F | not used: `as` and `Bound` name results and are validated |
| Conditions split by context (ability, trigger, static, replacement) | P | not used: one `Condition`, validated per position |

## Tasks

The order of the M3 work; `ROADMAP.md` lists the pieces. A structural task changes how abilities, characteristics, events, costs or views are shaped, so it runs alone and before the tasks that fill one stub. The rows after (c) are in build order.

| Item | Delivers | Needs |
|---|---|---|
| (a) DSL | Data model, loader, validation, support records, corpus | |
| (b) Card assembly | `PrintedCard` adapter, `assemble`, `CardDefinition` in `mtg-cards`, corpus cards in the testkit; the deck fingerprint keeps version 1 | (a) |
| (c) Pipeline core | Ability objects, zone-change counter, `Snapshot`, journal and its events, interpreter with one stub file per `Effect` variant, value evaluator, replacement seam, `ChooseTargets` with recheck and fizzle, every new `DecisionKind`, `Answer`, `Event` and redaction arm as types, keyword stubs, engine `Caps` | (b) |
| (e) Keywords | One task each with a table test | (c) |
| (f) Keyword actions | One `Effect` each; discard, sacrifice, the library effects, destroy, exile, counter, pay, search and tokens are built | (c) |
| (i) Triggers | Detection, queue, APNAP, delayed, intervening if | (c) |
| (r) Compiler | Core, renderer and goldens, `card_rules` rows, templates for the vocabulary the engine runs, the coverage report, the missing vocabulary added in one pass | (a), (c) |
| (q) Small effects | `If`, `Tap`, `Untap`, `Mill`, `WinGame`, `Flash` | (c) |
| (k) Abilities and layers | One reader of an object's abilities with stable addresses (printed, keyword-provided, granted, lost, by zone), static abilities, every `Modification`, `Rule`, the CR 613 derivation (timestamps, dependencies, characteristic-defining abilities), ward, prowess and battle cry (e2), state triggers and abilities from other zones (i3) | (c) |
| (l) Counters and planeswalkers | Counters on permanents and players with their effects, layer 7c, state-based actions, loyalty abilities | (k) |
| (m) Attachments | `Attach`, Auras (`Enchant`), Equipment (`Equip`, `Reconfigure`), their state-based actions | (k), (l) |
| (j) Replacement | The step, `ChooseReplacement`, one `ReplaceWhen` per replaceable event, enters tapped or with counters, prevention shields, regeneration | (k), (l) |
| (g) Costs and casting | The announcement procedure, X, optional, alternative and additional costs, cost modifiers, cast permissions, costs of activated abilities, complex and restricted mana, `ChooseNumber`, `ChooseCosts`, `DivideAmount` | (k), (j) |
| (i4) Trigger families | Patterns for every event, batches, first time each turn, event amounts | (g), (l), (m) |
| (n) Faces | Multi-face cards, `Effect::Transform`, face-down objects | (g), (k) |
| (o) Copies | Copy effects, spell copies, enters as a copy, populate, storm | (k), (j), (n) |
| (p) Manual cards | A card the engine does not run is cast and resolves without effects; generic logged actions through the primitives | (l), (g) |
| (h) By demand | Keywords and keyword actions the DSL does not declare, sagas, classes, designations, extra turns: one task each, in the order of the coverage dashboard | the rows above |

(d) Decision kinds are built with the first task that asks for them: `ChooseNumber`, `ChooseCosts` and `DivideAmount` with (g), `ChooseReplacement` with (j). Their web prompts follow in M5.

The tasks run one at a time in the order of the table; (h) fills stub files and may run in any order. Rules data leads the engine: a compiled card is `Partial` until the engine runs its primitives. From (q) on, a task ends with the templates and goldens for its wordings, and tasks that do not need each other run in the order of the cards the coverage report says they free.

- Scope of (f), the keyword actions of CR 701: draw, discard, mill, shuffle, scry, surveil, fateseal, search, reveal, sacrifice, destroy, exile, counter, create tokens, populate, investigate, fight, explore, proliferate, amass, connive, learn, transform, meld, attach, regenerate, goad, monstrosity, adapt, bolster, support, venture. Arena-only digital actions (conjure, seek, specialize) are out of scope; a card that needs one is `Unplayable` (`deck-check.md`). An action that belongs to a cluster is built there: attach (m), regenerate (j), proliferate (l), transform and meld (n), populate (o); the others are (h).
