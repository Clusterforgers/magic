# Card DSL and effect pipeline (M3)

Design record (2026-10-18) for the card rules data in `mtg-cards` and the pipeline that runs it in `mtg-engine`. Built with this record: the data model in `crates/mtg-cards` (types, serde, loader, validation, support records, a corpus of 40 cards and 3 tokens in `crates/mtg-cards/cards/`). Not built: everything under "Execution", "Triggers, replacement and layers" and "Tasks"; that is the contract the M3 engine tasks follow. Rules of the house: `CLAUDE.md`. Engine notes: `engine.md`. Wire types: `protocol.md`. Deck check: `deck-check.md`.

The shape in five lines:
1. A card's printed side (name, cost, types, P/T, oracle text) always comes from the imported Scryfall record; the DSL file holds only the **rules**: abilities per face, keyed by oracle id, plus the fingerprint of the oracle text they were written for.
2. Rules are JSON data built from a small closed vocabulary: verbs (`Effect`, `TriggerPattern`, `CostItem`, `Modification`, `Keyword`) and values (`Ref`, `Selector`, `Amount`, `Condition`). A new primitive is one enum variant.
3. The engine runs an effect list as a **journal**: each step is a pure function of state that either yields events or asks one `Decision`; answers are logged as events, so state stays the fold of the log and a rebuilt engine asks the same question again.
4. Abilities are objects on the stack; targets are chosen on announcement and rechecked on resolution (CR 608.2b); triggers are detected from events before and after the fold; replacement effects are a step in front of the fold; static abilities are derived when characteristics are derived (layers).
5. Keywords are enum variants with parameters in the data and engine code in one stub file each; whether the engine runs a card is a computed **support record**, not a claim.

## Decisions

Each decision has a reason and, in one line, what was rejected.

### D1 Format: JSON, one file per oracle card
Plain JSON read with `serde_json`; enums are serde's default externally tagged form (`{"DealDamage": {...}}`, unit variants as strings), variants in PascalCase and fields in snake_case, the same dialect as events and decisions on the wire. Unknown fields are errors. Defaults are omitted, so `{"Draw": {}}` means "you draw one card". A mana cost is a string (`"{X}{R}"`), an `Amount` is a bare number when fixed, a mana-only `Cost` is a bare string. The canonical text of a file is `serde_json::to_string_pretty` of the parsed value plus a newline; `MTG_UPDATE_CARDS=1 cargo test -p mtg-cards --test corpus` rewrites the files (as `MTG_UPDATE_BINDINGS` does for the bindings), and the test fails on a file that is not canonical, so diffs stay small and the compiler's output is byte-identical to hand-written files.
*Why:* the database stores JSON, the M6 compiler emits data, the web can read it, and `serde_json` and `serde_path_to_error` give errors with a path. *Rejected:* RON (no jsonb, no TypeScript reader, gaps in serde support), YAML and TOML (typing traps, awkward trees), Rust builders like `vanilla.rs` (not data, cannot be generated into rows), a Forge-style text script (needs its own parser; the compiler already targets data).

### D2 Identity and layout
The key is the Scryfall `oracle_id` (UUID). The file is `cards/<slug>.json` where the slug is the lowercase ASCII card name (`fire_ice`); the loader reads any depth, so sharding by first letter later is a rename. Predefined tokens are files in `cards/tokens/` keyed by a slug `id` (`treasure`). The loader checks the slug against the name, and that oracle ids and names are unique.
*Why:* `cards.oracle_id` is the primary key of the database and rulings, decks and precons already resolve to it; names change and collide with faces. *Rejected:* slug as key (a rename breaks joins), per-printing ids (rules belong to the oracle card), a sharded layout now (a rename later costs nothing).

### D3 Multi-face cards are one file
One file per oracle card with `faces` in Scryfall's order (split, adventure, transform, modal, flip, meld fronts). A face holds its name, the text fingerprint and its abilities; the layout and every printed characteristic of each face come from the record, so the file never says what kind of card it is. A property of the whole card (Fuse) is a keyword on face 0.
*Why:* decks and imports name the card, not the face; faces share identity and status. *Rejected:* one file per face (loses the shared oracle id and status), a `layout` field in the file (a second copy of import data that can drift).

### D4 Where characteristics come from at game time
**Decision:** the printed side of every card comes from the imported record (`mtg_import::CardRecord`, stored in the `cards` table: `faces` jsonb, `oracle_text`, `keywords`, `type_line`, `mana_cost`), keyed by oracle id. The DSL file carries abilities and, per face, `oracle_fingerprint`, the SHA-256 (first 16 bytes, hex) of the face's oracle text as imported when the abilities were written, with whitespace normalised. If the text now differs (errata), the card is `Stale` (see "Support records") and plays as `Manual` until someone reviews it. Tokens are the exception: a token has no printed record, the effect text defines it, so a `TokenSpec` carries its own characteristics.
*Why:* (1) most cards are `Manual` and have no file, so a converter from the record to the engine's `CardDefinition` must exist anyway; two sources for the same fields would disagree. (2) Errata and Scryfall fixes flow in with each import and the fingerprint makes the rules visible as drift instead of silently wrong. (3) M6 will generate tens of thousands of rules entries; copying cost, types and P/T into each is waste. (4) The file stays short: abilities only. *Rejected:* duplicating the characteristics in each file (self-contained, but a second source of truth with its own drift and a typo in a P/T silently changes a game).
*Cost accepted:* DSL tests need records, so `crates/mtg-cards/tests/fixtures/corpus_scryfall.json` holds the trimmed Scryfall objects of the corpus (from the 2026-10-18 Oracle Cards bulk file); a real-data test can compare it with a fresh download.

**From a deck of oracle ids to `CardDefinition` (no converter exists today).** In the server, when a game is created:
1. `mtg-db` reads one `cards` row per distinct oracle id of the deck (`CardRecord` with `faces`).
2. An adapter turns each record into `mtg_cards::PrintedCard` (plain strings: oracle id, name, Scryfall keywords, and per face name, mana cost, type line, oracle text, power, toughness, loyalty, defense). The adapter lives in the server because `mtg-cards` does not depend on `mtg-import`; the engine can then depend on `mtg-cards` without the parsers.
3. `mtg_cards::assemble(&PrintedCard, Option<&Card>, &Caps) -> Assembled` (a pure function, task "Card assembly") builds the characteristics (cost via `ManaCostX`, type line into supertypes, types and subtypes, power and toughness, loyalty and defense, colours) and attaches the abilities of the rules file when the support record says the card runs, otherwise it attaches none and the card is `Manual`. `Assembled { definition, support }` carries the effective support so the table can show it.
4. `PlayerSetup.library` gets one `Arc<CardDefinition>` per deck line. `CardDefinition` and `AbilityDef` move from `mtg-core` to `mtg-cards` in that task (the engine already has to depend on `mtg-cards` to interpret rules, and `mtg-core` cannot name them); `AbilityDef::Mana` becomes the lowered form of the plain `{T}: Add {X}` ability so the existing mana code keeps working until the interpreter replaces it.
5. The game recipe stores the oracle ids and, per card, the support fingerprint and a hash of the assembled definition, so a resumed game fails loudly when the data changed (this closes the roadmap item "Resume checks the deck contents").
`*` power and toughness (characteristic-defining) have no value until the layers task; until then `assemble` marks such a card `Manual`.

## The type tree

All of it is in `crates/mtg-cards/src`. Value types are written `Name { field: Type = default }`.

**Card and face**
```
Card      { oracle_id, name, impl_status: ImplStatus, faces: [Face], notes? }
Face      { name, oracle_fingerprint, abilities: [Ability] }
ImplStatus = Full | Partial | Manual | Unplayable          what the file claims; see "Support records"
TokenFile { id, token: TokenSpec }
TokenSpec { name, colors, supertypes, types, subtypes, power?, toughness?, abilities }
```

**Abilities** (one per rules kind; the first four carry `targets` and `effects`)
```
Spell      { targets, effects }                                  instants and sorceries
Activated  { cost: Cost, timing = Instant|Sorcery, limit? , from = Battlefield, targets, effects }
Loyalty    { cost: Plus(n) | Minus(n) | Zero | MinusX, targets, effects }     planeswalkers only
Triggered  { when: TriggerPattern, intervening_if?: Condition, limit?, from = Battlefield, targets, effects }
Static     { affected: Ref, mods: [Modification], condition?, from = Battlefield }
Replacement{ when: ReplaceWhen, by: ReplaceBy, condition?, from = Battlefield }
CostModifier{ applies_to: Selector, change: Increase|Reduce, mana: ManaCostX, per = 1, condition? }
Keyword(Keyword)
Manual     { reason: String, span: Span }                        an unmodelled clause (Partial cards)
```
Whether an activated ability is a mana ability (CR 605.1a) is derived: no targets, only `AddMana` effects, not a loyalty ability. A planeswalker's loyalty abilities carry the timing of CR 606.3 in the engine, not in the data.

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
ReplaceWhen = TokenCreation{controller} | Damage{to,source} | ZoneChange{what,from?,to?}
ReplaceBy   = Multiply(n) | Add(n) | Prevent | Instead([Effect])
CostItem    = Mana(ManaCostX) | Tap | Untap | PayLife(Amount) | SacrificeSelf
            | Sacrifice{filter,count} | Discard{count,filter?}
Modification= ModifyPT{power,toughness} SetPT{..} SwitchPT AddType AddSubtype SetColors
            | AddKeyword RemoveKeyword SetController(Ref) CantAttack CantBlock
Duration    = UntilEndOfTurn | UntilEndOfCombat | UntilYourNextTurn | WhileSourceOnBattlefield | Forever
GameRule    = DamageCantBePrevented
Keyword     = Flying | Reach | FirstStrike | ... | Ward(Cost) | Equip(Cost) | Enchant(Selector) | Kicker(Cost) | ...
```

**Values** (evaluated when an effect runs or a pattern is matched; never produce events)
```
Ref       = You | Opponents | EachPlayer | This | Target(i) | Triggering | Attached | Bound(name)
          | ControllerOf(Ref) | OwnerOf(Ref) | Each(Selector)
Selector  = [Pred]                       all predicates must hold; zone is Battlefield unless an In(zone) says otherwise
Pred      = This | Other | Type | Supertype | Subtype | Color | Colorless | Token | Spell | Tapped | Untapped
          | Attacking | Blocking | ControlledBy(Ref) | OwnedBy(Ref) | In(zone) | HasKeyword | Named
          | Compare{stat,op,to} | Not(Pred) | AnyOf([Pred])
TargetSpec{ what: AnyTarget | Player | Opponent | Object(Selector) | Either([..]), bounds = 1..1, divide?: Amount }
Amount    = n | X | Count(Selector) | Stat{of,stat} | Life(Ref) | Counters{on,counter} | Sum([Amount])
          | Divided | EvenShare(Amount) | TargetCount(i)
Condition = Not | All | Any | Compare{left,op,right} | Matches{subject,selector} | Kicked | YourTurn
```
`Target(i)` is the set chosen for the i-th `TargetSpec` (one object for a plain spec, several for "any number"). An effect on a set applies to each member.

Small enums: `ZoneKind` (Battlefield, Hand, Library, Graveyard, Stack, Exile, Command), `Step`, `Cmp` (Eq, Ne, Lt, Le, Gt, Ge), `StatKind` (Power, Toughness, ManaValue, Loyalty), `Destination` (Hand | Graveyard | Battlefield{tapped, under?} | Library{place: Top | Bottom | Shuffled, order?: Chosen | Random}), `CounterKind` (+1/+1, -1/-1, loyalty, poison, charge, other), `Bounds { min = 1, max = 1 }` where `max: null` means any number, `Timing`, `Limit` (once per turn), `Produce`.

**Why this shape.** Verbs and values are separate so a new mechanic is usually one verb over existing values. Lists replace nesting for sequencing (`effects` is a plain list; there is no `Seq`). Everything that returns something later is named (`as`, `Bound`). `who` defaults to `You`; `source` to `This`.

**Rejected:** implicit anaphora (`last created`, `the target of the previous clause`, as phase.rs and Forge do), which cannot be validated and breaks when a step is inserted; one enum for everything (no validation of what each position accepts); a class per card (XMage), which is code, not data.

## Support records

The deck check (`deck-check.md`) needs, for every oracle id, a state, reasons with text spans, and fingerprints, all computed without running the engine. The data supports it like this.

| State (computed) | Meaning | From |
|---|---|---|
| `Verified` | rules exist, cover the whole text, the text is unchanged, the engine runs every primitive used | `impl_status: Full`, fingerprints equal, `Caps` cover the primitives |
| `Partial` | rules exist except the listed clauses | `impl_status: Partial`, or `Full` whose primitives the engine lacks |
| `Manual` | no rules, or the engine runs none of them | no file, `impl_status: Manual`, or no primitive covered |
| `Stale` | rules exist but the oracle text (or the face list) changed since | `Full` or `Partial` with a different fingerprint; counts as `Manual` until reviewed |
| `Unplayable` | the engine cannot host the card (digital-only actions, ante, joke sets) | `impl_status: Unplayable` |

- **Declared and computed are different.** The file declares `impl_status: Full | Partial | Manual | Unplayable` (what the data covers; `Full` is the database enum `full`). `support(&Card, &PrintedCard, &Caps) -> SupportRecord` computes the five states. "Its tests pass" is not computed: it is a repository rule that a file may say `Full` only with a scenario test or compiler golden, and CI enforces it. The corpus of this task declares what the data expresses; the engine gate below keeps the computed state truthful while the engine is behind.
- **Clauses.** A clause is an `Ability::Manual { reason, span }` in the face. `reason` is plain language ("the engine has no trigger-doubling effect"); `span` is `{start, end}` in Unicode scalar values into the face's imported oracle text, end exclusive (JavaScript `slice` agrees for the BMP text cards use). `Partial` needs at least one clause and one modelled ability; `Full` has none; `Manual` and `Unplayable` have no abilities. Clauses must not overlap, and with the printed text they must lie inside it.
- **Engine gate.** `Caps` is the set of primitives the running engine implements, named `Effect.DealDamage`, `Keyword.Flying`, `Trigger.ZoneChange`, `Cost.Tap`, `Mod.ModifyPT`, `Replace.TokenCreation`, `Ability.Loyalty`. Validation records the primitives a face uses (`used_primitives`); `support` reports a face that uses an uncovered primitive as a clause "the engine does not run X yet" covering the whole face text (per-ability spans are a later refinement), so a card flips from `Manual` to `Partial` to `Verified` as engine tasks land, with no edit to its file. `Caps` is an argument, supplied by the engine crate (its stub table flips one entry per primitive), so `mtg-cards` stays pure. Value-level nodes (`Ref`, `Selector`, `Amount`, `Condition`) are not gated: the interpreter foundation implements them whole.
- **Fingerprints.** `oracle_fingerprint(text)`: lines trimmed and whitespace collapsed, joined by `\n`, SHA-256, first 16 bytes as 32 hex characters. `implementation_hash(card)`: the same hash of the canonical JSON of every face's abilities. `support_fingerprint`: SHA-256 (32 hex) over `oracle_id`, the computed state, the clauses, the implementation hash, the stored fingerprints and the current fingerprints of the imported text. It therefore changes when the data changes, when an import changes the text (`Stale`), and when an engine release changes what is covered; it does not change when an unrelated primitive is added. The deck's `coverage_hash` is built from `(oracle_id, support_fingerprint)` as in `deck-check.md`.
- **Not built here:** the deck report, the endpoint, the marker, highlighting (M4), the `card_support` rows and the coverage run (M6). The database keeps `cards.impl_status` for the declared value; the computed five states are written by the coverage run.

## Execution

This is the contract for the engine tasks. The principle stays: state is the fold of events; the engine is a pure function from state to `Progress` (events or a decision).

### Stack objects, announcement, resolution
- **Abilities are objects.** `GameObject` gets a kind: `Card`, `Token`, `Ability`. An ability object lives in `Zone::Stack` with an `ObjectId` from the same counter and holds `source` and a `Snapshot` of it (below), `controller`, which ability of which card (an `Arc` to the data, so it resolves even if the source is gone, CR 113.7a), its `Choices` and, for a trigger, the cause. Spells keep being the card object on the stack.
- **Identity across zones.** Every object has a zone-change counter, bumped by the fold on each move (CR 400.7). A stored reference is `(ObjectId, counter)`; a reference whose counter differs names a different object.
- **Announcement (CR 601.2, 602.2).** Choosing a mode, X, targets, a division, optional costs and an alternative cost are decisions asked in that order while the cast or activation is pending outside state, exactly like the payment today: nothing is logged until the cast is paid, cancel is free, a rebuilt engine asks again. The final batch (`SpellCast` or `AbilityActivated`, the cost events) carries the `Choices { modes, x, targets per spec, division, optional costs }`, which the fold stores on the object. Triggers choose targets when they are put on the stack (CR 603.3d); that cannot be cancelled, so those answers are logged (see journal).
- **Targets are rechecked on resolution (CR 608.2b).** When the top object resolves, each chosen target is tested again with the spec that chose it: same object (counter), still in the zone, still matching the selector, not protected (hexproof, protection, shroud). If the object had targets and **every** one is illegal, it does nothing at all (`ResolutionEnded { Fizzled }`): none of its effects, none of its choices, including the parts that do not use the target. If at least one is legal it resolves, and an instruction about an illegal target does nothing for that target. Legality is one function, `legal_targets(state, spec, source, controller)`, used for choosing and for the recheck.
- **The journal.** `GameState.resolving: Option<Resolving { object, position, bindings, snapshots, journal }>`. `position` is a path of indices into the effect tree (`[2, 0, 1]` is the second effect of the first branch of the third effect). A step is `fn step(&GameState) -> Step`: it reads state and the journal, never mutates, and returns `Done(events)` (this step's events, ending with `StepDone { position, bound, snapshots }`) or `Ask(decision)`. An answer to an effect decision is validated like every answer and logged as `ChoiceMade { player, choice }`; the fold appends it to the journal; the engine runs the same step again, which now consumes the answer. `StepDone` folds the next position, the new bindings and snapshots in and clears the journal. A step with k decisions runs k+1 times, which is the whole cost; random numbers come from the seeded substream whose counter only moves when events fold, so every re-run draws the same ones. A rebuilt engine reads `resolving` from the folded log and asks the same decision again.
- **New events** (each needs a visibility decision in the protocol's redaction `match`): `AbilityTriggered`, `AbilityPutOnStack`, `AbilityActivated`, `ResolutionBegan`, `ChoiceMade`, `StepDone`, `ResolutionEnded { Resolved | Fizzled | Countered }`, and one event per primitive that has no event yet (`ZoneChanged { object, from, to, cause }` for moves, destroys, sacrifices and exiles, `LifeGained`, `LifeLost`, `CountersChanged`, `TokenCreated`, `LibraryLooked`, `CardsFound`, `Scried`, `Revealed`, `DamageDealt` already exists). Library peeks and searches are private to one seat, which needs a third visibility class beside public and hidden.
- **`SpellResolved` stays the last event of a spell's resolution:** it moves the card to the graveyard or battlefield after the effect steps. A permanent spell has no steps.
- **Casting during resolution** (cascade, cast-from-exile effects) runs the cast sub-procedure inside a step: its decisions are journal decisions, and the spell goes on the stack above the resolving object.
- **Nothing to choose.** A decision with no legal option is not asked and logs a `NothingToDo` of the new kinds (sacrifice, discard, search). A decision with exactly one legal answer and no "may" is answered by the engine unless the seat has Full Control (`arena-flow.md`); the answer is logged like any other.

### Decisions the effects need
All are `DecisionKind` variants with their own legal options, like today's. The roadmap's eight plus three found while designing (`DivideAmount`, `ChooseOption`, `ChooseCosts`). A decision may be addressed to any seat, not only the controller: `Decision.player` already says who answers, and every effect decision also carries `source` (the object that asks) so the prompt can say whose ability it is.

| Decision | Who answers | When | Corpus cards that need it |
|---|---|---|---|
| `ChooseTargets` | controller | announcement; trigger put on stack | Lightning Bolt, Doom Blade, Swords, Giant Growth, Counterspell, Burst Lightning, Eternal Witness, Izzet Charm, Jace, Pacifism (via Enchant), Bonesplitter (equip), Archon of Cruelty, Fireball, Fire // Ice |
| `ChooseMode` | controller | announcement | Izzet Charm |
| `ChooseNumber` | controller | announcement (X, multikicker); resolution ("any number") | Fireball |
| `DivideAmount` | controller | announcement | Fire // Ice |
| `ChooseCards` | any seat | resolution; cost payment | Rampant Growth, Cultivate, Flooded Strand (search), Brainstorm and Jace 0 (put back), Faithless Looting and Archon (discard), Archon (sacrifice, by the target player), Sneak Attack |
| `OrderCards` | any seat | resolution | Opt (scry), Brainstorm, Jace 0, cascade (bottom) |
| `YesNo` | any seat | resolution ("may", pay or not) | Eternal Witness, Jace +2, Rhystic Study (both players), Delver |
| `ChooseOption` | any seat | resolution | Hero of Bladehold in multiplayer (which defender the tokens attack), Treasure (colour) |
| `ChooseCosts` | controller | announcement (optional costs; alternative costs are separate `castable` entries) | Burst Lightning (kicker), Mulldrifter (evoke), Faithless Looting (flashback) |
| `OrderTriggers` | controller | before triggers go on the stack, two or more of one controller | Hero of Bladehold (battle cry and the attack trigger), Mulldrifter (evoke sacrifice and draw) |
| `ChooseReplacement` | affected player | an event with two or more applicable replacement effects | Anointed Procession with a second token effect |
| `PayMana` (exists) | any seat | announcement; "pay" effects | all spells; Rhystic Study, Izzet Charm (opponent pays) |

New decisions for a non-active seat get a response clock like blocking (`server.md`), so one player cannot hold the table.

### Snapshots (last known information)
`Snapshot { id, controller, owner, zone, characteristics, counters, attachments }` is the immutable record of an object as it last existed, built by `snapshot(state, id)`. It is stored in three places and nowhere else. (1) On a stack ability for its source and, for a trigger, for the objects of its cause, taken when the trigger is detected (CR 603.10a: leaves-the-battlefield abilities look back, so the pre-event state is used). (2) In the resolution for every object a step is about to move off the battlefield or stack, carried by `StepDone`, so "its power" and "its controller" in Swords to Plowshares read the creature as it was when exiled (CR 608.2h). (3) On an X: the value lives in `Choices` and outlives the spell. `Amount::Stat`, `ControllerOf` and `Matches` read the live object when its counter still matches and the snapshot otherwise.

## Triggers, replacement and layers

### Triggers
- **Detection.** After a batch of events is folded, `triggers::detect(pre, events, post)` returns `AbilityTriggered { source, ability, cause }` events. Patterns that look back in time (`ZoneChange` from the battlefield, dies, leaves, sacrifices) match against `pre`, all others against `post`. The cause stores a snapshot. Delayed triggers are registered in state (`GameState.delayed`) with their pattern, the bindings of the resolution that created them and a duration, and match the same way. Detection is a function of events and DSL data only.
- **Patterns and the events that back them.** `ZoneChange` is `LandPlayed`, a permanent spell's `SpellResolved`, `CreatureDied`, `CardDiscarded`, `CardDrawn` today and the planned `ZoneChanged` for the rest; `Attacks` is `AttackersDeclared`; `BecomesTarget` needs the new targeting event; `SpellCast` is `SpellCast`; `Sacrificed` is `ZoneChanged` with cause sacrifice; `StepBegins` is `StepBegan`. A pattern the engine has no event for yet is simply not in `Caps`.
- **Queue and APNAP.** `GameState.pending_triggers` fills at detection. Before a player would receive priority (CR 117.5, 603.3) the engine puts them on the stack: the active player's first, then each other player in turn order (CR 101.4). A controller with two or more asks `OrderTriggers`; each trigger then asks `ChooseTargets`; each goes on the stack with `AbilityPutOnStack`. State-based actions run before every such sweep and the sweep repeats until nothing is pending.
- **Intervening if.** `intervening_if` is checked when the event happens (it decides whether the ability triggers) and again when it resolves; if it is false on resolution the ability does nothing (CR 603.4).
- **Several events in one ability.** `AnyOf` joins patterns ("enters or attacks"); one matching event triggers it once.
- **Limits.** `limit: OncePerTurn` is counted per object in state and reset at the turn.

### Replacement and prevention (CR 614, 615, 616)
- **A step in front of the fold.** Effect steps and rules code produce *proposed* events. `replace::process(state, proposed) -> Processed` returns the final events or needs a choice. Only events that mean "would" go through it (damage, life, draw, discard, mill, zone change, counters, tokens, tap); the rest skip it at the cost of a `match`. A replacement effect is data (`Replacement { when, by }` on a permanent, or a registered shield with a duration) and cannot be applied twice to one event (CR 614.5).
- **Choice.** When two or more apply, the affected player (or controller of the affected object) picks the next one (`ChooseReplacement`, CR 616.1), after the self-replacement effects (CR 614.15). The choice is a journal entry like any other, so `process` is again a pure function of state and the journal, and applying one effect re-evaluates which others still apply.
- **Prevention** is `ReplaceBy::Prevent`; a "can't be prevented" rule is `GameRule::DamageCantBePrevented` and wins over any shield.
- **Why not events with a "replaced" flag:** the fold must stay dumb; the log must contain what happened, not what almost did.

### Continuous effects and layers (CR 613)
- **Static abilities are derived, never stored as effects.** `characteristics(state, id)` gathers the `Static` abilities of everything that can function in its zone, plus the registered `ContinuousEffect`s of resolved spells and abilities, and applies them layer by layer. `Modification` maps to a layer by its kind: `SetController` 2, `AddType`/`AddSubtype` 4, `SetColors` 5, `AddKeyword`/`RemoveKeyword` 6, `SetPT` 7b, `ModifyPT` 7c, `SwitchPT` 7d; `CantAttack` and `CantBlock` are restrictions with no layer. (Characteristic-defining abilities, 7a, and copy effects, layer 1, come with the layers task.)
- **Locked-in or live.** An effect created by a resolving spell or ability that modifies characteristics or control fixes the set of objects when it begins (CR 611.2c): `Effect::Continuous` evaluates `affected` once and registers the ids. A `Static` ability evaluates its `affected` selector every time characteristics are derived.
- **Timestamps (CR 613.7).** A static ability takes the timestamp of its permanent's entry (an Aura's or Equipment's effects take the time it became attached); a resolved effect takes the time it was created. Both are counters in state, assigned by the fold.
- **Dependencies (CR 613.8).** Because the rules are data, the footprint of an effect can be computed rather than hand-listed: `Modification::writes()` names the characteristics it changes, and a `Static`'s selector and condition name the ones they read (`Pred::Type` reads types, `Pred::Color` colours, `HasKeyword` abilities, `Compare` power or toughness). Effect A depends on B when B writes something A reads or applies to. Within a layer, effects apply in timestamp order, except that an effect waits for those it depends on; a cycle falls back to timestamps.

## Keywords

- A card names a keyword with its parameters (`{"Keyword": {"Ward": "{2}"}}`); `Keyword` is an enum in `mtg-cards/src/keyword.rs` with `Keyword::name()` (the Scryfall spelling, used to compare with the record's `keywords`). The foundation task of this design declares every keyword the roadmap lists, so a keyword task does not edit the enum.
- The engine holds one table, `keywords/mod.rs`, mapping each variant to a `KeywordRules` value defined in its own file `keywords/<name>.rs`, all created as stubs by the foundation task. A keyword task replaces its stub only (and adds its `Caps` entry in the same file), so parallel keyword tasks never touch a shared file. Hooks are narrow: `restricts_blocking`, `modifies_damage`, `grants_timing`, `adds_abilities`. A keyword that is expressible in the DSL (equip, cycling, prowess, battle cry) returns its meaning as abilities from `adds_abilities`, so it is tested like any card; one that bends rules machinery (flying, first strike, trample) is a hook in combat or damage code.
- *Why:* "implemented once, tested once" (`PLAN.md`); an exhaustive `match` over the enum makes a forgotten keyword a compile error. *Rejected:* keywords as strings validated against a table (silent no-ops, slower checks in combat loops), a shared registry file every keyword task edits (conflicts; `agents.md`).

## Validation and errors

Three levels, all with errors that name the file and the path inside it, collected rather than stopping at the first.
1. **Parse.** `serde_json` through `serde_path_to_error`: syntax, wrong types, unknown fields, unknown variants (a typo in a keyword is "unknown variant `Flyign`, expected one of ..." at `faces[0].abilities[1]`). Message form: `cards/serra_angel.json: faces[0].abilities[1]: unknown variant ...`.
2. **Structure** (the file alone). Slug equals the name, oracle id is a UUID, unique ids and names, face names unique, `impl_status` against the abilities (`Full` has no `Manual`, `Partial` has both, ...), clause spans (non-empty, no overlap), `Target(i)` in range and used, `Bound(name)` bound earlier on the path (names bound in a branch are not visible after it; no duplicates), `Triggering` only in triggers, `X` only where a cost has X, `Divided` and `EvenShare` only on a divided or multi-target spec, `TargetCount` in range, `Kicked` only with a kicker keyword on the card, `Modal` as the only effect of its ability, `CreateToken::Named` against the token files, kinds of references (`Draw.who` cannot be an object).
3. **Against the printed record** (`check_printed`): face count and names, fingerprints, Scryfall keywords that are keyword abilities of ours but absent from the face, `X` against the printed cost, `Spell` only on instants and sorceries (and an instant or sorcery face has one unless `Manual`), `Loyalty` only on planeswalkers, `Enchant` only on Auras, clause spans inside the text, card names in `Pred::Named` and `PartnerWith` that exist among the printed names.
Level 1 and 2 run in `Library::from_sources` and `load_dir`; level 3 runs where records are at hand (the corpus test, the CLI, the server at start).

## M6: the compiler and the Manual fallback

- The oracle-text compiler emits exactly these types (`serde_json::to_string_pretty` of a `Card`), validated by the same functions, with the oracle fingerprint of the text it read. Hand-written files win over generated ones for the same oracle id. Generated rules live in the database (`card_rules` rows keyed by oracle id with the JSON, status and fingerprint), hand-written ones in the repository; `Library::from_sources` accepts both.
- A line the compiler cannot parse becomes `Ability::Manual { reason, span }` and the card `Partial`; if nothing parses the card is `Manual` with no abilities. Each compiled card is rendered back to English from the data and compared with its oracle text in the golden tests (`ROADMAP.md`).
- A `Manual` card is cast and resolved with no automatic effect; the table sees the oracle text with the unsupported spans marked and uses the generic logged actions. The card's `Support` is shown to every seat (`deck-check.md`).

## Worked example: Archon of Cruelty

Oracle text (2026-10-18): `Flying` / `Whenever this creature enters or attacks, target opponent sacrifices a creature or planeswalker of their choice, discards a card, and loses 3 life. You draw a card and gain 3 life.` {6}{B}{B}, 6/6 Archon.

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
This is `crates/mtg-cards/cards/archon_of_cruelty.json` verbatim (a test keeps the two equal; the canonical layout keeps short values on a line and splits long ones). What it shows: one triggered ability with two patterns (`AnyOf`); a `TargetSpec` "target opponent" (one of several in multiplayer); effects in sequence with different actors (`who: Target(0)` for the first three, the default `You` for the last two); a sacrifice whose filter is a creature-or-planeswalker predicate and whose choice belongs to the sacrificing player, not the controller.

What the engine does, in order:
1. **Trigger detection.** Archon enters: the engine emits `SpellResolved` (today) or `ZoneChanged { to: Battlefield }`; `detect` matches `ZoneChange { what: This, to: Battlefield }` of the `AnyOf` against `post`, emits `AbilityTriggered { source: Archon, ability: 1, cause: ZoneChange }` with a snapshot of Archon. Attacking later matches `Attacks` on `AttackersDeclared` the same way. One event triggers once.
2. **Putting it on the stack** at the next priority sweep (APNAP; one trigger, so no `OrderTriggers`): `ChooseTargets { spec: 0, candidates: [each living opponent that can be targeted], min 1, max 1 }` goes to the controller; with a single legal opponent and no Full Control the engine answers it itself. `AbilityPutOnStack` creates the ability object with `targets: [[Player(opp)]]`.
3. **Resolution.** Both players pass; `ResolutionBegan`. Recheck (CR 608.2b): if the opponent is no longer a legal target (they left the game, or gained hexproof), `ResolutionEnded { Fizzled }` and **nothing happens, including the controller's draw and life gain**. Otherwise the steps run:
   - step 0 `Sacrifice`: `Decision { player: opponent, source: Archon's ability, ChooseCards { from: [their creatures and planeswalkers], min 1, max 1 } }`. Addressed to the **target** player, who is not the ability's controller. No legal option: no decision, `NothingToDo`. Answer: `ChoiceMade`, then `ZoneChanged { cause: Sacrificed }` (its replacement processing and its own triggers follow), `StepDone`.
   - step 1 `Discard`: `ChooseCards` from the opponent's hand, addressed to the opponent and private to them; `CardDiscarded`, `StepDone`.
   - step 2 `LoseLife`: `LifeLost { player, 3 }` through the replacement step; `StepDone`.
   - step 3 `Draw` and step 4 `GainLife` for the controller, no decisions.
   - `ResolutionEnded { Resolved }`; the ability object leaves the stack. Triggers caused by the sacrifice, discard and life loss queue meanwhile and go on the stack at the next sweep, after resolution (CR 603.3b).
4. **What the engine needs for this card:** the `Triggered` ability kind and `AnyOf`, `ZoneChange` and `Attacks` patterns with their events; trigger detection with snapshots; the `Ability` stack object and `AbilityPutOnStack`; `ChooseTargets` (with `Opponent` legality); `ChooseCards` addressed to a non-controller with a response clock; the journal events; the `Sacrifice`, `Discard`, `LoseLife`, `Draw` and `GainLife` primitives with `LifeGained` and `LifeLost` events; the `Flying` keyword (blocking restriction in `declare_blockers`). Each of those is a `Caps` entry; until all are in, the support record of this card is `Partial` or `Manual`, not `Verified`.

## Corpus and gaps

`crates/mtg-cards/cards/` (40 cards): Lightning Bolt, Counterspell, Giant Growth, Llanowar Elves, Serra Angel, Doom Blade, Swords to Plowshares, Wrath of God, Rampant Growth, Cultivate, Brainstorm, Opt, Mulldrifter (evoke), Rhystic Study, Hero of Bladehold (attack trigger, tokens, battle cry), Anointed Procession (replacement), Fireball (X, even split, cost per extra target), Izzet Charm (modes), Eternal Witness, Pacifism (Aura), Bonesplitter (Equipment), Flooded Strand (fetchland), Dockside Extortionist (Treasure maker), Tireless Tracker (Clue maker), Burst Lightning (kicker), Faithless Looting (flashback), Bloodbraid Elf (cascade), Jace, the Mind Sculptor (planeswalker), Delver of Secrets // Insectile Aberration (transform), Fire // Ice (split, divided damage, Fuse), Bonecrusher Giant // Stomp (adventure), Sol Ring, Felidar Sovereign (intervening if), Sneak Attack (delayed trigger), Gaea's Cradle (mana per creature), Archon of Cruelty, Forest and Grizzly Bears (no abilities), Questing Beast (`Partial`: three clauses) and Panharmonicon (`Manual`). Tokens: Treasure, Clue, Food.

What the DSL cannot say yet (each is a later variant, see Cross-check):
- Panharmonicon (trigger doubling) and Questing Beast's prevention rule and evasion clause: no `Modification`/`GameRule` for them; they are the `Manual` and `Partial` examples on purpose.
- Fireball's "costs {1} more for each target beyond the first" is expressible (`CostModifier` with `per: Sum[TargetCount(0), -1]`); the engine needs the target count before it prices the spell, so announcement orders targets before cost (CR 601.2c, 601.2f).
- "If you do" (Eternal Witness is fine; "you may sacrifice X. If you do, draw") needs `Condition::Performed`, recorded by the journal as the outcome of a step. Not in the corpus.
- Characteristic-defining power and toughness (`*`), copy effects on permanents (layer 1) and `Exile` with a duration (Oblivion Ring) are reserved: the layers task and the tokens-and-copy task add their variants.
- Mana of any colour as a payment choice (Treasure) needs `PayOptions.sources` to carry a set of colours and `Payment` a pick; engine task, not data.

## Cross-check against other engines

Before freezing the vocabularies we read, for names and shapes only, three engines' card vocabularies: phase.rs (Rust, MIT or Apache-2.0; `types/ability.rs`, `triggers.rs`, `replacements.rs`, `statics.rs`, `parser/`), XMage (MIT; `abilities/` effects, costs, conditions, dynamic values, triggered abilities) and Forge's scripting vocabulary (GPL-3.0; the wiki pages' names only). Nothing from Forge or Magarena is copied; no code or card script from any of them is in this repository.

**Added because of it**
- `TriggerPattern::AnyOf` (phase `EntersOrAttacks`, `AttacksOrBlocks`; XMage `EntersBattlefieldOrAttacks...`): compound triggers are common and the Archon needs one.
- `Effect::Pay { who, cost, paid, unpaid }` instead of an `UnlessPays` verb (phase `PayCost`, XMage `DoIfCostPaid`, Forge `UnlessCost`): "unless pays" and "you may pay, if you do" are one verb.
- `limit` on triggered and activated abilities ("only once each turn") and a `from` zone on abilities (phase and Forge `TriggerZones`, XMage zones on abilities).
- A `Modification` for every layer (phase `ContinuousModification`, XMage `Layer`/`SubLayer`), so the layers task adds behaviour and not data; `TokenRef::CopyOf`; `WinGame`.

**Confirmed by all three and kept:** `DelayedTrigger`, `Rule` (rule-changing effects), cost modification as its own ability kind, `If` and `May` wrappers, a dynamic target-count value, modal choices with per-mode targets.

**Left out, with the reason**
| Primitive | Seen in | Verdict |
|---|---|---|
| Keyword actions: Fight, Explore, Proliferate, Populate, Amass, Connive, Learn, Goad, Detain, Monstrosity, Adapt, Bolster, Support, Venture, Manifest, Cloak, Incubate, Suspect | all | later, one `Effect` variant each in "Keyword actions"; Investigate, Treasure, Food are `CreateToken Named` |
| `RevealUntil`, `ExileTop`, `Dig`, `DigUntil` | phase, Forge, XMage | `LookAtTop` + `Choose` + `Move` already compose "look at N, take some"; `RevealUntil` is added with cascade and discover |
| `CastFromZone`, `FreeCast`, `GrantCastingPermission`, `AsThough` effects (flash, play from graveyard or top of library) | all | later with alternative costs |
| `CopySpell`, `BecomeCopy`, `CopyTokenOf` | all | `TokenRef::CopyOf` now, `CopySpell` later |
| `GainControl`, `ExchangeControl` | all | `Continuous { SetController }` now; exchange later |
| Shields (`PreventDamage`, `CreateDamageReplacement`, `AddTargetReplacement`), redirection | phase, XMage | later: `Effect::Replacement { replacement, duration }` using the same `ReplaceWhen`/`ReplaceBy` |
| `CreateEmblem`, `ExtraTurn`, `SkipTurn`, `AdditionalPhase`, `EndTheTurn`, `SetLifeTotal`, `ExchangeLifeTotals` | all | later or on demand from the coverage dashboard |
| `LoseTheGame`, `RollDie`, `FlipCoin`, `Clash`, `Vote`, `SeparateIntoPiles` | all | `WinGame` now; the rest by dashboard demand (Vote appears in Commander precons) |
| `ChooseOption` as an effect (colour, creature type, card name, player) with a remembered choice (`Ref::Chosen`, `Pred::OfChosen`) | all | later with the `ChooseOption` decision |
| `RememberCard`, `TrackedSet`, `LastCreated`, `ParentTarget`, `Remember`/`Imprint` | phase, Forge | rejected: explicit `as`/`Bound` names do the same and are validated |
| Trigger families: damage dealt or received, life gained or lost, draw, discard, mill, cycle, exile, reveal, shuffle, search, tap, untap, taps for mana, counters added or removed, attach, blocks, becomes blocked or unblocked, spell copied or countered, ability activated, transform, token created, monarch, initiative | all | later, one `TriggerPattern` variant each, backed by the event that does not exist yet |
| "One or more" batch triggers, "first time each turn", "nth card drawn" | XMage `OneOrMore*`, phase `*Once`/`*All` | later: `batch` and `nth` fields on the patterns |
| State triggers | phase `StateCondition`, Forge `Always`, XMage `StateTriggeredAbility` | later: `TriggerPattern::State(Condition)` (CR 603.8) |
| Costs: Exile (hand, graveyard, top), TapUntapped (crew), RemoveCounters (also variable), PayEnergy, ReturnToHand, Mill, Reveal, Exert, Unattach, OneOf | all | later, one `CostItem` each with its choice decision; `Amount::X` may appear in any cost amount |
| `Pred`: counters, shares a quality, entered, attacked or was dealt damage this turn, face down, transformed, historic, commander | phase `FilterProp`, XMage | later; "this turn" ones need a per-turn history in state |
| `Amount`: spells cast this turn (Storm), cards drawn, life gained or lost this turn, devotion, distinct card types, kicker count, event amount ("that much"), size of a bound set ("that many") | phase `QuantityRef`, XMage dynamic values | later, each with the task that needs it (Storm, multikicker, triggers) |
| `Condition`: monarch, initiative, day or night, ring, dungeon, `Performed` | phase, Forge | later; `Performed` with the journal |
| Duration: until host leaves, until next turn of a player, until event, for as long as | phase | later with `Exile`'s `until` |
| Statics: must attack or block, can't be countered, can't gain life, can't draw, cast permissions, trigger doubling, can't search, reveal top, characteristic-defining flag, remove all abilities, remove type or subtype | phase `StaticMode`, Forge, XMage | later: a `Modification` or `GameRule` variant each in the layers and statics tasks |
| Replaced events: draw, gain or lose life, destroy, discard, mill, add or remove counters, tap, untap, enters tapped or with counters, scry | all | later, one `ReplaceWhen` each, as the engine gains the proposal for the event |
| Ability kinds: saga chapters, class levels, level up, opening-hand actions, special actions (morph, suspend) | phase `oracle_saga`/`class`/`level`, XMage `SpecialAction`, `OpeningHandAction` | after M3's core; tracked by the coverage dashboard |

**Structure ideas weighed.** phase.rs splits conditions by context (ability, trigger, static, replacement): rejected, one `Condition` with validation per position. XMage's `Conditional*` and `Optional*` decorators are our `If` and `May` effects. phase's `proposed_event` layer is the same idea as our replacement step. Forge's sub-ability chains are our `effects` lists.

## Tasks

The roadmap carries the order (`ROADMAP.md`, M3). In short: the data model (this record) first; then card assembly (so a deck of oracle ids becomes `CardDefinition`s and the testkit can build cards from the corpus); then the pipeline core (ability objects, zone-change counter, snapshots, journal, interpreter with one stub file per `Effect` variant, the replacement seam, `ChooseTargets` with the 608.2b recheck, and every other new `DecisionKind`, event and redaction arm declared once as types). After that, in parallel because each touches its own files: decision kinds (engine, protocol, web prompt, tests), effect primitives, keywords, the trigger pipeline, the replacement pipeline, layers. Tokens, copy, X costs, alternative and additional costs, and modal spells come last, since they use the pieces above.
