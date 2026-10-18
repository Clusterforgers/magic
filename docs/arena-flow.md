# Arena flow: priority, stops, auto-pass, yields and what pace gives away

Research for the M2 item "Game flow (c)" (2026-10-18), docs only. The player asked us to copy the feel of MTG Arena instead of inventing a pass-priority flow, and fears that our auto-pass lets an opponent tell from the pace that a seat has nothing to do. Tags: [official] a Wizards page or developer; [community] guides, forums, bots and the client's data-mined protocol; [unverified] my inference or memory, or no source found. `Sn` is the URL under Sources. Wizards' help and patch-note pages answer 403 to automated fetches and its feedback forum is private, so most Arena detail is [community]. I looked at two-player play only; what Arena does for 3 or more seats is unknown to me.

## What this changes
- Arena has our rule and our tell. It "only stops if you have an available play" [community S11], and one guide calls the information its stops give away the largest disadvantage of playing there [community S8]. I found no Wizards statement about the tell; its guidance is to set a stop or use Full Control [official S1, S7]. Copying Arena therefore means adding stops and Full Control. Hiding the pace better than Arena does is our own addition, ranked below.
- Ours is at least as sharp: an idle pass costs no round trip and lands in the same batch as the action that caused it (`server.md`), and we ask a seat that can act in its own upkeep and end step, where Arena's default gives no priority [official S1].
- The principle that fixes most of it: decide when a seat is asked from the table (step, stack, stops), never from its hand. The in-progress `yields.rs` already does; auto-pass does not.
- Arena keeps yields on the server in two strengths (Enter ends when the opponent acts, Shift+Enter does not), Pass Turn still stops at a stop, and the protocol also has per-ability yields [community S8, S9, S25]. Our `Yield` is the weak strength; Space and Enter must not be the same key.
- Do not copy: relaying the hovered hand card to the opponent (Arena does, players call it a leak [community S22]) or Arena's timer machinery (eight protocol versions [community S25]) beyond a bank and a delayed start.

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
- S25 [community, data-mined from the client's GRE protocol DLL] `messages.proto`: https://github.com/riQQ/MtgaProto. The enums are facts; what they do in the UI is my reading [unverified].
- S26 [community, data-mined] the settings message in a Go mirror: https://pkg.go.dev/github.com/di-wu/mtga/thread/connect_resp
- S27 [community, weak] a bot's README that names buttons: https://github.com/Barrylim366/mtga-farm-bot
- S28 [community, weak, search snippet only] fan wiki tutorial walkthrough ("All Attack"): https://magicarena.fandom.com/wiki/New_Player_Experience

## Arena behaviour

### 1. The primary button
One big orange button at the bottom right passes priority; its label follows the state. No source lists every label, so this is what is attested.

| Label | Shows when | What it does |
|---|---|---|
| Next | your priority, empty stack | passes priority; the game goes to the next phase or step [community S8, S12, S15] |
| Resolve | an object is on the stack and you hold priority; the other player is asked to click it too for each of your spells, sometimes with nothing to answer | passes so the top object resolves; if you do not click it your own clock runs down [community S19, S21, S23] |
| Resolve All | several objects on the stack | resolves them all; pressing it again stops and lets you respond; objects added later ask again [official S2; community S20] |
| Pass Turn (orange arrow, a toggle) | always | passes the rest of the turn but stops at your stops; Enter or the arrow turns it off; over a stack it acts as Resolve All; a 2019 fix stopped it applying to the next turn "if the server's state was ahead of the player's visual state", so the server holds it [official S2; community S8, S9] |
| Auto-Pay (was Pay) | paying a cost | Arena chooses the lands [official S3] |
| Submit, Submit 0, Done | a selection prompt | confirms it; "Submit 0" skips an optional target, "Done" closes scry or surveil [community S27] |
| No Blocks, All Attack | declare blockers, declare attackers | the bottom-right button is both pass and "No Blocks"; All Attack sends every creature [community S27, S28] |
| End Turn, No Attack, Cancel, Opponent's Turn, Keep, Mulligan | ? | no source says when they show; the 2018 beta had separate pass and end-turn buttons [unverified; community S14] |

Hotkeys [community S8, S11, S15, S17]:
- Space passes once and also confirms an OK. Enter is "Pass Until Response": through the turn until the opponent acts or you can answer something, no priority back for triggers, and pressed before combat it can skip your own combat.
- Shift+Enter (left Shift) or Shift-click on the button is "Pass Turn", MTGO's F6: whatever the opponent does; priority comes back only to block.
- Ctrl is Full Control, Ctrl+Shift holds it and the same keys release it. Z undoes taps and other actions that did not use the stack. Q Q taps all mana.
- The guides disagree on L (show phases, or stops for a turn) and on whether Ctrl lasts a phase or a turn.

### 2. Stops
- The ladder: upkeep and draw, main 1, beginning of combat, declare attackers (sword), declare blockers (shield), damage (explosion), main 2, end step ("after this turn"), as icons by each portrait plus icons at the bottom right for the current turn and combat [community S8, S9]. The client protocol lists eleven `StopType`s: upkeep, draw, precombat main, beginning of combat, declare attackers, declare blockers, combat damage, end of combat, postcombat main, end step, first-strike damage; no untap, no cleanup [community S25].
- Orange is your turn, blue the opponent's [community S8]; a protocol stop has a scope (AnyPlayer, Opponents, Team) and is Set or Clear [community S25]. A click toggles it. Set it well ahead of time, since the server can be ahead of the client's animations [community S9]. Shift-click on a stop: no source [unverified].
- A stop is Full Control for that phase; Pass Turn still stops at it, and moving past it ends Full Control [community S9, S24]. "Stops only last until you reach that phase" [community S12]. The protocol keeps `stops` and `transientStops` [community S25]; I read the second as one-turn stops, so whether any click makes a lasting stop is [unverified].
- Wizards' developer, asked for a "pass until" feature: "You can get exactly the functionality you mention by setting a stop on the phase where you want to act ... That will pass to anything the opponent does until you get to the phase you put a stop in" [official S7].
- Defaults: no priority in your own upkeep or end step, and usually none in response to your own triggers. "Smart priority" is a list of card-specific exceptions that set the stop for you (Urza's Saga, the evoke elementals, Goblin Rabblemaster, Sheoldred the Apocalypse, a creature flashed in at your end step ...); for the rest, "set a stop or go into full control" [official S1]. `smartStopsSetting` turns it on or off [community S26].
- Patches keep adding cases: an automatic upkeep stop when an opponent's draw-trigger card needs an answer (2022.20.0), beginning-of-combat and end-step trigger cards answerable in the main phase (2025.52.00), more per-card stops (2026.63) [official S4].
- The default stops of the opponent's turn, and whether being attacked while you hold an instant adds one: no source [unverified]. MTGO ships the common stops on for both sides and always gives you declare attackers and blockers when you control a creature [official S5].

### 3. Auto-pass and Full Control
- Arena "only stops if you have an available play" [community S11], "will automatically pass priority through your opponent's turn if you have nothing to play" [community S17] and passes the turn after a land drop with nothing else to do [community S12]. That is our `can_act`. Each action in the protocol carries `shouldStop`; my reading is the server telling the client which options deserve a stop [community S25, unverified reading].
- It never skips a decision that needs an answer: blocks (Pass Turn hands priority back to block [community S8]), targets ("New trigger that requires you to select a target will always stop" [community S19]); nor a stop you set (Pass Turn still stops there [community S9]), nor Full Control.
- Full Control: Ctrl holds priority for the phase and ends once you pass (the next phase then gets a stop), Ctrl+Shift holds it until toggled, on mobile press and hold your avatar [community S8, S10, S24]. It is how you answer your own abilities and triggers, spend pool mana by hand, and override Auto Tap, Auto Order Triggered Abilities, Auto Choose Replacement Effect and Auto Assign Combat Damage [community S10, S13, S24]. `FullControl` is also an `AutoPassOption` [community S25].
- The price of windows: Arena sometimes hands priority to a player who cannot answer, who then loses time, or the game, by not clicking [community S21].

### 4. Yields
- Resolve All and Pass Turn (section 1) [official S2]. Enter and Shift+Enter are two strengths: Enter ends when the opponent acts or you can answer something, Shift+Enter only at a stop or a decision that needs you; pressed on the opponent's turn it skips what is left of it and you are asked only to block [community S8, S11, S15].
- Resolve All ends when something new goes on the stack, so a long trigger chain asks again and again, which players dislike [official S2; community S19, S20].
- They live on the server. `autoPassOption` is one of Turn, UnlessAction, EndStep, UnlessOpponentAction, ResolveMyStackEffects, FullControl, ResolveAll, with a default and a separate `stackAutoPassOption` [community S25, S26]. Which key sets which value is [unverified].
- `AutoYield` is per ability or per card title and scoped to anyone, opponents or your team; `AutoAnswer` keeps ChooseOnce or ChooseAlways answers to optional prompts; trigger order keeps OrderArbitraryOnce, OrderArbitraryAlways or PromptAlways [community S25].
- MTGO is the checklist: pass until you can respond, yield through the turn, yield to a stack object this turn or always, "no possible play: yield all stops" (our auto-pass); you still confirm attackers and blockers [official S5].

### 5. Information hiding
What an opponent can infer in Arena:
- Pace. If a player plays a land and passes and then still gets priority on your turn, you can infer an instant, and a pause after a cast hints at a counterspell, since "there will be a noticeable stutter compared to the smoothness that Arena passes if the opponent has nothing" [community S8]. Players say it too: "if the game auto passed more it would give away you have nothing to play"; "having a stop constantly is a decent way to leave your opponent wondering" [community S19].
- Hover. A hand card you mouse over lights up on the opponent's screen; players call it a flaw and a bluff tool, and I found no setting to turn it off [community S22]. The protocol has `OnHover` and `OnSelect` messages [community S25].
- The timer rope is on the avatar, and players report each other's roping [community S14, S18].
- The state names who holds priority and who is deciding, and each player record has a `pendingMessageType`; whether the other seat sees it is [unverified] [community S25]. A lead: actions have a `Visibility` of Public, Private, Hidden or Deceptive and the protocol has a "DeceptiveActions" version; purpose unknown [unverified].

How Arena mitigates, and paper:
- Nothing by the game. The player's tools are stops on every phase (a better bluff than constant Full Control, which is "very obvious" and can signal an empty hand), Full Control, tapping lands without casting, and Space quickly after plays that need no answer [community S8, S12]. I found no fixed delay, minimum think time or uniform animation; that is only absence of evidence [unverified].
- Paper: shortcuts skip priority passes "to the mutual understanding of all players"; after "attacks?" the opponent is assumed to act in the beginning of combat, a player may interrupt a shortcut by saying so, and "a player may not request priority and take no action with it" [official S6]. Hesitation is a physical tell that the rules do not equalise [unverified]. MTGO ships default stops for both sides [official S5].

### 6. Timers
- 2018 beta: a fuse after "just over a minute", then 45 s; three dots, filled by playing quickly, earn one timeout (hourglass) [community S14]. Later guides: a rope burns 30 s when one action takes too long; a timeout is earned after three turns without the rope and adds 30 s when the rope burns; best of three has a 30-minute match clock; two automatic passes in a row lose the game [community S16]. Figures differ by era, so treat today's as [unverified].
- Official 2019: the non-active player's timer "no longer resets between phases" (anti-stall); "a delay before your timer starts whenever you are prompted for a response"; inactivity timer 120 s, and a player who has not responded by then forfeits when the current timer expires [official S2].
- Protocol: timer kinds Decision, Inactivity, ActivePlayer, NonActivePlayer, Prologue, Epilogue, Delay, MatchClock; behaviours include Timeout and AutoRespond; `TimeoutConfig` counts timeouts and pips; packages per format (best of one with a match clock, best of three, cubes); `TakeTimeoutReq` looks like a player spending one [unverified reading]; the timer has eight protocol versions, `Timers` and `TimersV2` to `TimersV8` [community S25].
- At expiry the server answers for the player (a pass, the next phase) [community S14, S16, S25]. Complaints: long stacks run the clock of the player who cannot answer [community S21]; timers too long [community S23].

### 7. Inputs for M3
- Combat: the board is the picker; the server validates each click (`canSubmitAttackers`, legal recipients per attacker including planeswalkers, `mustAttack`, warnings such as CannotAttackAlone and MustBeBlocked), then a submit; All Attack; settings Auto Assign Combat Damage and Gameplay Warnings; a damage-order message exists behind a flag [community S25, S13]. Block arrows: no source [unverified]; ours already draws the lines. Our `Preview` already does the validate-as-you-click part.
- Triggers: Auto Order Triggered Abilities is a setting (guides say to turn it off to order them yourself) and Full Control overrides it [community S8, S13]; ordering answers can persist once or always [community S25].
- Hold priority: none per spell, Full Control or stops instead. You keep priority when an effect you control is put on the stack by an opponent's action (the example is a creature with a "becomes the target" trigger) [official S3]. MTGO: hold Ctrl while casting [official S5].
- Payment: Auto Tap is a setting, dragging or hovering a spell previews the lands it will tap, and Pay became Auto-Pay [official S3; community S9, S13, S17]; manual mana needs Full Control [community S10].
- Targeting: a request and a "Submit N" confirmation [community S25, S27]. Undo: Z before anything resolves; `UndoReq` and `CancelActionReq` exist [community S8, S25].

## Information leaks
What ours leaks today:
- Pace. With auto-pass on (the default) a seat whose `can_act` is false is passed inside the actor's step: no wait, and its pass is in the batch of the answer that caused it. A seat that can act makes the table wait for a human, up to 10 s as a non-active seat, unbounded as the active one. `can_act` means a land to play, a spell affordable at this timing or a non-mana ability; on the opponent's turn that is an instant or flash card with mana up, so the pace is the secret itself.
- The log is uniform: `PriorityGranted` and `PriorityPassed` are public events logged for every window, skipped or not, so the text does not leak; arrival time and batch shape do.
- The 10 s clock exists only for a non-active seat that can act. Nobody else sees it, but a pass landing exactly at the deadline reads as "had something, dithered".
- We ask the active seat in its own upkeep and end step whenever it can act; Arena's default gives no window there [official S1]. More windows where a pause means something.
- The only knob is the per-seat auto-pass switch: off gives up to ten prompts a turn per seat, uniform but slow, and flipping it mid-game changes the pace the table sees. There are no stops, so a player can neither ask for a window nor bluff one.
- Good already: no hover or selection relay; clocks, previews and `Welcome.auto_pass` reach only their seat.

What Arena leaks: pace (stops), the hovered hand card, the rope, possibly the kind of answer owed (section 5).

Mitigations, ranked by value for effort:

| # | Mitigation | Source | Cost | Hides |
|---|---|---|---|---|
| 1 | Stops and Full Control: the player chooses when to be asked and can bluff a window | Arena [official S1, S7] | medium (P1, P2, P4-P6) | whatever that player decides to |
| 2 | Table-driven windows with Arena's default table (none in own upkeep or end step unless set) | Arena [official S1] | small, inside P2 | pauses in steps where only a holder of an instant stops |
| 3 | A window the seat cannot use is held for a human-like 0.3 to 0.9 s, then passed as its own step and batch | ours, not in Arena [unverified] | small (P3), about 1 s per window a seat could use | the instant pass and the batch burst |
| 4 | Leak test over what the other seats receive | ours | small (P8) | keeps 2 and 3 true |
| 5 | Strict table: auto-pass off for every seat | ours (MTGO's default stops are the nearest [official S5]) | tiny, a creation option | everything, paid in clicks |
| 6 | Never add a hover or selection relay, a clock or "thinking" cue for opponents, or the kind of decision owed to the views | Arena leaks these [community S22, S25] | none | avoids new tells |

Skipped: pacing step changes in the client. The fx layer must never wait for a cadence, and P3 does it at the source.

## Mapping
| Arena feature | Ours | Proposed change |
|---|---|---|
| Main button relabels (Next, Resolve) | in progress: `Next: <step>`, `Resolve <spell>`, `End turn`, `Continue` (`passWords.ts`) | keep; the words say more than Arena's |
| Space one pass, Enter pass until response, Shift+Enter pass turn | in progress, but Space and Enter both press the primary button | P7 |
| Resolve All, press again to stop, new object asks again | in progress: `YieldUntil::Resolve`, `CancelYield`, ends on an opponent's new object | keep |
| Pass Turn held by the server for its turn | in progress: yields record the turn they were asked in | keep |
| A yield ignores what the seat could do | in progress, by design | keep; extend to auto-pass (P2, P3) |
| Stops: two sides, eleven steps, interrupt Pass Turn | missing | P1, P2, P5, P6 |
| Full Control: Ctrl once, Ctrl+Shift held | missing (the switch off is a crude held mode) | P4, P5 |
| No window unless a play | done: `can_act`, per-seat switch, default on | keep; P2 makes the windows table-driven |
| No window in own upkeep or end step by default | missing: we ask when `can_act` | P2 default table |
| Smart priority, card-specific stops | missing (no triggers yet) | P10 |
| Blocks asked whenever possible, `NothingToDo` otherwise | done | none |
| Timers: rope, bank, delayed start, inactivity | partial: 10 s only for a non-active seat that can act, 45 s blocks, none for the active seat | P9 |
| Expiry answers for the player | done: `fall_back` | none |
| Combat choice validated while clicking | done: `Preview` | none |
| All Attack, No Attack, No Blocks | done in the prompts ("Attack with all", "No attack", "No blocks") | none |
| Auto-tap, Auto-Pay, land preview | partial: "Pay automatically", "Pay the rest for me", no hover preview | optional, M3 |
| Undo (Z) | partial: payment Undo and Clear, Cancel casting | later |
| Auto-order triggers, per-ability yield | missing (needs triggers) | P10 |
| Hand hover relayed to opponents | absent | keep absent |
| Hold priority | none in Arena either | Full Control and stops cover it |

In-progress item (Game flow c), keep: yields independent of `can_act`, the recorded turn, ending on an opponent's new object, the labels, the indicator. Change: Enter is the EndOfTurn yield, not the primary button (P7); a set stop must interrupt a yield (P2); cancelling a Resolve yield should be one click on the same place (Arena: press the button again).

## Proposed roadmap items (about 15 minutes each, in order of value)
- P1 Stops on the wire: `Stop {side: Own | Opponent, step}`, `ClientMessage::SetStop {side, step, on, keep}` (`keep` survives the turn; the default is one turn), `Welcome.stops`, bindings, round-trip and drift tests. `crates/mtg-protocol`, `web/src/protocol/generated`, `docs/protocol.md`.
- P2 Stops in the actor with Arena's default table: a per-seat set; a seat is asked only at the default windows (own main steps, declare attackers and blockers, a new opponent object on the stack; the opponent's end step waits for question 2) and at its stops; one-turn stops clear when the step ends; a stop interrupts a yield, and the decision is sent even when `can_act` is false. Table test plus bot games equal to the bare engine under the same rule. `crates/mtg-server` (new `stops.rs`, the skip rule in `running.rs`, `start_up`), `crates/mtg-testkit` (`PlayOptions`), `docs/server.md`.
- P3 Idle windows as hidden clocks: inside the window set of P2 a window the seat cannot use waits `Pace.dwell` (300 to 900 ms jittered, `MTG_DWELL_MS`, 0 = off), then passes as its own step and batch through `fall_back`; yields and Full Control are exempt; wall clock only, so the log and the hash do not change. Tests on paused time (`tests/idle_dwell.rs`). `crates/mtg-server` (`pace.rs`, `running.rs`), `docs/server.md`.
- P4 Full Control: `ClientMessage::FullControl {mode: Off | NextWindow | Held}`, `Welcome.full_control`; the seat is asked at every step and never passed or yielded for; the response clock keeps its `can_act` condition, so nobody is timed out on a window they cannot use. `crates/mtg-protocol`, `crates/mtg-server`, tests.
- P5 Web session for stops and Full Control: `session.setStop` and `setFullControl`, store fields from `Welcome`, `net/parse.ts`, Ctrl and Ctrl+Shift, tests on the fake socket. `web/src/play`, `web/src/net`.
- P6 Stop ladder: `web/src/features/table/stops/`, two rows of step icons (yours, theirs) by the seat panels, click to toggle, lit when set, the current step marked, screen-reader text, Full Control state; `/dev/table?stops=`; `docs/web.md`.
- P7 Keys like Arena, after Game flow (c) merges: Space one pass, Enter the EndOfTurn yield, Shift+Enter and Shift-click the strict pass only if the player says yes (a `YieldUntil` strength that an opponent's play does not end). `web/src/features/table/prompt` (`usePrimaryKey`, `PassControls`), `crates/mtg-server/src/yields.rs`.
- P8 Leak test: two seeded games equal but for one hidden card (an instant against a land); the other seat's messages (types, batch boundaries, arrival times on paused time) must match with the defaults. `crates/mtg-server/tests/pace_leak.rs`.
- P9 Timer bank: extensions earned after three turns without using a clock and spent at expiry, a grace before the countdown shows, an optional clock for the active seat. `crates/mtg-server/src/pace.rs`, `Welcome.extensions`, `web/src/features/table/prompt/ResponseClock`.
- P10 With M3 triggers: smart stops (a step where an object can answer becomes a window), per-ability yield (`AutoYield`), auto-order triggers with once or always, damage auto-assign. `crates/mtg-engine` (`PriorityOptions`), `crates/mtg-protocol`, `web/src/features/table/prompt`.

## Open questions for the player
1. Screenshots of the main button in each state (own main phase with an empty stack, a spell on the stack, the opponent's turn with and without an answer, attackers before and after picking, blockers before and after, last window of the turn) and of the ladder on both sides, to settle every label marked [unverified].
2. Do your Arena stops last one turn or stay (is there a shift-click)? Which stops are lit by default on the opponent's turn, and do you get priority at the opponent's end step with an instant in hand?
3. Arena's behaviour only (accept the tell, bluff with stops), or also P3 (every idle pass takes a human-like 0.3 to 0.9 s at windows a seat could use)? On for everyone, or an option when the game is created?
4. Should Shift+Enter exist (pass the turn whatever the opponent casts, as in Arena), given that a slip skips a response?
5. Does an Arena opponent see anything while you hold priority (a "thinking" cue, the rope), and what does the button say on their turn? Do we keep "Waiting for Cy" in the strip, which names who holds a window and helps at tables of 3 or more?
6. Hand hover: keep it private (recommended) or show opponents what you hover, as Arena does?
7. Clocks in friendly games: keep 10 s and 45 s, add a bank, or no clock unless the table asks? A clock for the active player?
8. More than two seats: I only looked at two-player Arena. Windows are sequential, so P3 adds up per idle seat; cap it, or run idle seats together?
