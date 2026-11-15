# UI audit and the look

The state of the web client's look and feel, the problems ranked by their cost to play, the chosen look, the game-feel plan and the items left open. Rules stay in [ui-design.md](ui-design.md); implementation in [web.md](web.md).

## Method

Every screen is shot with `scripts/ui-shot.mjs` at 1400 × 900 and 390 × 900, the table also at 1280 × 720. The table runs on `/dev/table` with real card pictures (`cards=real`) in 1v1 (`board=duel`), the 4-seat Commander fixture (priority, attack, block, mulligan, crowded stack, attachments, counters), six seats and the boss fight (`boss=2`); the pages on `/dev/lobby`, `/dev/builder`, `/dev/precons`, `/dev/delve?state=…`, `/dev/read` and the plain routes. The result page and team tables run on `result=` and `teams=`.

## Problems ranked by impact on play

Impact: how much a problem costs a player in a 4-player Commander game at 1400 × 900, the main use. The look, the seat colours, the commander portraits, the stack pictures, the bright own board, the opening hand drawn large, the gold attack lines and the step-bound "nothing to do" note are in; what remains:

| # | Problem | Where | What a player meets |
|---|---|---|---|
| 1 | Crowded boards draw small cards | table, every board | Opponents' lands fold into their pods' chips and a grid's cards weigh as yours: with 20 permanents a seat and the side panel folded, an opponent's creatures draw about 66 px wide at 1400 × 900; with 40 a seat (14 creatures each) about 48 px, which no layout of three boards side by side lifts much further. Each opponent's board enlarges over the row from its pod (about 95 px cards in the crowd fixture); your row holds your creatures at 60 px first (about 62 px at 1400 × 900, 50 at 1280 × 720 where the pods take the rest). |
| 2 | Chrome still takes room | table | The side column takes 272 px until folded to its rail; an opponent's pod head is two to three lines; a long prompt grows the strip to 140 px. |
| 3 | The hand is still small | hand dock | The hand lies in an arc and rises on hover, but its cards stay about 90 px wide at 1400 × 900; a full-size hover is the preview beside it. |
| 4 | Combat reads in two places | combat overlay | An attack at a player is a badge on the defender's pod per attacking seat, so the lines that remain join two boards; the block lines still cross the strip between them. |
| 5 | Choices share the strip | prompts | Card prompts take the strip's width and grow over the opponents' boards with 7 rem cards; a long list opens the centre overlay, and a mode choice stands beside its card's picture. |
| 6 | The phone table is tight | table at 390 | Boards fold their lands and draw 64 px cards that scroll, a card prompt lays its cards in the overlay; your pod still takes four lines over your board. |
| 7 | The turn is said in two places | top bar | The badge says whose turn and the step, the ladder names the phase; below 1024 px the badge names it instead. |
| 8 | The table is quiet between moments | table | The table plays its casts, attacks, blocks, damage, draws and shuffles and taps turn in 150 ms; there is no your-turn moment beyond the banner and no stack resolution beat. |

## Per screen

- **Commander table (4 seats).** Problems 1 to 4, 7, 8. Also: the right column opens on Commands (life ± buttons and a long command list) during play, where the Log or Stack is what a player reads; the "+N" overflow counts in board corners are easy to miss.
- **1v1.** Cards are large enough (about 90 px); the prompt paragraph is the largest thing in the strip. The pass button is bottom right as in Arena.
- **Crowded stack.** Grouping (×N), pictures and "+3 more" work; the targets arrows add lines to problem 4.
- **Boss fight.** The ability bar is clear; the boss pod's 120 life reads well. The party's boards inherit problem 1.
- **Six seats.** Tiles and the focused board work; tiles carry seat colours but no commander picture.
- **Phone.** Problem 6; the stack sheet covers most of the opponent board.
- **Play (landing).** On the page plaque and warm light; a list page without art, where open games and tutorials have one weight.
- **Lobby.** Clear and complete on the new tokens; decks show a placeholder icon instead of the commander's art; seat rows are text-heavy; Ready sits low.
- **Deck builder, precons.** Dense and functional; a precon row's Play is a quiet button, so the list reads as rows.
- **Delves.** Its own ember and stone; the descent uses icons where boss art would carry the mood.
- **Result page.** The headline in the display face above the win burst; each seat led by its commander's art in its seat's colour.
- **Sign in, Learn, Settings.** Plain and consistent.

## The look

Spellforge, a dark fantasy table, with each seat in its own colour: a lit green felt with a dark rim under the boards, warm plaques with a soft shadow for pods, tiles and the centre strip, a brass hairline on seat frames and docks, cards that cast a shadow on the felt, ivory text, Cinzel for life, seat names and page titles, and a gold main action with a glow. Each seat's pod and tile carry a 2 px top edge in the seat's hue and its life figure in that hue, so four players tell apart at a glance. Tokens and rules are in [ui-design.md](ui-design.md) (Design system).

| Token | Dark | Light |
|---|---|---|
| Ground, surface, raised | `oklch(0.17 0.012 60)`, `0.215 0.016 60`, `0.26 0.018 60` | `0.984 0.006 85`, `0.995 0.004 85`, `0.995 0.004 85` |
| Felt, rim, lane | `0.33 0.06 160`, `0.15 0.03 170`, `0.17 0.035 165` | `0.93 0.035 155`, `0.89 0.04 160`, `0.9 0.03 150` |
| Text, muted | `0.95 0.018 85`, `0.75 0.03 80` | `0.2 0.02 60`, `0.47 0.025 60` |
| Primary, brass | gold `0.8 0.13 78`, `0.72 0.1 78` | bronze `0.48 0.11 60`, `0.55 0.1 70` |
| Seat | edge `0.74 0.13 <hue>`, life `0.88 0.09 <hue>` | edge `0.6 0.14 <hue>`, life `0.42 0.12 <hue>` |
| Depth | plaques `0 6px 18px` 30 % and a 7 % top light; cards `0 2px 6px` 45 % | the same |
| Radius, type, spacing | 6 px cards, 8 px controls, 12 px panels; Geist for UI, Cinzel for display; the 4 px scale | the same |

## Game feel

Effects and sounds decorate the batch that arrived, as the table effects do today ([web.md](web.md), Table feedback): nothing waits for them, no answer is delayed, they come from public events only, so every seat sees and hears the same, and a hidden card never makes a sound of its own beyond what its public move shows (a draw by another seat is one card-slide, never its name or kind).

| Moment | Motion (full) | Sound | Reduced motion |
|---|---|---|---|
| Draw | card back slides from the library to the hand (exists) and the new card lifts 8 px for 300 ms | card slide | the new card outlined |
| Shuffle | the library chip riffles (three 60 ms offsets) | riffle | none |
| Cast | hand to stack (exists), spotlight (exists), a colour glow of the spell's identity under it | a chime per colour family, short | spotlight without motion |
| Activate, trigger | ability chip flies (exists); the source pulses once | soft tick | outline |
| Resolve | stack to its zone (exists); a ring expands from where it lands | low settle | mark only |
| Counter, fizzle | the chip cracks and fades | dull knock | mark only |
| Tap, untap | the card turns 90° in 150 ms | tap click (own actions only) | instant turn |
| Attack | attackers slide 8 px toward the defender; coloured lines draw in per defender | drum hit | lines appear |
| Block | blocker slides to its attacker's side; a shield mark | metal clank | mark only |
| Damage | strike lunge (exists); the target shakes 3 px; the number floats red | thump, pitch by amount | number only |
| Life change | life figure counts to the new value in 400 ms, red or green flash (exists as a diff) | gain shimmer, loss thud | new value at once |
| Your turn | banner (exists), the felt brightens from your side, the pass button glows | two-note horn | banner fades |
| Priority to you | the pass button pulses once | soft ping (exists) | none |
| Stack resolves to empty | strip relaxes | none | none |
| Win, lose | full-screen result over the table with the final board behind | stinger | static |

- Sound engine: extend `features/sound/` from one cue to a set with categories (table, cues, results), each with its own volume and a master volume and mute, kept in `localStorage` as today. At most three sounds per batch, merged by kind, and none for a snapshot.
- Production: Web Audio synthesis for interface cues (ping, horn, chimes, ticks: oscillators and envelopes, as `synth.ts` does) and filtered noise for physical sounds (slide, riffle, thump). If synthesis sounds thin for card handling, Kenney's Casino Audio pack (CC0) has card slide, place and shuffle; any asset ships under `web/public/sounds/` with its source and licence in `web/public/sounds/CREDITS.md`. No unlicensed asset.
- Settings: Effects (full, reduced, off; today's `mode`), Sounds (master, table, cues, results sliders, mute) in the table menu and on Settings; `prefers-reduced-motion` sets reduced by default; every effect ends at once on a new batch or a key press.

## Open items

Each item is one commit with before and after shots at 1400 × 900, 1280 × 720 and 390 × 900 of the screens it touches, and its tests.

- Commander art on open games: a commanders field on the lobby list (server), then each open game as a card with its commanders' art.
- Boss art: a picture for each boss of the Delves content, shown on the descent and the fight panel in place of the crown.
