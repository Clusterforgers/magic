# Tutorial

The Learn course: seven short lessons at a scripted table that teach a new player the game and the table, with a coach that lights exactly what to press and holds back everything else.

## Principles

- One lesson teaches one idea and takes two to three minutes; each starts its own scripted game, so no lesson depends on how an earlier one went.
- A step asks for one thing. The coach lights the one element to press, and pressing it is the only input that passes.
- What is lit is named by an anchor, never by a CSS path. A layout change moves the anchor with its element or fails a test.
- Steps follow the game state, never a timer or a count. A step that only explains is shown while the game waits on the learner, so nothing moves under a reader.
- One wording serves mouse and touch: "choose" for a card, "press" for a button. A button's name in a text is read from the button itself.
- The coach follows `docs/ui-design.md`: one focus at a time (staging), an answer within 400 ms to every press (Doherty), one obvious next action (Hick), lessons of at most twelve steps (Miller), 44 px targets, motion still under reduced motion.

## Course

The Learn page shows the course as a numbered path with the next lesson as its one primary button ("Start", then "Continue: Combat"), each lesson's minutes and a Done mark; the page tours and the manual-panel lesson follow under "The app". A finished lesson offers "Next lesson" and "Back to lessons".

| # | Id | Lesson | Teaches |
|---|---|---|---|
| 1 | `seat` | Your seat | opening hand and Keep, your pod and life, the opponent, the turn badge, the land drop, the battlefield, the prompt band, passing |
| 2 | `mana` | Mana and spells | reading a cost, casting, paying by tapping, Pay for me, power and toughness, summoning sickness, untapping, a mana ability |
| 3 | `combat` | Combat | going to combat, attackers, unblocked damage, blocking, creature damage, the graveyard, a tapped creature cannot block, winning |
| 4 | `stack` | The stack | the stack, priority, passing to resolve, a trigger, holding mana open, an instant in response, a target, until end of turn |
| 5 | `turn` | Turns and stops | the steps of a turn, why the game skips, a stop, End turn, Full Control |
| 6 | `commander` | Commander | 40 life, the command zone, casting the commander, returning it to the command zone, commander tax, commander damage |
| 7 | `help` | Finding help | the card preview, keywords, the log, the rules lexicon, the Commands tab, the table menu |

The lessons replace the guided game and the table tour of the Learn page. The manual-panel lesson runs on the same coach and anchors.

## Scripted table

`POST /api/tutorial` takes `{lesson}` and answers 201 `{game, seat, token}`: a Commander game of two seats that is never recorded or listed (`docs/server.md`, Tutorial table). A lesson is one entry of `tutorial::LESSONS`:

| Field | Meaning |
|---|---|
| `id` | the lesson's id, shared with the web |
| `setup` | a `GameSetup` with fixed hands, libraries (top card first), permanents and life; every card is a Verified corpus card with its printing |
| `stops` | the stops the learner's seat holds, so each scripted decision arrives and no other |
| `bot` | the bot's script: `(turn, step, act)` with `Keep`, `Land`, `Cast` (with its target), `Attack`, `Block`; anything unscripted is a pass, no attack, no block, the engine's payment |
| `script` | the learner's script: `(turn, step, decision kind, act)` for every decision the learner is sent |

- Lesson 1 starts at the mulligan with `GameSetup::stacked`: the opening deals each library in its given order and seat 0 plays first. Every other lesson starts with `Start::At` in the state it needs.
- The bot answers after a beat (`MTG_TUTORIAL_BEAT_MS`, 700 by default, 0 in tests), so the learner sees each of its plays.
- The table starts with Full Control off, automatic payment off and the lesson's stops; the lesson's session sends no saved preferences.
- The learner's seat is named after the account, else "Guest".
- `tests/it/tutorial.rs` plays every lesson by its script: each decision sent to the learner is the scripted one, each scripted answer is legal when asked, and the game ends where the lesson says. The test writes `web/src/features/learn/traces/<id>.json` (each decision's turn, step, kind, act, cards and facts) and fails while a committed trace is stale.

### States

| Lesson | Start | Learner | Stops | Bot |
|---|---|---|---|---|
| `seat` | mulligan | opening hand Forest ×3, Plains, Grizzly Bears, Llanowar Elves, Giant Growth; library of Forests | none | keeps; plays a Swamp on its turn |
| `mana` | turn 3, first main | Forest ×2 in play; hand Forest, Grizzly Bears, Llanowar Elves | own second main | plays a Swamp, casts Dregscape Zombie |
| `combat` | turn 5, first main | four lands, Grizzly Bears, Serra Angel (summoning sick), no hand; 40 life | own first main, declare attackers, second main | 8 life, Dregscape Zombie ×2; never blocks, attacks with both on its turn |
| `stack` | turn 5, second main | Forest ×3, Plains, Grizzly Bears; hand Hero in Training, Giant Growth; library Forest first | none | Dregscape Zombie; attacks with it |
| `turn` | turn 5, first main | four lands, Grizzly Bears (summoning sick, so no attack is asked); hand Giant Growth | none; the learner sets the opponent's end step | plays a Swamp |
| `commander` | turn 5, first main | Plains ×3, Forest; Giada, Font of Hope in the command zone; library of Serra Angels the lands cannot cast | own second main | Swamp ×3; hand Doom Blade, cast on Giada on its turn |
| `help` | turn 3, end step, so the log holds turns 3 to 5 | four lands, Serra Angel, Grizzly Bears; draws a Forest on turn 5 | none | plays a Swamp and casts Dregscape Zombie on turn 4 |

## Steps

A lesson is a list of steps (`web/src/features/learn/lessons/<id>.ts`). A step has:

| Field | Meaning |
|---|---|
| `kind` | `read` (explains, left with Next), `do` (waits for the learner's presses), `watch` (the bot acts; moves on by itself) |
| `title`, `text` | at most 28 and 160 characters; `{@anchor}` is the words on that anchor's element now, `{look}` is "Hover over" or "Press and hold", `[[term]]` links the lexicon |
| `lit` | a `read` or `watch` step's anchors, at most two |
| `presses` | a `do` step's anchors in the order they are pressed; the coach lights one at a time and moves on at a press inside it; a later press falls back while its element is absent or disabled |
| `allow` | the input that passes: `keep`, `land(name)`, `cast(name)`, `pass`, `pay('tap' | 'auto')`, `endTurn`, `attack(names)`, `block(pairs)`, `target(name)`, `choose(answer)`, `stop(side, step)`, `manual` (the Commands panel and card menu, for the manual-panel lesson) or `look` (no game input) |
| `done` | a test over the lesson facts (turn, step, pending decision, zones, combat, life, result) and the page facts (an anchor shown or enabled) |
| `hint` | what the coach says when another input is tried |

The cursor is the first step that is not done, with `read` steps counted as done once left; it is kept in `sessionStorage` with the lesson, so a reload returns to the same step. A `read` or `do` step starts only while a decision of the learner is pending or the game is over; until then the coach shows the `watch` step before it, or the line "The game moves on" under the last step, so nothing is asked while the bot plays.

### Lesson 1: Your seat

| Step | Kind | Lit or pressed | Passes | Text | Done |
|---|---|---|---|---|---|
| `welcome` | read | none | nothing | Welcome. This is a practice table: nothing here counts, and each step shows where to press. | Next |
| `hand` | read | `opening.hand` | nothing | Your opening hand: seven cards. Lands make mana; the others are spells. Two to four lands is a hand to keep. | Next |
| `keep` | do | `opening.keep` | `keep` | Four lands and three spells. Press {@opening.keep}. | the game is in turn 1 |
| `you` | read | `own.pod` | nothing | This is you: your name, your life and how many cards you hold. In Commander you start at 40 life. | Next |
| `them` | read | `opp.pod` | nothing | Your opponent sits across the table. Bring their life to 0 and you win. | Next |
| `turn` | read | `turn.badge` | nothing | The top bar says whose turn it is and which step. It is your turn, in your main phase. | Next |
| `land` | do | `hand:Forest` | `land(Forest)` | Choose a Forest in your hand to play it. You may play one land each turn. | a Forest of yours is in play |
| `board` | read | `own.board` | nothing | The Forest is on your battlefield. Cards here stay in play until something removes them. | Next |
| `band` | read | `band` | nothing | This bar says what the game asks of you. Read here first when unsure. | Next |
| `pass` | do | `pass.primary` | `pass` | You have nothing more to do. Press {@pass.primary}; the game skips the steps where nothing can happen. | turn 2 |
| `bot` | watch | `opp.board` | nothing | The bot takes its turn and plays a land. | your turn 3, a decision pending |
| `end` | read | `hand` | nothing | A new turn: you drew a card. That is the table. | Finish |

### Lesson 2: Mana and spells

| Step | Kind | Lit or pressed | Passes | Text | Done |
|---|---|---|---|---|---|
| `land` | do | `hand:Forest` | `land(Forest)` | Start with your land for the turn: choose the Forest. | three Forests in play |
| `cost` | read | `hand:Grizzly Bears` | nothing | A card's cost is at its top. {1}{G} is one mana of any colour and one green. Each Forest makes {G}. | Next |
| `cast` | do | `hand:Grizzly Bears` | `cast(Grizzly Bears)` | Choose Grizzly Bears to cast it. | a payment is asked |
| `tap` | do | `pay.tap`, `pay.tap`, `pay.confirm` | `pay('tap')` | Pay by tapping lands: press {@pay.tap} twice, then {@pay.confirm}. | Grizzly Bears is in play |
| `creature` | read | `board:Grizzly Bears` | nothing | Grizzly Bears is a 2/2: 2 power, 2 toughness. It is marked Sick: a creature cannot attack the turn it arrives. | Next |
| `cast-2` | do | `hand:Llanowar Elves` | `cast(Llanowar Elves)` | One Forest is still untapped. Choose Llanowar Elves. | a payment is asked |
| `auto` | do | `pay.auto`, `pay.confirm` | `pay('auto')` | This time let the game choose: press {@pay.auto}, then {@pay.confirm}. | Llanowar Elves is in play |
| `pass` | do | `pass.primary` | `pass` | All your lands are tapped. Press {@pass.primary}. | turn 4 |
| `bot` | watch | `opp.board` | nothing | The bot plays a land and casts a creature of its own. | your turn 5, a decision pending |
| `untap` | read | `own.board` | nothing | A new turn: your lands untap and you draw a card. | Next |
| `ability` | read | `board:Llanowar Elves` | nothing | Llanowar Elves has an ability, {T}: Add {G}. It taps for mana like a land, from now on. | Finish |

### Lesson 3: Combat

| Step | Kind | Lit or pressed | Passes | Text | Done |
|---|---|---|---|---|---|
| `intro` | read | `turn.badge` | nothing | Combat is the middle of your turn: you choose attackers, the opponent chooses blockers, then damage is dealt. | Next |
| `go` | do | `pass.primary` | `pass` | Press {@pass.primary} to go to combat. | attackers are asked |
| `attack` | do | `board:Grizzly Bears`, `attack.confirm` | `attack(Grizzly Bears)` | Choose Grizzly Bears, then press {@attack.confirm}. Serra Angel arrived this turn and stays home. | Grizzly Bears attacks |
| `window` | do | `pass.primary` | `pass` | An attacker taps. Both players may act before blocks; you have nothing, so press {@pass.primary}. | second main phase |
| `hit` | read | `opp.life` | nothing | The bot did not block, so Grizzly Bears dealt its 2 damage to the bot. | Next |
| `pass` | do | `pass.primary` | `pass` | Press {@pass.primary} to end your turn. | turn 6 |
| `block` | do | `board:Serra Angel`, `opp:Dregscape Zombie`, `block.confirm` | `block(Serra Angel, any Dregscape Zombie)` | Two Zombies attack you. A sick creature can still block: choose Serra Angel, then a Zombie, then press {@block.confirm}. | Serra Angel blocks |
| `damage` | read | `opp.graveyard`, `own.life` | nothing | Serra Angel dealt 4 damage to a 2/1 Zombie, so it died and went to the graveyard. The other Zombie hit you for 2. | Next |
| `go-2` | do | `pass.primary` | `pass` | Your turn again. The bot's Zombie is still tapped from attacking, and a tapped creature cannot block. Press {@pass.primary}. | attackers are asked |
| `all` | do | `attack.all`, `attack.confirm` | `attack(Grizzly Bears, Serra Angel)` | Press {@attack.all} to attack with everything, then {@attack.confirm}. | both attack |
| `win` | do | `pass.primary` | `pass` | 6 damage against 6 life. Press {@pass.primary}. | the game is won |

### Lesson 4: The stack

| Step | Kind | Lit or pressed | Passes | Text | Done |
|---|---|---|---|---|---|
| `cast` | do | `hand:Hero in Training` | `cast(Hero in Training)` | Choose Hero in Training to cast it. | a payment is asked |
| `pay` | do | `pay.auto`, `pay.confirm` | `pay('auto')` | Press {@pay.auto}, then {@pay.confirm}. One Forest stays untapped. | the spell is on the stack |
| `stack` | read | `stack` | nothing | A spell does not happen at once. It waits on the [[stack]], and each player may respond to it. | Next |
| `resolve` | do | `pass.primary` | `pass` | You have [[priority]]: the right to act. Press {@pass.primary} to pass; when both players pass, the spell resolves. | Hero in Training is in play |
| `trigger` | do | `pass.primary` | `pass` | Hero in Training entered, so its ability triggered. Abilities wait on the stack too. Press {@pass.primary}. | you drew a card |
| `open` | do | `pass.primary` | `pass` | Giant Growth is an instant: it can be cast on any turn. Keep the Forest untapped and press {@pass.primary}. | turn 6 |
| `attacked` | do | `pass.primary` | `pass` | The bot attacks with a Zombie. You could respond now, but block first: press {@pass.primary}. | blockers are asked |
| `block` | do | `board:Grizzly Bears`, `block.confirm` | `block(Grizzly Bears, Dregscape Zombie)` | Choose Grizzly Bears to block the Zombie, then press {@block.confirm}. As it stands both would die. | Grizzly Bears blocks |
| `respond` | do | `hand:Giant Growth` | `cast(Giant Growth)` | Before damage you get priority again. Choose Giant Growth. | a target is asked |
| `target` | do | `board:Grizzly Bears`, `targets.confirm` | `target(Grizzly Bears)` | Giant Growth needs a target. Choose your Grizzly Bears, then press {@targets.confirm}. | a payment is asked |
| `pay-2` | do | `pay.auto`, `pay.confirm` | `pay('auto')` | Press {@pay.auto}, then {@pay.confirm}: the Forest you kept pays for it. | Giant Growth is cast |
| `end` | read | `board:Grizzly Bears`, `opp.graveyard` | nothing | Giant Growth resolved before damage: the Bears were 5/5, survived and killed the Zombie. At end of turn they became 2/2 again. | Finish |

### Lesson 5: Turns and stops

| Step | Kind | Lit or pressed | Passes | Text | Done |
|---|---|---|---|---|---|
| `steps` | read | `turn.ladder` | nothing | Every turn runs through the same steps: untap, draw, main, combat, main, end. The lit one is where the game is. | Next |
| `skips` | read | `band` | nothing | The game passes for you wherever you cannot act, and this bar says what it skipped. | Next |
| `stop` | do | `stops.side.theirs`, `stops.step.End` | `stop(Opponent, End)` | A stop makes the game wait for you at a step. Set one at the opponent's end step. | the stop is held |
| `end-turn` | do | `pass.end-turn` | `endTurn` | {@pass.end-turn} passes through the rest of your turn in one press. | turn 6 |
| `bot` | watch | `opp.board` | nothing | The bot takes its turn. | the bot's end step, a decision pending |
| `stopped` | read | `band` | nothing | The game waits here because of your stop: the last moment to cast an instant before your own turn. | Next |
| `pass` | do | `pass.primary` | `pass` | Nothing to cast this time. Press {@pass.primary}. | turn 7 |
| `full` | read | `fullcontrol` | nothing | Full Control stops at every step and never passes for you. Leave it off until you miss it. | Finish |

### Lesson 6: Commander

| Step | Kind | Lit or pressed | Passes | Text | Done |
|---|---|---|---|---|---|
| `format` | read | `own.life` | nothing | Commander: a deck of 100 different cards, 40 life, and one legendary creature that leads the deck. | Next |
| `zone` | read | `command:Giada, Font of Hope` | nothing | Your commander waits in the command zone, shown beside your hand. You cast it from there like a card in hand. | Next |
| `cast` | do | `command:Giada, Font of Hope` | `cast(Giada, Font of Hope)` | Choose Giada to cast it. | a payment is asked |
| `pay` | do | `pay.auto`, `pay.confirm` | `pay('auto')` | Press {@pay.auto}: it picks the lands to tap. Then press {@pay.confirm}. | Giada is in play |
| `pass` | do | `pass.primary` | `pass` | Press {@pass.primary} to end your turn. | turn 6 |
| `bot` | watch | `stack` | nothing | The bot casts Doom Blade on your commander. | the return is asked |
| `return` | do | `commander.return` | `choose(command zone)` | Giada would die. A commander may go back to the command zone instead: press {@commander.return}. | Giada is in the command zone |
| `tax` | read | `command.tax` | nothing | Each time you cast your commander again it costs {2} more. This is the commander tax. | Next |
| `recast` | do | `command:Giada, Font of Hope` | `cast(Giada, Font of Hope)` | Giada now costs {3}{W}. Choose it again. | a payment is asked |
| `pay-2` | do | `pay.auto`, `pay.confirm` | `pay('auto')` | Press {@pay.auto}: it picks the lands to tap. Then press {@pay.confirm}. | Giada is in play |
| `damage` | read | `opp.pod` | nothing | A player dealt 21 combat damage by one commander loses, whatever their life. Each panel counts it. You are ready to play. | Finish |

### Lesson 7: Finding help

| Step | Kind | Lit or pressed | Passes | Text | Done |
|---|---|---|---|---|---|
| `look` | do | `board:Serra Angel` | `look` | {look} Serra Angel to read it large. | the preview shows |
| `keywords` | read | `preview` | nothing | The preview reads the card large, with its keywords such as Flying and what each one does. | Next |
| `log` | read | `log` | nothing | The log lists everything that happened, newest last. | Next |
| `lexicon` | do | `help.menu`, `help.lexicon` | `look` | The rules lexicon explains every keyword and rules term. Open it from Help. | the lexicon shows |
| `close` | do | `lexicon.close` | `look` | Search by name or by what a rule does. Close it to go on. | the lexicon is gone |
| `commands` | read | `side.tab.commands` | nothing | A few cards are not automatic yet. The Commands tab plays them by hand; "The manual panel" shows how. | Next |
| `menu` | read | `table.menu` | nothing | Layout, theme, card images, the order of your hand and Concede are in the menu. | Finish |

## Anchors

`web/src/features/learn/anchors.ts` is the registry: every anchor's id, what it names, a default label and its opener. An element carries its anchor as `data-coach="<id>"` through `anchor(id)`, typed by the registry; a zone that holds cards carries `data-coach-zone` and a card is found in it by `data-object-id`.

| Anchor | Names | Opener when hidden |
|---|---|---|
| `opening.hand`, `opening.keep`, `opening.mulligan` | the opening hand's cards and its two answers | |
| `own.pod`, `own.life`, `own.board`, `own.graveyard` | the learner's panel, life total, board and graveyard | |
| `own.lands` | the learner's lands | `own.lands-chip` (a phone folds lands) |
| `opp.pod`, `opp.life`, `opp.board`, `opp.graveyard` | the same of the bot | |
| `turn.badge`, `turn.ladder` | the turn badge and the step ladder | `turn.badge` opens the ladder under 1024 px |
| `band`, `band.question`, `stack` | the drawn prompt band, its question, the stack chips | |
| `pass.primary`, `pass.end-turn`, `pass.more` | the primary button, End turn, the more-passing menu | `pass.more` opens End turn where the dock is narrow |
| `hand`, `hand.menu`, `command`, `command.tax` | the hand dock, the hand order in the table menu, the command slot and its tax mark | `table.menu` opens the hand order |
| `pay.tap`, `pay.auto`, `pay.confirm`, `pay.meter` | the first untapped source row's Tap, Pay for me, Confirm, the cost meter | `sheet.grab` opens a folded sheet |
| `attack.all`, `attack.confirm`, `attack.none`, `block.confirm`, `block.none`, `targets.confirm` | the band's combat and target answers | |
| `commander.return` | the answer that returns a commander to the command zone | |
| `stops.side.theirs`, `stops.step.<Step>`, `fullcontrol` | the stops switch, a step's stop, Full Control | `stops.open` where the stops are a popover |
| `side.tab.log`, `side.tab.commands`, `log`, `commands` | side tabs and their panels | `side.open` on a rail or a phone; a panel's tab opens the panel |
| `help.menu`, `help.lexicon`, `lexicon`, `lexicon.close`, `table.menu` | Help, its lexicon entry, the lexicon dialog and its close button, the table menu | `help.menu` opens its entries |
| `preview`, `preview.keywords` | the card preview and its keyword list | |
| card refs `hand:<name>`, `command:<name>`, `board:<name>`, `opp:<name>`, `stack:<name>` | the first card of that name in that zone that the pending decision can use, else the first | |

- An anchor resolves to exactly one drawn element: inside the window, with a size, and on top at its centre. None or several is an error in development and in tests; in production the coach centres its card and lights nothing.
- A hidden anchor with an opener lights the opener instead, and its text line reads "Open {opener's label} first". Steps therefore name the same anchor in every layout; only the registry knows that a phone reaches the log through the side sheet.
- A source test lists every `data-coach` in `web/src` and requires each registry id to occur once and no id outside the registry.

## Gate

A lesson wraps its `Session` in `lessonSession` (`features/learn/lessonSession.ts`), which the game page creates in place of the plain session.

- `answer` passes only what the current step's `allow` accepts for the pending decision. Anything else is not sent; the coach says the step's `hint` or the kind's default ("Not yet: play the Forest first").
- Everything that changes the game outside a decision is held unless the step allows it: manual actions, Concede, Full Control, stops, yields, automatic choices, take-back. Preview, chat and resync pass.
- The store the table reads is the session's store with the pending decision narrowed (`narrow(decision, allow)`): a card the step does not ask for is not legal and carries the hand reason `Lesson` ("Later", "Not in this step of the lesson"), and the primary button is disabled with the same line while `pass` is held. The table needs no other change to look right.
- A selection that never reaches the server (an attacker picked, a source tapped in the payment) is free; its answer is checked when it is sent.
- A `do` step whose anchor does not resolve, or whose decision kind is not the pending one, for 3 s while a decision is pending shows "This lesson lost its place" with Restart lesson. Tests require that it never shows.
- "Leave lesson" in the coach's menu returns to the Learn page; "Play on without the coach" drops the gate and the coach and leaves the game.

The gate is a teaching aid of the client. The server runs an ordinary game and checks every answer as always.

## Coach

`features/learn/Coach.tsx` draws over the table in one layer that takes no pointer except on its own card.

- Light: a scrim of 40 % over the table with a hole at each lit box, a 3 px ring in the coach colour (`--coach`, a hue no table ring uses) that pulses twice and rests, and a pointer beside the ring that bobs toward it. A press step lights one box; a read step at most two.
- Card: the lesson's name and dots for its steps, the step's title, its text, and one foot line: Next for a read step, "Your move" with the action for a do step, "Watch" for a watch step. A menu holds Leave lesson, Restart lesson and Play on without the coach. Back returns to earlier read steps.
- Place (`coachPlace`): beside the lit box on the first side with room, never over a lit box, the prompt band, the primary button, an open decision sheet or the card preview. With no free side it docks in the corner farthest from the lit box.
- Phone: the card is a bar across the window with at most three lines of text, docked under the top bar while the lit box is in the lower half and above the hand dock otherwise. It folds to its title on a press and never covers the band's answers.
- Feedback: a finished step shows a tick for 300 ms before the next text; a held input shakes the card once and shows the reason for 3 s in a polite live region. Under reduced motion both are a colour change.
- A lesson table holds the card preview closed (hover and pin) while the coach lights anything but the preview or a card to look at, so the preview never covers the band or the part to press.
- The root carries `data-coach-step`, `data-coach-kind`, `data-coach-lit` (parts joined by |), `data-coach-press` and `data-coach-moment` (turn, step and pending decision), and each lit element `data-coach-lighting`, which the tests read.

## Tests

- Server (`tests/it/tutorial.rs`): the script test of [Scripted table](#scripted-table) over every lesson, the card test (Verified, printed, nothing by hand) and the unlisted-game test.
- Steps (`lessons.test.ts`, one table over every lesson): walking the lesson's trace, each decision meets a `do` step whose `allow` accepts the scripted act and refuses every other option of that decision; each `read` step is entered while a decision of the learner is pending; every step is reached and the last moment leaves the final step; titles and texts keep their limits; every anchor and `{@…}` token is in the registry.
- Gate (`gate.test.ts`): a table of decision kinds against allowed and refused answers; `narrow` keeps exactly the allowed options; every `Session` method that changes the game is held by default.
- Anchors (`e2e/learn-anchors.spec.ts`): every lesson at 1920 × 1080, 1366 × 768 and 390 × 844 with touch. At each step every lit anchor resolves to one element inside the window that is on top at its centre and corners; the ring's box equals the element's; the coach card overlaps no lit box and none of the kept-clear parts; the card is inside the window; no text is cut.
- Course (`e2e/learn-course.spec.ts`): the same three layouts play every lesson to its end with one generic walker and no per-step code: a read step presses Next, a do step presses the centre of the ring, a watch step waits. Before each do step the walker tries the inputs the step holds back (the primary button, another hand card, Space, End turn) and requires the game unchanged and the reason shown. The lost-place notice fails the run. The walker presses and measures only once the page is still (`settle`: each lit part one drawn element, its ring on it, the lit boxes and the coach card unmoved between two looks), so a loaded machine changes how long a walk takes, never what it presses.
