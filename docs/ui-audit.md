# UI audit and visual directions

The state of the web client's look and feel, the problems ranked by their cost to play, three visual directions to choose from, the game-feel plan and the ordered build plan for the overhaul. Rules stay in [ui-design.md](ui-design.md); implementation in [web.md](web.md).

## Method

Every screen is shot with `scripts/ui-shot.mjs` at 1400 × 900 and 390 × 900, the table also at 1280 × 720. The table runs on `/dev/table` with real card pictures (`cards=real`) in 1v1 (`board=duel`), the 4-seat Commander fixture (priority, attack, block, mulligan, crowded stack, attachments, counters), six seats and the boss fight (`boss=2`); the pages on `/dev/lobby`, `/dev/builder`, `/dev/precons`, `/dev/delve?state=…`, `/dev/read` and the plain routes. The end of a game and a team table have no dev scene yet (Phase 2, pass 0).

## Problems ranked by impact on play

Impact: how much a problem costs a player in a 4-player Commander game at 1400 × 900, the main use.

| # | Problem | Where | What a player meets |
|---|---|---|---|
| 1 | Cards are too small to read | table, every board | Opponents' permanents draw at about 40 px wide, yours at about 45; names cut to two letters ("S…", "Gri…"); art crops are unreadable, so a board is a mosaic that needs a hover per card. The three opponent boards share about 1080 × 270 px. |
| 2 | Chrome eats the boards' room | table | The side column takes 300 px (22 % of the width) for Commands; each opponent pod head is 130 px of chips; the shared strip is 140 px; the hand 115 px. Boards get under half the height. |
| 3 | The hand is small and cut | hand dock | Hand cards are about 70 × 98 px, cut by the bottom edge, in one flat row; a hand of 7 fills a third of the dock and the rest is empty. Arena shows large cards rising from the bottom edge and lifts a hovered card to full size. |
| 4 | Your own board reads as disabled | own board | While a decision is open every permanent you cannot use now fades to about 40 %, so your board looks greyed out and its state (damage, counters) is hard to read. Arena keeps the board bright and marks what is playable. |
| 5 | Pod heads are chip walls | opponent pods | Hand, library, clock, poison, energy, commander tax, monarch, mana pool and commander damage sit in four rows of equal 12 px chips. Life is the only strong figure; the commander has no picture; who is who rests on an 8 px dot. |
| 6 | The centre strip carries little | shared strip | A 140 px band holds three text chips for the stack and a paragraph prompt; a stale hint ("You have no creatures that can block") shows in the main phase and in the block prompt; the stack has no card pictures. |
| 7 | Big decisions are small | prompts | The mulligan, choose cards and look at the top are a paragraph and two buttons in the strip; the seven cards being judged stay in the small hand dock. |
| 8 | Combat lines cross the table | combat overlay | Thin white lines run from the strip and pods across unrelated boards; "4 attackers → you" labels float over cards; attacker and blocker sides differ only in line style. |
| 9 | No table identity | table, pages | The table is the same flat navy as the pages: no felt, no depth, no card shadow; panels, chips and buttons share one weight, so nothing says "game". |
| 10 | The phone table does not fit | table at 390 | Two and a half hand cards, the controls overlap ("…triggers" cut), the own pod takes a third of the height, boards draw 30 px cards. |
| 11 | Turn display repeats itself | top bar | "Main" in the badge, "Your turn · 5 · Precombat main" beside it and "Main" again in a ladder of twelve small empty boxes. |
| 12 | Motion and sound are thin for a game | table | Tapping is instant (no turn of the card); three quiet sine cues (turn, decision, win); no sound for a cast, attack, damage, draw or shuffle. The travel, strike and spotlight effects exist but read weakly on 40 px cards. |

## Per screen

- **Commander table (4 seats).** Problems 1 to 9, 11, 12. Also: the right column opens on Commands (life ± buttons and a long command list) during play, where the Log or Stack is what a player reads; the "+N" overflow counts in board corners are easy to miss; the commander-damage and tax chips use the same style as counts.
- **1v1.** Cards are large enough (about 90 px); the dimmed own board (4) and the empty strip (6) dominate. The pass button is bottom right as in Arena.
- **Crowded stack.** Grouping (×N) and "+3 more" work; the chips carry no art and the targets arrows add more lines to problem 8.
- **Boss fight.** The ability bar is clear; the boss pod's 120 life reads well. The party's boards inherit problem 1.
- **Six seats.** Tiles and the focused board work; tiles carry the same chip walls (5).
- **Phone.** Problem 10; the stack sheet covers most of the opponent board.
- **Play (landing).** A generic list page: no art, no sense of a game; open games, join code and tutorials have one weight.
- **Lobby.** Clear and complete; decks show a placeholder icon instead of the commander's art; seat rows are text-heavy; Ready sits low.
- **Deck builder, precons.** Dense and functional; consistent with the tokens.
- **Delves.** The strongest identity in the app (ember and stone); the descent uses icons where boss art would carry the mood.
- **Sign in, Learn, Settings.** Plain and consistent.

## Directions

Each direction is applied to the 4-seat table and the lobby behind the dev-only switch `?look=a|b|c` on any dev-server page (`src/dev/looks/`, kept for the tab; `?look=off` clears it). The production build carries none of it (`devBuild.test.ts`). They are skins over today's layout: the space problems (1 to 3, 5, 6) are layout work that every direction needs.

### A. Spellforge — a dark fantasy table

A premium digital card game: a lit green felt with a dark rim and a fine weave, warm obsidian plaques with a brass hairline for pods, the centre strip and the docks, cards that cast shadows on the felt, ivory text, a display serif for names and life, and a gold pass button that glows. It answers "a better feeling close to a real game" most directly and keeps the gold of your turn as the one warm accent family. Risk: the brass and gold must stay off state colours (attacking gold, selected crimson).

| Token | Value |
|---|---|
| Colour | ground `oklch(0.15 0.012 60)`, surface `0.2 0.016 60`, felt `0.36 0.075 160` to `0.15 0.035 170`, text `0.95 0.018 85`, muted `0.76 0.03 80`, border `0.34 0.03 70`, brass `0.72 0.1 78`, primary `0.8 0.13 78` |
| Depth | plaques `0 10px 28px` 55 % plus a 7 % top highlight; cards `0 3px 8px` 60 %; lanes inset `0 2px 6px` |
| Radius | 10 px panels, 6 px cards, 12 px lanes |
| Type | Geist for UI; a display serif (Cormorant Garamond or Cinzel, OFL, through fontsource) for names, life and page titles; small caps headings |
| Spacing | today's 4 px scale |

### B. Clean Slate — flat, card first

A neutral, frameless table where only the cards carry colour: no seat frames (space and one surface step separate seats), chips without outlines, flat surfaces, a solid blue pill for the primary action, life a step larger. It reads fastest and costs least, but it feels like a tool more than a game.

| Token | Value |
|---|---|
| Colour | ground `oklch(0.16 0 0)`, surface `0.2 0 0`, table `0.13 0 0`, lane `0.165 0 0`, text `0.97 0 0`, muted `0.72 0 0`, border `0.26 0 0`, primary `0.68 0.16 252` |
| Depth | none; a 1 px black edge on cards |
| Radius | 6 px panels, 6 px cards, pill for the primary action |
| Type | Geist only; life 600 weight, tight tracking |
| Spacing | today's 4 px scale, seat gaps doubled |

### C. Nightglass — glass panels and seat colours

An indigo night table with soft light pools, translucent blurred glass for pods, strip and docks, and each seat in its own hue (`seatHue`): the seat's frame, a 2 px top edge on its pod and its life figure, with the active seat glowing. Seat identity is the strongest of the three, which helps most in 4-player pods; blur costs paint time on crowded boards and contrast varies with what is behind the glass.

| Token | Value |
|---|---|
| Colour | ground `oklch(0.14 0.03 275)`, surface `0.2 0.035 275`, glass `0.24 0.04 275 / 0.55`, text `0.96 0.01 275`, muted `0.76 0.03 275`, primary gradient `0.84 0.12 200` to `0.72 0.15 260`, seat `oklch(0.75 0.13 <seatHue>)` |
| Depth | `blur(14px) saturate(1.3)`, a 10 % top edge, `0 8px 24px` 35 % |
| Radius | 12 px panels, 14 px lanes |
| Type | Geist; life in the seat's hue with a soft glow |
| Spacing | today's 4 px scale |

### Recommendation

A, with C's seat hue on each pod's top edge and life figure: the felt and depth give the table its game feel, and the seat hue tells four players apart at a glance. B's restraint applies to the chips in every direction (problem 5).

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
| 0 | Dev scenes for the end of a game and a team table (`/dev/table?result=won|lost`, `?teams=2v2`), so every screen can be shot |
| 1 | The chosen direction's tokens in `index.css` (both themes, contrast held), the display font through fontsource, card shadows, the table felt and plaques; the dev looks retired |
| 2 | Pod heads: name, life and commander picture first; counts in one quiet line; rare counters behind the pod; seat hue on the pod |
| 3 | Space for boards: the side column folds to a rail by default below 1600 px and remembers; the centre strip shrinks to one line with the stack as card pictures at its right; cards grow (measured by `fitLanes`) |
| 4 | Own board and hand: no dimming of unusable permanents, a glow on what is playable; a larger hand rising from the bottom edge with a fan and a full-size hover; tap and untap turn in 150 ms |
| 5 | Combat lines: per-defender colour, thicker lines with end marks, attacker groups as a badge on the defender's pod instead of floating labels |
| 6 | Feel: the sound set and its settings, the moments of the table above, the your-turn moment, shuffle and draw, damage shake; reduced motion and leak tests |
| 7 | Prompts and hand decisions: mulligan, choose cards and look at the top as a centre overlay with large cards; stale hints removed; targets and modes with card pictures |
| 8 | Phone table: hand and controls fit at 390, pod strip slimmer, boards larger |
| 9 | Pages: Play with open games as cards with commander art, lobby deck rows with art, the descent with boss art; sign in, Learn and Settings on the new tokens |
