# Arena flow: priority, stops, yields and the pace tell

Research on how MTG Arena handles priority (primary button, stops, auto-pass, Full Control, yields, timers), what each gives away to an opponent, and the proposals P1 to P10 that copy it. It covers two-player play only.

Tags: [official] a Wizards page or developer; [community] guides, forums, bots and the client's data-mined protocol; [unverified] an inference, or no source found. `Sn` is the source below. Wizards' help and patch-note pages refuse automated fetches and its feedback forum is private, so most Arena detail is [community].

## Summary

- Arena has our rule and our tell. It "only stops if you have an available play" [community S11], and one guide calls the information its stops give away the largest disadvantage of playing there [community S8]. Wizards' guidance is to set a stop or use Full Control [official S1, S7]; no statement addresses the tell. Copying Arena means stops and Full Control; hiding the pace better than Arena does is our own addition.
- Ours is at least as sharp: an idle pass lands in the same batch as the action that caused it ([server.md](server.md)), and a seat that can act is asked in its own upkeep and end step, where Arena's default gives no priority [official S1].
- The principle: decide when a seat is asked from the table (step, stack, stops), never from its hand. Yields follow it (`yields.rs`); auto-pass does not.
- Arena's server-side yields have two strengths (Enter ends when the opponent acts, Shift+Enter does not) [community S8, S9, S25]. Ours is the weak one, so Space and Enter must not be the same key.
- Not copied: relaying the hovered hand card to the opponent (a leak [community S22]) and Arena's timer machinery beyond a bank and a delayed start.

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
| Next | your priority, empty stack | passes priority; the game goes to the next step [community S8, S12, S15] |
| Resolve | an object is on the stack and you hold priority (the other player is asked too for each of your spells, sometimes with nothing to answer) | passes so the top object resolves; skipping the click runs your own clock down [community S19, S21, S23] |
| Resolve All | several objects on the stack | resolves them all; pressing again stops so you can respond; objects added later ask again [official S2; community S20] |
| Pass Turn (orange arrow, a toggle) | always | passes the rest of the turn but stops at your stops; Enter or the arrow turns it off; over a stack it acts as Resolve All; the server holds it (a 2019 fix stopped it carrying into the next turn "if the server's state was ahead of the player's visual state") [official S2; community S8, S9] |
| Auto-Pay (was Pay) | paying a cost | Arena chooses the lands [official S3] |
| Submit, Submit 0, Done | a selection prompt | confirms it; "Submit 0" skips an optional target, "Done" closes scry or surveil [community S27] |
| No Blocks, All Attack | declare blockers, declare attackers | the bottom-right button is both pass and "No Blocks"; All Attack sends every creature [community S27, S28] |

Hotkeys [community S8, S11, S15, S17]:

- Space passes once and confirms an OK. Enter is "Pass Until Response": through the turn until the opponent acts or you can answer something, with no priority back for triggers; pressed before combat it can skip your own combat.
- Shift+Enter (left Shift) or Shift-click on the button is "Pass Turn", MTGO's F6: whatever the opponent does; priority comes back only to block.
- Ctrl is Full Control, Ctrl+Shift holds it, and the same keys release it. Z undoes taps and other actions that did not use the stack.

### 2. Stops

- The ladder: upkeep and draw, main 1, beginning of combat, declare attackers, declare blockers, damage, main 2 and end step, as icons by each portrait (orange for your turn, blue for the opponent's) plus icons for the current turn and combat [community S8, S9].
- The client protocol has eleven `StopType`s (upkeep, draw, precombat main, beginning of combat, declare attackers, declare blockers, combat damage, end of combat, postcombat main, end step, first-strike damage; no untap, no cleanup), each Set or Clear with a scope of AnyPlayer, Opponents or Team [community S25].
- A click toggles a stop; set it ahead of time, since the server can be ahead of the client's animations [community S9].
- A stop is Full Control for that phase: Pass Turn still stops at it, and moving past it ends Full Control [community S9, S24]. "Stops only last until you reach that phase" [community S12]. The protocol keeps `stops` and `transientStops`; whether a click can make a lasting stop is [unverified].
- A Wizards developer, asked for a "pass until" feature: "You can get exactly the functionality you mention by setting a stop on the phase where you want to act ... That will pass to anything the opponent does until you get to the phase you put a stop in" [official S7].
- Defaults: no priority in your own upkeep or end step, and usually none in response to your own triggers. "Smart priority" is a list of card-specific exceptions that set the stop for you (Urza's Saga, the evoke elementals, Goblin Rabblemaster, Sheoldred the Apocalypse, a creature flashed in at your end step); for the rest, "set a stop or go into full control" [official S1]. `smartStopsSetting` turns it on or off [community S26], and patches keep adding per-card stops [official S4].
- The default stops of the opponent's turn, and whether being attacked while you hold an instant adds one: no source [unverified]. MTGO ships the common stops on for both sides and always gives you declare attackers and blockers when you control a creature [official S5].

### 3. Auto-pass and Full Control

- Arena "only stops if you have an available play" [community S11], "will automatically pass priority through your opponent's turn if you have nothing to play" [community S17] and passes the turn after a land drop with nothing else to do [community S12]. That is our `can_act`.
- It never skips a decision that needs an answer: blocks (Pass Turn hands priority back to block [community S8]) and targets ("New trigger that requires you to select a target will always stop" [community S19]). It never skips a stop you set (Pass Turn still stops there [community S9]) or Full Control.
- Full Control: Ctrl holds priority for the phase and ends once you pass, Ctrl+Shift holds it until toggled [community S8, S10, S24]. It answers your own abilities and triggers, spends pool mana by hand, and overrides Auto Tap, Auto Order Triggered Abilities, Auto Choose Replacement Effect and Auto Assign Combat Damage [community S10, S13, S24].
- The price of windows: Arena sometimes hands priority to a player who cannot answer, who then loses time, or the game, by not clicking [community S21].

### 4. Yields

- Resolve All and Pass Turn (section 1) [official S2]. Enter ends when the opponent acts or you can answer something; Shift+Enter only at a stop or a decision that needs you, and on the opponent's turn it skips what is left of it so you are asked only to block [community S8, S11, S15].
- Resolve All ends when something new goes on the stack, so a long trigger chain asks again and again, which players dislike [official S2; community S19, S20].
- Yields live on the server. The protocol's `autoPassOption` is one of Turn, UnlessAction, EndStep, UnlessOpponentAction, ResolveMyStackEffects, FullControl, ResolveAll, with a separate `stackAutoPassOption`; which key sets which value is [unverified] [community S25, S26]. `AutoYield` is per ability or card title, `AutoAnswer` keeps answers to optional prompts, and trigger order can persist once or always [community S25].
- MTGO is the checklist: pass until you can respond, yield through the turn, yield to a stack object this turn or always, "no possible play: yield all stops" (our auto-pass); you still confirm attackers and blockers [official S5].

### 5. Information hiding

What an opponent can infer in Arena:

- Pace. A player who plays a land, passes and still gets priority on your turn has an instant, and a pause after a cast hints at a counterspell, since "there will be a noticeable stutter compared to the smoothness that Arena passes if the opponent has nothing" [community S8]. Players say it too: "if the game auto passed more it would give away you have nothing to play"; "having a stop constantly is a decent way to leave your opponent wondering" [community S19].
- Hover. A hand card you mouse over lights up on the opponent's screen, and no setting turns it off; players call it a flaw and a bluff tool. The protocol has `OnHover` and `OnSelect` messages [community S22, S25].
- The timer rope is on the avatar, and players report each other's roping [community S14, S18].
- The state names who holds priority and who is deciding; whether the other seat sees a player's `pendingMessageType` is [unverified] [community S25].

What mitigates it:

- Arena mitigates nothing in the game. The player's tools are stops on every phase (a better bluff than constant Full Control, which is "very obvious" and can signal an empty hand), Full Control, tapping lands without casting, and Space quickly after plays that need no answer [community S8, S12]. No fixed delay, minimum think time or uniform animation was found [unverified].
- Paper: shortcuts skip priority passes "to the mutual understanding of all players", and "a player may not request priority and take no action with it" [official S6]. Hesitation is a tell the rules do not equalise [unverified].

### 6. Timers

- Official 2019: the non-active player's timer "no longer resets between phases" (anti-stall); "a delay before your timer starts whenever you are prompted for a response"; an inactivity timer of 120 s, after which a player who has not responded forfeits when the current timer expires [official S2].
- Guides: a rope burns 30 s when one action takes too long; a timeout is earned after three turns without the rope; best of three has a 30-minute match clock; two automatic passes in a row lose the game [community S16]. Figures differ by era (the 2018 beta used a fuse of just over a minute, then 45 s [community S14]), so today's are [unverified].
- The protocol has timer kinds Decision, Inactivity, ActivePlayer, NonActivePlayer, Delay and MatchClock, a timeout count, per-format packages and eight versions [community S25].
- At expiry the server answers for the player (a pass, the next phase) [community S14, S16, S25]. Complaints: long stacks run the clock of the player who cannot answer [community S21]; timers are too long [community S23].

### 7. Inputs for combat, triggers, payment and targets

- Combat: the board is the picker and the server validates each click (legal recipients per attacker including planeswalkers, `mustAttack`, warnings such as CannotAttackAlone and MustBeBlocked), then a submit. Settings: Auto Assign Combat Damage and Gameplay Warnings [community S25, S13].
- Triggers: Auto Order Triggered Abilities is a setting that Full Control overrides [community S8, S13].
- Hold priority: none per spell; Full Control or stops instead. You keep priority when an effect you control is put on the stack by an opponent's action [official S3]. MTGO: hold Ctrl while casting [official S5].
- Payment: Auto Tap is a setting, hovering a spell previews the lands it will tap, and manual mana needs Full Control [official S3; community S9, S10, S13, S17].
- Targeting: a request and a "Submit N" confirmation [community S25, S27]. Undo: Z before anything resolves [community S8, S25].

## Information leaks

What ours leaks:

- Pace. With auto-pass on (the default), a seat whose `can_act` is false is passed inside the actor's step: no wait, and its pass is in the batch of the answer that caused it. A seat that can act makes the table wait for a human, up to 10 s as a non-active seat, unbounded as the active one. On the opponent's turn `can_act` means an instant or flash card with mana up, so the pace is the secret itself.
- With auto-pass on, only a seat that can act is delivered a window and has the clock, so a pass landing exactly at the 10 s deadline reads as "had something, dithered".
- A seat that can act is asked in its own upkeep and end step, where Arena's default gives no window [official S1]: more windows where a pause means something.
- The log is uniform: `PriorityGranted` and `PriorityPassed` are public events for every window, skipped or not. Arrival time and batch shape leak; the text does not.
- The only knob is the per-seat auto-pass switch. Off gives up to ten prompts a turn per seat, uniform but slow, and flipping it mid-game changes the pace the table sees. Stops are stored but do not change when a seat is asked, so a player can neither ask for a window nor bluff one.
- Not leaked: there is no hover or selection relay, and clocks, previews and `Welcome.auto_pass` reach only their seat.

Arena leaks pace (stops), the hovered hand card, the rope and possibly the kind of answer owed (section 5).

Mitigations, ranked by value for effort:

| # | Mitigation | Source | Cost | Hides |
|---|---|---|---|---|
| 1 | Stops and Full Control: the player chooses when to be asked and can bluff a window | Arena [official S1, S7] | medium (P1, P2, P4 to P6) | whatever that player decides to |
| 2 | Table-driven windows with Arena's default table (none in own upkeep or end step unless set) | Arena [official S1] | small, inside P2 | pauses in steps where only a holder of an instant stops |
| 3 | A window the seat cannot use is held for a human-like 0.25 to 0.7 s, then passed as its own step and batch | ours, not in Arena [unverified] | small (P3), about 1 s per window a seat could use | the instant pass and the batch burst |
| 4 | Leak test over what the other seats receive | ours | small (P8) | keeps 2 and 3 true |
| 5 | Strict table: auto-pass off for every seat | ours (MTGO's default stops are the nearest [official S5]) | tiny, a creation option | everything, paid in clicks |
| 6 | Never add a hover or selection relay, a clock or "thinking" cue for opponents, or the kind of decision owed to the views | Arena leaks these [community S22, S25] | none | avoids new tells |

The client never paces steps: the fx layer must not wait for a cadence, and P3 does it at the source.

## Mapping

| Arena feature | Ours | Change |
|---|---|---|
| Main button relabels (Next, Resolve) | `Next: <step>`, `Resolve <spell>`, `End turn`, `Continue` (`passWords.ts`) | keep; the words say more than Arena's |
| Space one pass, Enter pass until response, Shift+Enter pass turn | Space and Enter both press the primary button | P7 |
| Resolve All, press again to stop, new object asks again | `YieldUntil::Resolve`, `CancelYield`, ends on an opponent's new object | keep |
| Pass Turn held by the server for its turn | yields record the turn they were asked in | keep |
| A yield ignores what the seat could do | by design | keep; extend to auto-pass (P2, P3) |
| Stops: two sides, eleven steps, interrupt Pass Turn | wire messages and per-seat storage only ([protocol.md](protocol.md)) | P2, P5, P6 |
| Full Control: Ctrl once, Ctrl+Shift held | wire messages and storage only; auto-pass off is a crude held mode | P4, P5 |
| No window unless a play | `can_act`, per-seat switch, default on | keep; P2 makes the windows table-driven |
| No window in own upkeep or end step by default | we ask when `can_act` | P2 default table |
| Smart priority, card-specific stops | missing (no triggers) | P10 |
| Timers: rope, bank, delayed start, inactivity | 10 s on a non-active seat's priority windows, 45 s for blocks, none for the active seat | P9 |
| Auto-tap, Auto-Pay, land preview | "Pay automatically", "Pay the rest for me", no hover preview | optional, with M3 |
| Undo (Z) | payment Undo and Clear, Cancel casting | later |
| Auto-order triggers, per-ability yield | missing (needs triggers) | P10 |
| Hand hover relayed to opponents | absent | keep absent |
| Hold priority | none in Arena either | Full Control and stops cover it |
| Blocks asked whenever possible, `NothingToDo` otherwise; expiry answers for the player (`fall_back`); combat choices validated while clicking (`Preview`); All Attack, No Attack, No Blocks | in place | none |

Constraints on the existing yields: a set stop interrupts a yield (P2); Enter is the `EndOfTurn` yield, not the primary button (P7); cancelling a `Resolve` yield is one click in the same place (Arena: press the button again).

## Proposals

About 15 minutes each, in order of value.

- **P1 Stops on the wire**: `Stop`, `SetStop` and `Welcome.stops`, specified in [protocol.md](protocol.md).
- **P2 Stops in the actor with Arena's default table**: a seat is asked only at the default windows (own main steps, declare attackers and blockers, a new opponent object on the stack; the opponent's end step is [unverified]) and at its stops.
  - One-turn stops clear when the step ends; a stop interrupts a yield, and the decision is sent even when `can_act` is false.
  - Tests: a table test, and bot games equal to the bare engine under the same rule.
  - Scope: `mtg-server` (`stops.rs`, the skip rule in `running.rs`, `start_up`), `mtg-testkit` (`PlayOptions`), `server.md`.
- **P3 Idle windows as hidden clocks**: inside P2's window set, a window the seat cannot use waits `Pace.dwell` (250 to 700 ms jittered, `MTG_DWELL_MS`, 0 = off), then passes as its own step and batch through `fall_back`.
  - Yields and Full Control are exempt. Only the wall clock changes, so the log and the hash do not.
  - Windows are sequential, so at three or more seats the waits add up per idle seat.
  - Tests on paused time (`tests/idle_dwell.rs`). Scope: `mtg-server` (`pace.rs`, `running.rs`), `server.md`.
- **P4 Full Control**: `FullControl { mode: Off | NextWindow | Held }` and `Welcome.full_control` ([protocol.md](protocol.md)). The seat is asked at every step and never passed or yielded for; the response clock keeps its `can_act` condition, so nobody is timed out on a window they cannot use.
- **P5 Web session for stops and Full Control**: `session.setStop` and `setFullControl`, store fields from `Welcome`, `net/parse.ts`, Ctrl and Ctrl+Shift; tests on the fake socket. Scope: `web/src/play`, `web/src/net`.
- **P6 Stop ladder**: `web/src/features/table/stops/`, two rows of step icons (yours, theirs) by the seat panels. A click sets a one-turn stop and a shift-click a lasting one; set stops are lit and the current step is marked, with screen-reader text and the Full Control state; `/dev/table?stops=`; `web.md`.
- **P7 Keys like Arena**: Space one pass, Enter the `EndOfTurn` yield; Shift+Enter and Shift-click the strict pass only if the player asks (a `YieldUntil` strength that an opponent's play does not end). Scope: `web/src/features/table/prompt` (`usePrimaryKey`, `PassControls`), `crates/mtg-server/src/yields.rs`.
- **P8 Leak test**: two seeded games equal but for one hidden card (an instant against a land); the other seat's messages (types, batch boundaries, arrival times on paused time) must match with the defaults. Scope: `crates/mtg-server/tests/pace_leak.rs`.
- **P9 Timer bank**: extensions earned after three turns without using a clock and spent at expiry, a grace before the countdown shows, an optional clock for the active seat. Scope: `pace.rs`, `Welcome.extensions`, `web/src/features/table/prompt/ResponseClock`.
- **P10 With triggers**: smart stops (a step where an object can answer becomes a window), per-ability yield (`AutoYield`), auto-order triggers once or always, damage auto-assign. Scope: `mtg-engine` (`PriorityOptions`), `mtg-protocol`, `web/src/features/table/prompt`.
