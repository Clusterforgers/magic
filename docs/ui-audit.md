# UI audit and the look

The state of the web client's look and feel, the problems ranked by their cost to play, the chosen look and the game-feel plan. Rules stay in [ui-design.md](ui-design.md); implementation in [web.md](web.md).

## Method

Every screen is shot with `scripts/ui-shot.mjs` at 1400 × 900 and 390 × 900, the table also at 1280 × 720. The table runs on `/dev/table` with real card pictures (`cards=real`) in 1v1 (`board=duel`), the 4-seat Commander fixture (priority, attack, block, mulligan, crowded stack, attachments, counters), six seats and the boss fight (`boss=2`); the pages on `/dev/lobby`, `/dev/builder`, `/dev/precons`, `/dev/delve?state=…`, `/dev/read` and the plain routes. The result page and team tables run on `result=` and `teams=`.

## Problems ranked by impact on play

Impact: how much a problem costs a player in a 4-player Commander game at 1400 × 900, the main use. The look, the seat colours, the commander portraits, the stack pictures, the bright own board, the opening hand drawn large, the gold attack lines and the step-bound "nothing to do" note are in; what remains:

| # | Problem | Where | What a player meets |
|---|---|---|---|
| 1 | Crowded boards draw small cards | table, every board | Opponents' lands fold into their pods' chips and a grid's cards weigh as yours: with 20 permanents a seat and the side panel folded, an opponent's creatures draw about 66 px wide at 1400 × 900; with 40 a seat (14 creatures each) about 48 px, which no layout of three boards side by side lifts much further. Each opponent's board enlarges over the row from its pod (about 95 px cards in the crowd fixture); your row holds your creatures at 60 px first (about 62 px at 1400 × 900, 50 at 1280 × 720 where the pods take the rest). |
| 2 | Chrome still takes room | table | The side column takes 272 px until folded to its rail; an opponent's pod head is two to three lines; a card prompt below 1600 px adds a row of cards under the band. |
| 3 | The hand is still small | hand dock | The hand lies in an arc and rises on hover, but its cards stay about 90 px wide at 1400 × 900; a full-size hover is the preview beside it. |
| 4 | Combat reads in two places | combat overlay | An attack at a player is an arrow from each attacker to the defender's pod or tile and a badge on it per attacking seat in every layout; the block lines still cross the strip between them. |
| 5 | Choices share the strip | prompts | Card prompts lay 7 rem cards across the band and grow over the opponents' boards; a long list opens the centre overlay, and a mode choice stands beside its card's picture. |
| 6 | The phone table is tight | table at 390 | Boards fold their lands and draw 64 px cards that scroll, a card prompt lays its cards in the overlay; your pod still takes four lines over your board. |
| 7 | The turn is said in two places | top bar | The badge says whose turn and the step, the ladder names the phase; below 1024 px the badge names it instead. |
| 8 | The table is quiet between moments | table | The table plays its casts, attacks, blocks, damage, draws and shuffles and taps turn in 150 ms; there is no your-turn moment beyond the banner and no stack resolution beat. |

## Per screen

- **Commander table (4 seats).** Problems 1 to 4, 7, 8. Also: the right column opens on Commands (life ± buttons and a long command list) during play, where the Log or Stack is what a player reads; the "+N" overflow counts in board corners are easy to miss.
- **1v1.** Cards are large enough (about 90 px); the prompt paragraph is the largest thing in the strip. The pass button is bottom right as in Arena.
- **Crowded stack.** Grouping (×N), pictures and "+3 more" work; the targets arrows add lines to problem 4.
- **Boss fight.** The ability bar is clear; the boss pod's 120 life reads well. The rows share the height by content: the party's boards draw 100 px cards or more with ten permanents each at 1400 × 900, the boss side is capped, boards without permanents are one line and the opening hands float in the centre.
- **Six seats.** Tiles and the focused board work; tiles carry seat colours but no commander picture.
- **Phone.** Problem 6; the stack sheet covers most of the opponent board.
- **Play (landing).** On the page plaque and warm light; a list page without art, where open games and tutorials have one weight.
- **Lobby.** Clear and complete on the new tokens; decks show a placeholder icon instead of the commander's art; seat rows are text-heavy; Ready sits low.
- **Deck builder, precons.** Dense and functional; a precon row's Play is a quiet button, so the list reads as rows.
- **Delves.** Its own ember and stone; the descent uses icons where boss art would carry the mood.
- **Result page.** The headline in the display face above the win burst; each seat led by its commander's art in its seat's colour.
- **Sign in, Learn, Settings.** Plain and consistent.

## The prompt band

Every prompt kind is shot on `/dev/table?prompt=…` at 2550 × 1380, 2300 × 840, 1830 × 855, 1400 × 900, 1280 × 720 and 390 wide, and with empty boards (`seats=2&bare=1`). The rules are in [web.md](web.md) (Prompt band).

| Prompt | Band at 1830 and wider | 1280 to 1400 | Phone |
|---|---|---|---|
| Priority, yes or no, replacement order | one row: stack, status, sentence, answers; about 56 px | one row, the sentence cut with an ellipsis and opened by its button | sentence, then the answers |
| Number (X), a few options, mana choice | one row with the choices between the sentence and the answers | one row | the choices on a line of their own |
| Targets, cards, modes, costs, trigger order, payment, damage | from 100 rem of band the choices sit beside the sentence; below, a head row and the choices across the whole strip, cards from the left with the players in a column before them | head row and choices row | the long lists in the choice overlay |
| Attack, block | one row: the sentence and the combat buttons | one row | one row that wraps |
| Mulligan | at every table (duel, grid, teams, Two-Headed Giant, boss) the decision floats over the dimmed boards: the hand in one row of large cards, Keep and Mulligan, every seat's status; the dock hides its copy | the same | the hand in one scrolling row in the band |

- Empty boards: the sheet keeps at least a sentence line and a row of answers (`BAND_MIN_PX`), so a seat with nothing on the field above never shrinks the prompt to a scrolling sliver.
- What remains: a stack of three chips folds to one chip and "+2 more" beside a prompt below 1600 px; a mode choice stands as high as its card picture.

## Sweep at the owners' sizes

Every screen and table mode shot at 1873 × 1047, 1530 × 898, 1400 × 900, 1024 × 768 and 390 × 844 on the dev routes (`/dev/table` with `seats`, `teams`, `boss`, `permanents`, `crowd`, `alone`, `bare` and each prompt kind; `/dev/lobby`, `/dev/builder`, `/dev/delve?state=…`, `/new`, `/`). Screenshots are named `<case>-<width>`; ranked by cost to play, high first.

| # | Sev | Finding | Where | Shot | State |
|---|---|---|---|---|---|
| 1 | High | Five or more heroes crush your board to one cut line under the party's row | boss table, 6 heroes, every size | boss6-1400 | fixed: your board's floor comes before the allies' row |
| 2 | High | A card prompt's answers wrap into three rows in a strip-high sheet and the primary is cut | 4 seats, pay, 1024 | p-pay-1024 | fixed: the answers keep the first line in one row |
| 3 | High | Sticky answers on a phone are transparent: choices scroll behind Keep, Confirm order, Confirm payment | phone, mulligan, order, pay | p-order-390, p-pay-390 | fixed: an opaque answers row |
| 4 | Med | The opening hand's seventh card wraps under Keep and Mulligan | phone, mulligan | p-mull-390 | fixed: one scrolling row |
| 5 | Med | The band's top stack chip collapses to a dot at 1024 and drops its name at 1400 | every table with a prompt | ffa4-pri-1024 | fixed: label to a reader and "+N" under 64 rem |
| 6 | Med | "You have no creatures that can block" beside a prompt is a cut pill over the combat key at every width | every table in combat | duel-pri-1873, p-mode-1024 | fixed: a short status line |
| 7 | Med | Your pod's graveyard, exile and commander line scroll out of the pod column | 4 seats, teams, 2HG, 1024 to 1530 | ffa4-pri-1024, thg-attack-1530 | fixed: the blocks above the corner scroll |
| 8 | Med | Three heroes at 40 permanents: your lands lane is cut at the bottom | boss table, 3 heroes, 1400 | p-block-party-1400 | fixed: your board keeps its need at the floor first |
| 9 | Med | An attack badge on a tile lies over the seat's team chip | teams 3v3, 1400 | teams33-block-1400 | fixed: dot and number in tiles |
| 10 | Med | The choice overlay leaves the hand dock an empty band under the strip | 5 seats, targets-many, 1400 and 1873 | grid5-targets-1873 | fixed: only the opening hand hides the dock's cards |
| 11 | Med | The stack's target arrows run across the prompt sentence | phone, any prompt with a stack | p-x-390 | fixed: under the sheet with a faint copy above |
| 12 | Med | The boss-mode opening hand covers the boss's pod | phone, boss, mulligan | p-mull-boss-390 | fixed: a phone sheet keeps the head clear |
| 13 | Med | Pay at 1400: answers on two rows and the mana sources scroll sideways | 4 seats, pay | p-pay-1400 | fixed: one row, the sources wrap |
| 14 | Low | The run header fills the first phone screen before the room's choice | Delves map, camps, phone | dl-map-390, dl-campfire-390 | fixed: folded to the theme and hearts |
| 15 | Low | Shop offers of different heights put their Buy buttons on different lines | Delves shop, 1400 | dl-shop-1400 | fixed: each offer a column with its button at the foot |
| 16 | Low | Enter the fight and Retreat stand on two rows | Delves boss intro | dl-fight-1400 | fixed |
| 17 | Low | "1 relics" in the run summary | Delves victory | dl-victory-390 | fixed |
| 18 | Low | An alert's Retry row widens the icon column and indents the words 80 px | create page error, every alert with an action | pg-new-1400 | fixed |
| 19 | Low | Key hints (Esc, Space) are drawn on touch screens | phone prompts | p-x-390 | fixed: left out under a coarse pointer |
| 20 | Low | The deck detail is a tall empty panel while the seats run below the fold | lobby, 1400 | pg-lobby-2hg-1400 | pf10h |
| 21 | Low | End the run is plain text under the chronicle, unlike every other action | Delves, every state | dl-map-1400 | fixed: an outline button |
| 22 | Low | "No attack" is an icon-only button beside a labelled Attack | phone, attack | teams22-attack-390 | fixed: No attack and No blocks keep their words |
| 23 | Low | The commander tax badge lies over the commander's type line in the hand dock | every table | duel-pri-1400 | fixed: the pill in the middle of the art |
| 24 | Low | Hero names and figures of the run summary wrap on separate lines | Delves victory, phone | dl-victory-390 | fixed: one row, the relic names under it |
| 25 | Low | The deck list and groups, the replay and the open games list have no dev fixture, so they cannot be shot without a server | decks, replay, play | - | pf10m |

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
