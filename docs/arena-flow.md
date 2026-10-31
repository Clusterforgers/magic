# Arena flow: priority, stops, yields and the pace tell

Research on how MTG Arena handles priority (primary button, stops, auto-pass, Full Control, yields, timers), what each gives away to an opponent, and the proposals P1 to P10 that copy it, with their state. It covers two-player play only.

Tags: [official] a Wizards page or developer; [community] guides, forums, bots and the client's data-mined protocol; [unverified] an inference, or no source found. `Sn` is the source below. Wizards' help and patch-note pages refuse automated fetches and its feedback forum is private, so most Arena detail is [community].

## Summary

- Arena has our rule and our tell: it "only stops if you have an available play" [community S11], and one guide calls the information its stops give away the largest disadvantage of playing there [community S8]. Wizards' guidance is to set a stop or use Full Control [official S1, S7]; no statement addresses the tell.
- Ours copies the default windows, stops and Full Control and hides the pace better than Arena does (our own addition): a seat that cannot act at a combat or stack window waits 250 to 700 ms and passes as a batch of its own.
- The principle: decide when a seat is asked from the table (step, stack, stops, mode), never from its hand. Only the default windows read the hand (`can_act`), and the dwell hides that.
- Arena's yields have two strengths (Enter ends when the opponent acts, Shift+Enter does not) [community S8, S9, S25]. Ours is the weak one, so Space and Enter are different keys.
- Not copied: the hovered hand card relayed to the opponent (a leak [community S22]) and timers beyond a bank and a delayed start.

## Sources

- S1 [official] Arena Announcements 2025-10-27, "A Quick Guide to Smart Priority for Powered Cube": https://magic.wizards.com/en/news/mtg-arena/announcements-october-27-2025
- S2 [official, reposted] patch 0.12.00.00, 2019-02-14 (Resolve All, Pass Turn, timers): https://mtgarena.pro/news/mtga-february-patch-0-12-00-00/
- S3 [official, reposted] patch 1.00.00, 2019-09 (held priority, Auto-Pay): https://mtgazone.com/1-00-00-patch-notes/
- S4 [official, seen only as search summaries; the pages return 403] patch notes 2022.20.0, 2024.42.00, 2025.52.00, 2026.63.0, .10 and .20: https://mtgarena-support.wizards.com/hc/en-us/articles/9927983161876-Patch-Notes-2022-20-0 · https://mtgarena-support.wizards.com/hc/en-us/articles/31307518959252-Patch-Notes-2024-42-00 · https://mtgarena-support.wizards.com/hc/en-us/articles/41292678660244-Patch-Notes-2025-52-00 · https://mtgarena-support.wizards.com/hc/en-us/articles/53820137910932-Patch-Notes-2026-63-0 · https://mtgarena-support.wizards.com/hc/en-us/articles/54110909948820-Patch-Notes-2026-63-10 · https://mtgarena-support.wizards.com/hc/en-us/articles/54297570573204-Patch-Notes-2026-63-20
- S5 [official] Magic Online tips (stops, yields, hold priority; the sibling client): https://www.mtgo.com/getting-started/getting-started-tips-tricks
- S6 [official] Magic Tournament Rules 4.2, shortcuts: https://blogs.magicjudges.org/rules/mtr4-2/
- S7 [official, a developer in a community thread] WotC_Jay on r/MagicArena, 2019: https://devtrackers.gg/magic-arena/p/4c9a9e32-is-it-feasible-for-the-game-to-not-give-up-the-fact-that-you-have-instants-in-your-hand-by-constantly-asking-if-you-want-to-respond-to-things
- S8 [community] MTG Arena Zone, hot keys and interface (2022): https://mtgazone.com/arena-hot-keys-and-interface-guide-simplify-your-game-with-these-easy-tricks/
- S9 [community] MTG Arena Zone, playing a match: https://mtgazone.com/playing-a-match/
- S10 [community] MTG Arena Zone, interface and add-ons (2022): https://mtgazone.com/using-arena-interface-and-add-ons/
- S11 [community] Draftsim, shortcuts: https://draftsim.com/mtg-arena-keyboard-shortcuts/
- S12 [community] Draftsim, tips and tricks: https://draftsim.com/mtg-arena-tips-and-tricks/
- S13 [community] Draftsim, settings: https://draftsim.com/mtg-arena-settings/
- S14 [community] Hipsters of the Coast, closed beta 2018: https://www.hipstersofthecoast.com/2018/03/how-to-play-mtg-arena-the-in-match-screen/
- S15 [community] MTGA Assistant, shortcuts: https://mtgaassistant.net/Article/All-MTG-Arena-Keyboard-Shortcuts
- S16 [community] MTGA Assistant, match peculiarities (timers): https://mtgaassistant.net/Article/MTG-Arena-Match-Playing-Peculiarities-Guide
- S17 [community] beginner's guide, hotkeys: https://sites.google.com/view/beginners-guide-to-mtga/hotkeys-and-using-the-interface
- S18 [community] Draftsim on roping: https://draftsim.com/mtg-arena-roping/
- S19 [community] Steam, "i'm so tired of clicking resolve in this game" (2025-04): https://steamcommunity.com/app/2141910/discussions/0/597398236592900188/
- S20 [community] Steam, "So what good is the Resolve All button?" (2025-06): https://steamcommunity.com/app/2141910/discussions/0/597404077749453144/
- S21 [community] Steam, "So explain this to me", a timeout loss (2023-11 to 2024-04): https://steamcommunity.com/app/2141910/discussions/0/3969421533314928344/
- S22 [community] Steam, "Stop making my hand light up for my opponent" (2025): https://steamcommunity.com/app/2141910/discussions/0/734750159229956628/
- S23 [community] Steam, timers too long (2024-06): https://steamcommunity.com/app/2141910/discussions/0/6579276970675600456
- S24 [community] Steam, stops and Full Control: https://steamcommunity.com/app/2141910/discussions/0/6579276631058759450/
- S25 [community, data-mined from the client's GRE protocol DLL] `messages.proto`: https://github.com/riQQ/MtgaProto. The enums are facts; what they do in the UI is a reading [unverified].
- S26 [community, data-mined] the settings message in a Go mirror: https://pkg.go.dev/github.com/di-wu/mtga/thread/connect_resp
- S27 [community, weak] a bot's README that names buttons: https://github.com/Barrylim366/mtga-farm-bot
- S28 [community, weak, search snippet only] fan wiki tutorial walkthrough ("All Attack"): https://magicarena.fandom.com/wiki/New_Player_Experience

## Arena behaviour

### 1. The primary button

One big orange button at the bottom right passes priority; its label follows the state. No source lists every label, so this is what is attested.

| Label | Shows when | What it does |
|---|---|---|
| Next | your priority, empty stack | passes priority to the next step [community S8, S12, S15] |
| Resolve | an object is on the stack and you hold priority | passes so the top object resolves; the other player is asked too for each of your spells, and skipping the click runs your own clock down [community S19, S21, S23] |
| Resolve All | several objects on the stack | resolves them all; pressing again stops so you can respond; objects added later ask again [official S2; community S20] |
| Pass Turn (orange arrow, a toggle) | always | passes the rest of the turn but stops at your stops; Enter or the arrow turns it off; over a stack it acts as Resolve All; the server holds it (a 2019 fix stopped it carrying into the next turn when the server was ahead of the client's view) [official S2; community S8, S9] |

The same button is "No Blocks" at declare blockers, All Attack sends every creature and Auto-Pay chooses the lands [official S3; community S27, S28].

Hotkeys [community S8, S11, S15, S17]:

- Space passes once and confirms an OK. Enter is "Pass Until Response": through the turn until the opponent acts or you can answer something, with no priority back for triggers; pressed before combat it can skip your own combat.
- Shift+Enter (left Shift) or Shift-click on the button is "Pass Turn", MTGO's F6: whatever the opponent does, and on the opponent's turn it skips what is left of it; priority comes back only to block.
- Ctrl is Full Control, Ctrl+Shift holds it, and the same keys release it.

### 2. Stops

- The ladder shows the steps as icons by each portrait, orange for your turn and blue for the opponent's [community S8, S9]. The client protocol has eleven `StopType`s, one per step where a player gets priority, each Set or Clear with a scope of AnyPlayer, Opponents or Team [community S25].
- A click toggles a stop, best set ahead of time because the server can be ahead of the client's animations [community S9]. A stop is Full Control for that phase: Pass Turn still stops at it, and moving past it ends Full Control [community S9, S24]; "stops only last until you reach that phase" [community S12]. Whether a click can make a lasting stop is [unverified] (the protocol keeps `stops` and `transientStops`).
- A Wizards developer, asked for a "pass until" feature: "You can get exactly the functionality you mention by setting a stop on the phase where you want to act ... That will pass to anything the opponent does until you get to the phase you put a stop in" [official S7].
- Defaults: no priority in your own upkeep or end step, and usually none in response to your own triggers. "Smart priority" is a list of card-specific exceptions that set the stop for you (Urza's Saga, Sheoldred the Apocalypse and others); for the rest, "set a stop or go into full control" [official S1]. `smartStopsSetting` turns it on or off [community S26].
- The default stops of the opponent's turn, and whether being attacked while you hold an instant adds one: no source [unverified]. MTGO ships the common stops on for both sides and always gives you declare attackers and blockers when you control a creature [official S5].

### 3. Auto-pass and Full Control

- Arena "only stops if you have an available play" [community S11], "will automatically pass priority through your opponent's turn if you have nothing to play" [community S17] and passes the turn after a land drop with nothing else to do [community S12]. That is our `can_act`. A play is a land to drop, a spell to cast (from the hand, the command zone or elsewhere), an ability that is not a mana ability and can be paid, or a special action; a mana ability never is one, a filter land that costs mana included, and a Clue, an equip or a scry land is, so a seat that holds one is asked at each default window where it can use it, with nothing in its hand.
- It never skips a decision that needs an answer (blocks [community S8], targets [community S19]), a stop you set [community S9] or Full Control.
- Full Control: Ctrl holds priority for the phase and ends once you pass, Ctrl+Shift holds it until toggled [community S8, S10, S24]. It answers your own abilities and triggers, spends pool mana by hand, and overrides Auto Tap, Auto Order Triggered Abilities, Auto Choose Replacement Effect and Auto Assign Combat Damage [community S10, S13, S24].
- The price of windows: Arena sometimes hands priority to a player who cannot answer, who then loses time, or the game, by not clicking [community S21].

### 4. Yields

- Resolve All and Pass Turn are the yields (section 1) [official S2]. Enter ends when the opponent acts or you can answer something; Shift+Enter only at a stop or a decision that needs you [community S8, S11, S15]. Resolve All ends when something new goes on the stack, so a long trigger chain asks again and again, which players dislike [official S2; community S19, S20].
- Yields live on the server (`autoPassOption`, `stackAutoPassOption`; which key sets which value is [unverified] [community S25, S26]). `AutoYield` is per ability or card title, `AutoAnswer` keeps answers to optional prompts, and trigger order can persist once or always [community S25].
- MTGO is the checklist: pass until you can respond, yield through the turn, yield to a stack object this turn or always, "no possible play: yield all stops" (our default windows); you still confirm attackers and blockers [official S5].

### 5. Information hiding

What an opponent can infer in Arena:

- Pace. A land played and a pass that still gets priority on your turn means an instant, and a pause after a cast hints at a counterspell: "there will be a noticeable stutter compared to the smoothness that Arena passes if the opponent has nothing" [community S8]. Players say it too: "if the game auto passed more it would give away you have nothing to play"; "having a stop constantly is a decent way to leave your opponent wondering" [community S19].
- Hover. A hand card you mouse over lights up on the opponent's screen, and no setting turns it off; players call it a flaw and a bluff tool. The protocol has `OnHover` and `OnSelect` messages [community S22, S25].
- The timer rope is on the avatar, and players report each other's roping [community S14, S18].
- The state names who holds priority and who is deciding; whether the other seat sees a player's `pendingMessageType` is [unverified] [community S25].

Arena mitigates nothing in the game. The player's tools are stops on every phase (a better bluff than constant Full Control, which is "very obvious" and can signal an empty hand), Full Control, tapping lands without casting, and Space quickly after plays that need no answer [community S8, S12]. No fixed delay or uniform animation was found [unverified]. Paper Magic skips priority passes "to the mutual understanding of all players" and bars requesting priority and taking no action with it [official S6]; hesitation is a tell the rules do not equalise [unverified].

### 6. Timers

- Official 2019: the non-active player's timer "no longer resets between phases" (anti-stall); "a delay before your timer starts whenever you are prompted for a response"; an inactivity timer of 120 s, after which a player who has not responded forfeits when the current timer expires [official S2].
- Guides: a rope burns 30 s when one action takes too long; a timeout is earned after three turns without the rope; best of three has a 30-minute match clock; two automatic passes in a row lose the game [community S16]. Figures differ by era (the 2018 beta used a fuse of just over a minute, then 45 s [community S14]), so today's are [unverified].
- The protocol has timer kinds Decision, Inactivity, ActivePlayer, NonActivePlayer, Delay and MatchClock [community S25]. At expiry the server answers for the player (a pass, the next phase) [community S14, S16, S25]. Complaints: long stacks run the clock of the player who cannot answer [community S21]; timers are too long [community S23].

### 7. Inputs for combat, triggers, payment and targets

- Combat: the board is the picker and the server validates each click (legal recipients per attacker including planeswalkers, `mustAttack`, warnings such as CannotAttackAlone and MustBeBlocked), then a submit. Settings: Auto Assign Combat Damage and Gameplay Warnings [community S25, S13].
- Triggers: Auto Order Triggered Abilities is a setting that Full Control overrides [community S8, S13]. Hold priority: none per spell, Full Control or stops instead; you keep priority when an effect you control is put on the stack by an opponent's action [official S3]. MTGO: hold Ctrl while casting [official S5].
- Payment: Auto Tap is a setting, hovering a spell previews the lands it will tap, and manual mana needs Full Control [official S3; community S9, S10, S13, S17]. Targeting: a request and a "Submit N" confirmation ("Submit 0" skips an optional target, "Done" closes scry or surveil) [community S25, S27]. Undo: Z before anything resolves [community S8, S25].

## Information leaks

What ours leaks:

- Windows. A seat is asked only at the default windows (its own main phases, declare attackers and blockers, an object of another seat on the stack) and at its stops; elsewhere it is passed for at once, so a pause there means nothing. Its own upkeep and end step are no windows unless it sets a stop, as in Arena [official S1].
- Pace. At a default window a seat that can act makes the table wait for a human (10 s as a non-active seat, unbounded as the active one); a seat that cannot waits 250 to 700 ms and passes as a step and batch of its own, so the other seats see the same stream ([server.md](server.md), Dwell). Its own main phase with an empty stack is passed at once, because what it plays is in plain sight.
- What remains: how long a seat that can act takes (a pause past 700 ms or a pass before 250 ms is its own); a pass at the response-clock deadline, which reads as "was asked and sat on it"; and the choices of stops, Full Control and yields. Arena shows the same choices; constant Full Control is "very obvious" [community S12].
- The log is uniform: `PriorityGranted` and `PriorityPassed` are public events for every window, skipped or not. Arrival time and batch shape could leak; the text does not.
- Payments. An effect that asks a player to pay ("unless that player pays {2}", a shock land's two life) asks every player it names the same question at the same moment with the same clock, whether they can pay or not; a player who cannot may only say No, and the table or the clock answers No for them ([engine.md](engine.md), Paying in an effect). A skipped question would show that the player could not pay.
- Timer bank. The bank and the spend of an extension at a deadline are public (`PlayerView.bank`, `Bank`): they follow from how long a seat takes to answer, which every seat can see, and never from the cards. The clock fields (`deadline_ms`, `show_after_ms`) read the decision kind and the seat alone, so a seat that can act and one that cannot are sent the same ones.
- Not leaked: there is no hover or selection relay, and clocks, previews, `Welcome` and the `Stops`, `FullControl` and `Yielding` notices reach only their seat. The client's turn feed uses public events only, the same for every seat, and never marks another seat's pass as automatic or timed ([ui-design.md](ui-design.md), Turn and focus).

Mitigations, ranked by value for effort:

| # | Mitigation (source) | Hides | State |
|---|---|---|---|
| 1 | Stops and Full Control: the player chooses when to be asked and can bluff a window (Arena [official S1, S7]) | whatever that player decides to | done (P1, P2, P4 to P6) |
| 2 | Table-driven windows with Arena's default table, none in own upkeep or end step unless set (Arena [official S1]) | pauses in steps where only a holder of an instant stops | done (P2) |
| 3 | A window the seat cannot use is held for 0.25 to 0.7 s, then passed as its own step and batch (ours, not in Arena [unverified]) | the instant pass and the batch burst | done (P3); about 0.5 s per idle seat and window |
| 4 | Leak test over what the other seats receive (ours) | keeps 2 and 3 true | done (P8), for combat windows and for a spell or a triggered ability on the stack |
| 5 | Strict table: auto-pass off for every seat (ours; MTGO's default stops are the nearest [official S5]) | everything, paid in clicks | `MTG_AUTO_PASS=0`, server-wide |
| 6 | Never add a hover or selection relay, a thinking cue or the kind of decision owed to the views (Arena leaks these [community S22, S25]) | new tells | holds |
| 7 | A payment is asked of every player an effect names, with No the only answer for one who cannot pay (ours) | the skipped question of a player who cannot pay | done (g8), tested in `pay_leak.rs` |

The client never paces steps: the fx layer must not wait for a cadence, and P3 does it at the source.

## Mapping

| Arena feature | Ours | State |
|---|---|---|
| Main button relabels | `Next: <step>`, `Resolve <spell>`, `End turn`, `Continue` (`passWords.ts`) | kept |
| Space: one pass | the primary button | done (P7) |
| Enter: pass until response | the `EndOfTurn` yield ("End turn"); one pass where that button is not drawn (Full Control, the End step) | done (P7); an opponent's new object ends it |
| Shift+Enter, Shift-click: Pass Turn | none | not built (P7), only if a player asks |
| Resolve All, press again to stop | `YieldUntil::Resolve` (menu item over a stack); "Stop passing" cancels | done |
| Pass Turn held by the server | `EndOfTurn` and `MyTurn` record the turn they were asked in; `Active::ends_at` never reads `can_act` | done |
| Stops, interrupt Pass Turn | `Stop { side, step, keep }`; a ladder of ten steps per side (first-strike damage joins while current or set); a stop ends a yield | done (P1, P2, P6) |
| Full Control: Ctrl, Ctrl+Shift | `NextWindow` (Ctrl) and `Held` (Ctrl+Shift), a toggle beside the ladder | done (P4, P5) |
| No window unless a play; none in own upkeep or end step | default windows ask a seat that can act; elsewhere it is passed for at once | done (P2) |
| No equivalent | dwell for a seat that cannot act at a combat or stack window | done (P3), ours |
| Smart priority, auto-order triggers, per-ability yield | triggers exist and are ordered by the player only when the order can matter; smart stops, an order preference and the per-ability yield are missing | open (P10) |
| Timers: rope, bank, delayed start, inactivity | response, block and think clocks, a bank of three extensions earned by three turns without a clock, a delay before the countdown, an optional active-seat clock; no inactivity timer | done (P9) |
| Mulligans decided by both players at once | every seat is asked to keep or mulligan at once, each on its own think clock; the others read "deciding" or "kept" | done |
| Auto-tap, Auto-Pay, land preview, Undo (Z) | "Pay automatically", "Pay the rest for me", payment Undo and Clear, Cancel casting; no hover preview | optional, later |
| Hand hover relay; hold priority | no relay; none per spell in either, Full Control and stops cover it | keep |
| Blocks asked whenever possible, `NothingToDo` otherwise; expiry answers for the player; combat choices validated while clicking; All Attack, No Attack, No Blocks | `fall_back`, `Preview`, "Attack with all", "No attack", "No blocks" | in place |

## Proposals

State follows the game flow item of `docs/ROADMAP.md`; the built designs are in [protocol.md](protocol.md), [server.md](server.md) and [web.md](web.md).

- **P1 Stops on the wire** (done): `Stop`, `SetStop`, `Welcome.stops`.
- **P2 Stops in the actor with Arena's default table** (done): a seat is asked only at the default windows and at its stops, whether or not it can act at a stop. Arena's table for the opponent's end step is [unverified]; ours has no default window there.
- **P3 Idle windows as hidden clocks** (done): a seat that cannot act at a default window waits 250 to 700 ms (`MTG_DWELL_MS`, 0 = off), then passes as its own step and batch. Yields, Full Control and its own empty main phase are exempt, and only the wall clock changes, so the log and the hash do not.
- **P4 Full Control** (done): `FullControl { mode: Off | NextWindow | Held }`. The seat is asked at every window and never passed or yielded for; the response clock never reads `can_act`.
- **P5 Web session for stops and Full Control** (done): `session.setStop`, `setFullControl`, Ctrl and Ctrl+Shift.
- **P6 Stop ladder** (done): two rows of step buttons (yours, theirs) and the Full Control toggle.
- **P7 Keys like Arena** (done): Space passes once, Enter asks the end-of-turn yield. Not built: Shift+Enter and Shift-click as the strict pass, a `YieldUntil` strength that an opponent's play does not end, conditional on a player asking.
- **P8 Leak test** (done): `crates/mtg-server/tests/it/pace_leak.rs` requires two seeded games that differ by one hidden card to send the other seat the same messages in the same batches, 250 to 700 ms apart; with the dwell off it finds the difference. It covers combat windows and a spell or a triggered ability on the stack. The client's turn feed gets the same test ([ui-design.md](ui-design.md), U6).
- **P9 Timer bank** (done): a seat earns an extension (at most three) by finishing three turns of its own without a clock running out; at the deadline it spends one and the clock runs `MTG_EXTENSION_SECS` longer before the table answers; `Decision.show_after_ms` delays the countdown (`MTG_CLOCK_GRACE_SECS`); `MTG_ACTIVE_SECS` gives the active seat a clock. The bank is public (`PlayerView.bank`, `Bank`).
- **P10 With triggers** (open): triggers are built, and the player orders them only when the order can matter ([engine.md](engine.md), Triggered abilities). Open: smart stops (a step where an object can answer becomes a window), per-ability yield (`AutoYield`), a preference to order by timestamp, damage auto-assign, and the engine's answer to a one-option decision, off under Full Control ([cards-dsl.md](cards-dsl.md)).
