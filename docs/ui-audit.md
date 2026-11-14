# UI audit and the look

The state of the web client's look and feel, the problems ranked by their cost to play, the chosen look, the game-feel plan and the ordered build plan for the overhaul. Rules stay in [ui-design.md](ui-design.md); implementation in [web.md](web.md).

## Method

Every screen is shot with `scripts/ui-shot.mjs` at 1400 × 900 and 390 × 900, the table also at 1280 × 720. The table runs on `/dev/table` with real card pictures (`cards=real`) in 1v1 (`board=duel`), the 4-seat Commander fixture (priority, attack, block, mulligan, crowded stack, attachments, counters), six seats and the boss fight (`boss=2`); the pages on `/dev/lobby`, `/dev/builder`, `/dev/precons`, `/dev/delve?state=…`, `/dev/read` and the plain routes. The result page and team tables run on `result=` and `teams=`.

## Problems ranked by impact on play

Impact: how much a problem costs a player in a 4-player Commander game at 1400 × 900, the main use. The look, the seat colours, the commander portraits, the stack pictures, the bright own board, the opening hand drawn large, the gold attack lines and the step-bound "nothing to do" note are in; what remains:

| # | Problem | Where | What a player meets |
|---|---|---|---|
| 1 | Crowded boards draw small cards | table, every board | With about 40 permanents a seat, opponents' permanents draw at about 40 px wide and names are cut; a board needs a hover per card. Lands take a third of each board. |
| 2 | Chrome still takes room | table | The side column takes 272 px for Commands; an opponent's pod head is three lines; a long prompt grows the strip to 140 px. |
| 3 | The hand lies flat | hand dock | Hand cards are larger (16 dvh) but stand in one flat row; Arena fans them from the bottom edge and lifts a hovered card to full size. |
| 4 | Combat lines cross the table | combat overlay | Lines run from the strip and pods across unrelated boards; "4 attackers → you" labels float over cards. |
| 5 | Choices are small | prompts | Choose cards and look at the top draw 6 rem cards in the strip; targets and modes have no pictures. |
| 6 | The phone table does not fit | table at 390 | Two and a half hand cards, the controls overlap ("…triggers" cut), the own pod takes a third of the height, boards draw 30 px cards. |
| 7 | Turn display repeats itself | top bar | "Main" in the badge, "Your turn · 5 · Precombat main" beside it and "Main" again in a ladder of twelve small empty boxes. |
| 8 | Motion and sound are thin for a game | table | Tapping is instant (no turn of the card); three quiet sine cues (turn, decision, win); no sound for a cast, attack, damage, draw or shuffle. |

## Per screen

- **Commander table (4 seats).** Problems 1 to 4, 7, 8. Also: the right column opens on Commands (life ± buttons and a long command list) during play, where the Log or Stack is what a player reads; the "+N" overflow counts in board corners are easy to miss.
- **1v1.** Cards are large enough (about 90 px); the prompt paragraph is the largest thing in the strip. The pass button is bottom right as in Arena.
- **Crowded stack.** Grouping (×N), pictures and "+3 more" work; the targets arrows add lines to problem 4.
- **Boss fight.** The ability bar is clear; the boss pod's 120 life reads well. The party's boards inherit problem 1.
- **Six seats.** Tiles and the focused board work; tiles carry seat colours but no commander picture.
- **Phone.** Problem 6; the stack sheet covers most of the opponent board.
- **Play (landing).** A list page without art; open games, join code and tutorials have one weight.
- **Lobby.** Clear and complete on the new tokens; decks show a placeholder icon instead of the commander's art; seat rows are text-heavy; Ready sits low.
- **Deck builder, precons.** Dense and functional; consistent with the tokens.
- **Delves.** Its own ember and stone; the descent uses icons where boss art would carry the mood.
- **Result page.** The headline in the display face above the win burst; the seats as a plain list.
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

## Phase 2 build plan

Each pass is one commit with before and after shots at 1400 × 900, 1280 × 720 and 390 × 900 of the screens it touches, and its tests (contrast in `tokens.test.ts`, layout in `scripts/ui-audit.mjs` and the e2e layout gate).

| Pass | Outcome |
|---|---|
| 3 | Feel: the sound set and its settings, the moments of the table, the your-turn moment, shuffle and draw, damage shake, tap and untap turning in 150 ms; reduced motion and leak tests |
| 4 | Hand and prompts: a hand fanned from the bottom edge with a full-size hover; choose cards and look at the top as a centre overlay with large cards; targets and modes with card pictures |
| 5 | Board room: opponents' lands folded into a summary row; combat lines kept to the boards they join, attacker groups as a badge on the defender's pod; the turn badge and ladder said once |
| 6 | Phone table: hand and controls fit at 390, pod strip slimmer, boards larger |
| 7 | Pages: Play with open games as cards with commander art, lobby deck rows with art, the descent with boss art; sign in, Learn and Settings on the new tokens |
