# UI design

The rules and design the web client is built to: principles, design system, a fit-to-viewport table, turn feedback, the game browser, the definition of done and the task order. Game flow stays as in [arena-flow.md](arena-flow.md).

Tags: [official] a standard, vendor or Nielsen Norman Group (NN/g) page; [community] a guide, wiki or forum. `Sn` is a source below.

## Principles

| Principle | Applied here | Src |
|---|---|---|
| Visibility of system status | Turn, phase, connection and automatic passes are always on screen | S1 |
| Consistency, real-world words | One token set, button hierarchy and row pattern; Magic's words, Arena's pass labels | S1 |
| Control and freedom | Concede, Stop passing, Not ready and Leave are one reversible step | S1 |
| Error prevention | Confirm only what cannot be taken back: Concede, and in the lobby Close game, Leave game and Remove; a disabled Join says why; illegal cards dim | S1 S11 |
| Minimalism, disclosure | One primary action per region; the rest one menu level down, never two; key hints on buttons | S1 S8 |
| Visual hierarchy | Hand, pass button and life are biggest; state colour only for turn, phase, priority, legal | S2 |
| Proximity | A seat pod groups name, life, zones; 8 px inside a group, 16 between | S3 |
| Fitts's law | Pass is the largest target, bottom right; frequent controls 32 px or more, 44 on touch | S4 S5 |
| Contrast and focus | 4.5:1 text, 3:1 edges and rings, focus never covered, colour never alone | S12 to S15 S30 |
| Motion and timing | 100 to 300 ms, ease-out in; nothing delays an answer; a working state past 1 s | S6 S7 S16 |

## Design system

Tokens are CSS variables in `index.css`, mapped by `@theme inline` (S17); shadcn's names stay (S18). Surface and text steps follow Radix's roles (S19), `on-` pairs Material's (S20). `tokens.test.ts` keeps the contrast computed here.

| Role | Light | Dark | Use |
|---|---|---|---|
| background | `oklch(0.985 0.004 265)` | `oklch(0.17 0.012 265)` | page |
| surface | `oklch(1 0 0)` | `oklch(0.215 0.014 265)` | panels, rows |
| table | `oklch(0.955 0.006 265)` | `oklch(0.145 0.012 265)` | boards, sunken zones |
| raised | `oklch(1 0 0)` | `oklch(0.26 0.016 265)` | menus, dialogs |
| foreground | `oklch(0.20 0.02 265)` | `oklch(0.96 0.005 265)` | text, 15:1 or more |
| muted-foreground | `oklch(0.48 0.02 265)` | `oklch(0.72 0.015 265)` | secondary text, 5.7:1 |
| border | `oklch(0.90 0.008 265)` | `oklch(0.30 0.014 265)` | separators |
| border-strong | `oklch(0.60 0.02 265)` | `oklch(0.56 0.02 265)` | field and control edges, 3.7:1 |
| ring | `oklch(0.50 0.19 275)` | `oklch(0.74 0.14 275)` | focus, 2 px plus 2 px offset, 6:1 |
| primary, on-primary | `oklch(0.48 0.19 275)`, `oklch(0.985 0 0)` | `oklch(0.74 0.14 275)`, `oklch(0.17 0.012 265)` | main action, selection |
| turn-you, on-turn-you | `oklch(0.82 0.16 85)`, `oklch(0.20 0.02 265)` | `oklch(0.82 0.15 85)`, same | your-turn pill and banner |
| turn-you-edge | `oklch(0.55 0.14 65)` | `oklch(0.82 0.15 85)` | pod edge and ring, 4.8:1 |
| turn-them | `oklch(0.50 0.09 250)` | `oklch(0.74 0.09 250)` | opponent turn marker, pod and tile edge |
| phase-`<id>`, `-soft`, on-phase-`<id>` | [Phase themes](#phase-themes) | same | accent, tint and text of the four phases |
| success, warning, destructive | `oklch(0.50 0.14 150)`, `oklch(0.50 0.14 55)`, `oklch(0.50 0.20 27)` | `oklch(0.74 0.15 150)`, `oklch(0.80 0.15 70)`, `oklch(0.72 0.17 25)` | ready, urgent clock, lethal and Concede |
| mana fill, ink | blue `0.50 0.13 245`, red `0.52 0.20 28`, green `0.50 0.14 150`, black `0.30 0.02 300` with white ink; gold `0.82 0.14 85`, white `0.96 0.03 90`, colorless `0.78 0.01 260` with dark ink | same | pips, 5.6:1 or more |

| Token | Scale |
|---|---|
| Spacing | 4 px base as Tailwind's `--spacing` (S17): 4, 8, 12, 16, 24, 32, 48, 64 |
| Type | Geist Variable. 11/16 card labels only, 12/16 caption and log, 14/20 UI, 16/24 body and fields, 18/28 section, 24/32 title, 36/40 life with `tabular-nums`, display `clamp(2rem, 4vw, 3rem)`; weights 400, 500, 600 |
| Radius | 6 card, 8 control, 12 panel, full pill (`--radius` 0.5rem) |
| Elevation | e0 border; e1 `0 1px 2px` 8%; e2 `0 4px 12px` 12% menus; e3 `0 12px 32px` 20% dialogs; dark lifts the `raised` surface instead |
| Motion | fast 100 ms (hover, toggle), base 200 (menu, row), slow 300 (sheet, banner); effects stay at most 550 (`fx.css`); `--ease-out` `cubic-bezier(0.2, 0, 0, 1)` in, `cubic-bezier(0.3, 0, 1, 1)` out; linear for clocks only; under `reduce` opacity only, 100 ms |
| Icons | lucide-react: 16 px in dense rows, 20 in controls, 24 in menus; always a label or `aria-label`; mana symbols stay in `ManaPips` |
| Breakpoints | XS under 640, S 640 to 1023, M 1024 to 1279, L 1280 to 1599, XL 1600 up; container queries and `clamp()` inside the table (S21) |

| Component | States | Maps to |
|---|---|---|
| Button | default, hover, active, focus-visible, `aria-disabled`, loading; 32, 36, 44 px | shadcn `button` |
| Field, switch, segmented control | default, focus, invalid with text, disabled, selected | `input`, `native-select`, `switch`; add `radio-group`, `toggle-group` |
| Pill, surface, list row | neutral, success, warning, turn-you, turn-them; hover, selected | `badge`; a `surface` utility over `card` |
| Menu, popover, dialog, sheet | open, closed | `dropdown-menu`, `dialog`, `alert-dialog`; add `popover`, `sheet` |
| Status, tooltip, key hint | skeleton, empty, error, offline | `alert`; add `skeleton`, `EmptyState`, `tooltip`, `Kbd` |
| Game card | tapped, sick, legal, selected, dimmed, attacking, blocking, doomed | `Card` |
| Seat pod, stack group, turn badge, banner, ticker, ladder, pass button, hand dock | below | new |

## Table layout

Arena puts the opponent on top, your hand on the bottom edge rising on hover, the phase strip above it and pass buttons bottom right [S24]; Marvel Snap and Slay the Spire put end turn lower right [S26 S28]; Hearthstone lights End Turn when no action remains [S25]. The table follows, in a `100dvh` grid (S22) that never scrolls the page.

```
+-----------------------------------------------+------+
| top bar: turn badge, phase pips, menu          |      |
+-----------------------------------------------+ side |
| opponents: pods and boards                     | tabs |
+-----------------------------------------------+ Log  |
| shared: stack chips, ticker, prompt line       | Chat |
+-----------------------------------------------+ Stack|
| own: pod, lanes: creatures; lands, other      |      |
+-----------------------------------------------+      |
| hand dock                    | controls dock   |      |
+-----------------------------------------------+------+
```

| Region | Content | Size |
|---|---|---|
| top bar | home, turn badge, mini phase pips, menu (layout, theme, Concede) | 44 px |
| opponents | a pod and board per opponent | `minmax(0, 1fr)` |
| shared | stack chips, ticker, prompt line; a long prompt grows up into a sheet of at most 40 dvh into room the opponents' area keeps free, beside the pod column, so it covers no card, and never over your board (its cards are what a prompt asks you to click), the hand or the controls | `clamp(56px, 9dvh, 88px)` |
| own | pod, then the lanes of your permanents: creatures toward the middle, lands left and other permanents right | `minmax(0, 1.15fr)` |
| hand dock | overlapping fan, 62% of each card showing; hover or focus lifts it 40% and opens the preview | `clamp(112px, 17dvh, 200px)` |
| controls dock | pass controls (button 44 px or more, End turn, menu) and the stop ladder, one row with a Mine or Theirs switch, with Full Control | bottom right |
| side | Log, Chat, Stack tabs with unread counts (the stack also shows as chips in the shared region) | L, XL 280 to 320 px; M a 44 px rail opening over the table; S, XS a sheet |

- Lanes: a board draws its permanents in three lanes, lands, creatures and other permanents (artifacts, enchantments, planeswalkers, battles; a creature of any other type is a creature). Creatures face the middle of the table, lands sit at the left and other permanents at the right, on every board, an opponent's mirrored top to bottom. Lanes never overlap, an empty lane takes no room, and each lane has a faint ground (`--lane`). The command zone, graveyard and exile are the pod's corner beside the board.
- `fitLanes(box, counts, facing)` (pure, `laneFit.ts`) halves its way over a ladder of card widths from 224 px down to 44 px in 2 px steps (5:7, lands at 0.8) and returns the largest at which some shape fits every lane: one band of all three lanes, creatures over a band of lands and other permanents, or a band each; a tie goes to two bands. Cards shrink as counts grow, lanes wrap into more lines, and at 44 px the zone scrolls inside, creatures first, with an edge fade and a "+N" count. The duel board of an early Commander game draws cards 100 px wide or more at 1440x900 and 130 at 1920x1080. Card text stays at 11 px or more; the preview carries the rest.
- `group.ts` merges permanents equal in name, controller, tapped state, counters, attachments and summoning sickness into one stack with a ×N badge. Lands and tokens group at once, other cards when a zone would fall under 64 px. A stack opens in place on click or Enter, and while a prompt needs each member.
- Seats: 2 draws one wide opponent board beside a 168 px pod column; 3 and 4 draw two or three columns of one-line pods; 5 or more, or any column under 280 px, draws a tile strip and one focused board that follows the active seat. That is Grid or Focus mode; the menu overrides it. Opponent boards shrink before yours.
- S (tablet): the same grid, a side sheet, 44 px targets. XS (phone, portrait): top bar, a 56 px pod strip (active first), the focused opponent board, your board, the shared line, a 96 to 120 px hand dock; a tap on a pod focuses it. Landscape phones use S with a 32 px top bar.

## Turn and focus

Whose turn it is stays on screen as a token: Legends of Runeterra's attack token [S27], Hearthstone's lit End Turn button [S25]. Cues decorate the batch that arrived: none waits, delays an answer or adds a cadence, and text comes from public events, identical for every seat ([arena-flow.md](arena-flow.md)).

| Cue | When | Shows | Reduced motion |
|---|---|---|---|
| Turn badge | always, in the top bar | the phase's icon and name, then "Your turn · 5 · Precombat main" (accent fill) or "Bo's turn · 5 · Declare blockers" (tint, accent edge); [Phase themes](#phase-themes) | unchanged |
| Turn banner | turn number or active seat changes | a band over the middle of the top bar, "Your turn" or "Bo's turn" and the turn; 250 ms in, 600 held, 150 out; `pointer-events: none` | fade only, held 1.5 s |
| Active seat | during its turn | a 3 px edge inside the pod or tile (turn-you-edge or turn-them), with a faint tint on a tile; a dot after the name of a priority holder who is not active, named "Bo has priority"; your pod adds `your-turn-cue`, pulsing twice (5.6 s) and then resting; in Focus mode the strip leads with the active seat and the large board follows it, a 150 ms cross-fade | static ring, no fade |
| Ticker | every batch | one line: the newest non-minor log line, plus "Skipped Upkeep, Draw" when steps passed with no window for you; three lines, 3 s each | static |
| Ladder | a step changes | current step lit in the phase's accent, passed steps filled, a skipped step dashed for 1 s | no sweep |

- Only your own skips are explained ("You had no plays"). Another seat's pass never says if it was automatic, how long it took or who is deciding.
- `turn/feed.ts` is pure over `(prev, next, events)`; a test replays two transcripts that differ in one hidden card and requires identical feeds.
- A polite live region announces the banner. The tab title cue stays.

### Phase themes

Four themes mark the phase of the turn at a glance. A step maps to one by the step alone, which every seat sees (`turn/phase.ts`). Arena lights the current phase's symbol in one orange [S24] and Hearthstone lights one button [S25]; here the hue changes with the phase, and an icon and a name sit beside it.

| Phase | Steps | Icon | Accent, light | Accent, dark |
|---|---|---|---|---|
| Beginning | untap, upkeep, draw | sunrise | `oklch(0.54 0.16 250)` | `oklch(0.84 0.09 250)` |
| Main | precombat and postcombat main | wand | `oklch(0.44 0.11 170)` | `oklch(0.68 0.157 170)` |
| Combat | beginning of combat to end of combat, first-strike damage included | swords | `oklch(0.45 0.147 36)` | `oklch(0.68 0.16 36)` |
| Ending | end step, cleanup | moon | `oklch(0.54 0.097 342)` | `oklch(0.68 0.09 342)` |

Each theme has `phase-<id>` (accent), `phase-<id>-soft` (tint, lightness 0.95 light and 0.30 dark) and `on-phase-<id>` (text on the accent, `on-primary`'s values), in both colour schemes. The table root carries `data-phase`, and `phase.css` points `phase`, `phase-soft` and `on-phase` at that set; before and after the turns they stay neutral.

| Piece | Takes the theme as |
|---|---|
| Top bar | a 3 px rule in the accent |
| Turn badge | the phase's icon and name, then the turn's words: accent fill on your turn, tint with an accent edge on another seat's |
| Ladder | the current step in the accent (solid on your turn, tint with a heavier edge on another seat's), an icon before each run of steps of one phase, the name beside the run holding the step |
| Shared strip | a 2 px edge in the accent, inside the your-turn glow |
| Table | the accent at 5 % over the ground |

- Never colour alone [S30]: the badge and the ladder carry the icon and the name; whose turn it is stays in words and in fill against tint.
- The hues follow Okabe and Ito [S31]: blue, bluish green, vermillion for red and reddish purple for violet. None is yellow or orange, which is the gold of your turn.
- Contrast: text on the accent and on the tint 4.5:1 [S13]; the accent against every surface, its tint and the washed table 3:1 [S14]; text on the washed table 4.5:1. The wash is 6 % at most.
- Colour-blind: any two accents, and an accent and the gold of your turn, differ by CIEDE2000 [S33] 12 or more under full vision, protanopia, deuteranopia and tritanopia [S32].
- Motion: a change of theme fades in 200 ms and is immediate under reduced motion. Nothing waits for a theme, and none adds a delay to an answer.

## Playing is legible

Every play reads on the table without the log: what was played, from where, at what, and what it did. As in Arena [S25 S27], a spell travels from where it was to the stack, stands large for a moment, points at its targets and travels on to where it ends. The moves decorate the batch that arrived, as the effects of [web.md](web.md) (Table feedback) do: the table shows the true state when they start, nothing waits for them, and they come from public events only, so every seat that sees the same events sees the same moves.

### Moments

A move is a picture of the card (a card back for a card the viewer may not identify) that flies on an overlay layer, `aria-hidden` and without a pointer, from one place to another. A place is a card on the table (`data-object-id`) or a zone: `data-fx-anchor` on the hand dock (`<seat>:hand`) and the shared strip's stack (`stack`), the `data-zone` chip of a seat's pod (`data-seat`) or tile (`data-tile-seat`). A mark is a still attribute on a card (`data-fx`).

| Moment | Comes from | Moves | Marks | Log line |
|---|---|---|---|---|
| Cast | `SpellCast`; the zone before the batch, else the caster's hand | source to stack | spotlight, then target arrows | "Ana casts Lightning Bolt targeting Hill Giant" |
| Activate | `AbilityActivated`, `AbilityPutOnStack` (`source`, `snapshot`) | an ability chip from the source to the stack | `activated` on the source | "Ana activates an ability of Gravedigger targeting Grizzly Bears" |
| Trigger | `AbilityTriggered`, `AbilityPutOnStack` with `cause` | an ability chip from the source to the stack | `triggered` on the source | "Soul Warden triggers" |
| Target | `ObjectView.targets`, else `choices.targets` of the event | none | `targeted` on each target while the item is on the stack; arrows | in the cast or activate line |
| Resolve | `ResolutionEnded` `Resolved`; the zone after | stack to the battlefield (a permanent) or the owner's graveyard | `resolved` on what it touched | "Lightning Bolt resolves" |
| Counter | `ResolutionEnded` `Countered`, inside the resolution of the counter | stack to graveyard | `countered` on the chip before it leaves | "Counterspell counters Lightning Bolt" |
| Fizzle | `ResolutionEnded` `Fizzled` | stack to graveyard | `fizzled` | "Lightning Bolt does nothing: its targets are gone" |
| Enter | `Entered`, a battlefield object new to the batch | from its last zone unless that was the stack | `entered` | the line of the event that moved it |
| Die, exile, bounce | `CreatureDied`, `ZoneChanged` (`cause`) | the ghost, then battlefield to graveyard, exile or hand | `died`, `exiled`, `bounced`; an arrow from the resolving object | "Hill Giant dies", "Hill Giant is exiled by Swords to Plowshares" |
| Draw | `CardDrawn`, public or hidden | library to hand, a card back for another seat | `drawn` on the viewer's new hand card | "Ana draws a card" |
| Play land | `LandPlayed` | hand to battlefield | `entered` | "Ana plays Forest" |
| Other moves | `ZoneChanged` (mill, discard, return) | old zone to new zone | none | the event's line |

- The resolving object at an event is the last `ResolutionBegan` without its `ResolutionEnded`; what it destroys, exiles, bounces or counters gets a short arrow from it.
- A spell cast and resolved in one batch (no seat had a decision) plays both legs with the spotlight between, so a spell never happens unseen.
- A batch plays its moves in event order, `--fx-stagger-ms` apart, at most `MAX_MOVES` (8); the rest are marks only. A newer batch finishes the moves of an older one at once.

### Motion tokens

| Token | ms | Easing | Use |
|---|---|---|---|
| `--fx-travel-ms` | 380 | `--ease-out` | one leg of a move; the picture takes the size of where it lands |
| `--fx-spotlight-ms` | 900 | in 150 `--ease-out`, out 150 `--ease-in` | the large spell after a cast by another seat, or one that passes through the stack in one batch; the viewer's own cast stands 0.6 of it (540 ms) with the same fades |
| `--fx-stagger-ms` | 120 | none | between the moves of one batch |
| `--fx-arrow-ms` | 250 | `--ease-out` | an arrow draws in (stroke-dashoffset) |
| `--fx-mark-ms` | 600 | none | `activated`, `triggered`, `resolved`, `countered`, `fizzled`, `exiled`, `bounced`, `drawn` |

A single mark stays at most 600 ms; a whole sequence stays under 1,700 ms (two legs, the spotlight and the staggers). `--fx-slow` stretches all of them.

### Reduced motion

Under `prefers-reduced-motion: reduce` nothing travels or scales: the source and the destination take their marks as still outlines at once, the spotlight appears and goes without motion for as long as it stands, marks show as badges that simply appear and go, and arrows appear without drawing in. The store drops moves under `reduced` as it drops ghosts.

### The stack

- The shared strip's stack chips stay; the top chip gains a targets line ("→ Hill Giant, Bo", at most 3 names, then "+N more") and a dot in its controller's colour. Every chip's label for a screen reader ends with ", targeting Hill Giant and Bo".
- Arrows run from the top chip to each target (a card, a player's pod, or the chip of another stack item) while it is on the stack, at most 6; hover or focus on any chip draws that item's arrows instead. They share the combat arrow layer (its own layer above the strip), in `--primary` for the viewer's items and `--destructive` for another seat's, and appear without drawing in under reduced motion.
- A target that is gone (its player lost, or it left where it was since the item went on the stack) gets no arrow and drops out of the line; an item with none left says "no legal target" on a dashed chip.
- The spotlight is the card at 1.6 times the hand card's height (at least 168 px, at most 40 % of the window, shrunk to 96 px at the least to fit above the strip), centred over the shared strip with its foot above the strip, and under it a caption: who casts, the name and the targets line. It stands for the first cast of a batch that another seat made or that passes through the stack, comes in and goes out in 150 ms each, takes no pointer, shows a card back for a card the viewer may not identify, and a newer batch with moves replaces it. It never stays, so the layout gate measures the table without it.
- The viewer's own cast (a cast by the viewer's seat that stays on the stack; one that resolves in its batch stands in full) stands in the same place and size, but for 0.6 of `--fx-spotlight-ms` (540 ms: the 150 ms fades and a hold of 240 ms) after its first leg, in a frame of lower contrast (`--border` instead of `--border-strong`, a lighter shadow) and with the caption "You cast". The viewer has just chosen it, so it only confirms: nothing waits for it, the priority prompt and the hand answer as at any time, and the layer takes no pointer. A cast of another seat in the same batch takes the spotlight; a newer batch with moves replaces it. A card without a face to show stands for no spotlight, never as a back. Under reduced motion it is still, for the same time.
- A mark is a badge with an icon and a short word (`FxMark`) on the card or chip it names, its edge and icon in a token colour (primary: activated, bounced; warning: triggered; success: resolved, drawn; destructive: countered; border-strong: fizzled; turn-them: exiled) and its words in `--foreground` on `--card`; `markTokens.test.ts` holds the contrast. A spell that resolves, is countered or fizzles carries its badge on its flying card.
- A resolving object points a short arrow at what it counters, exiles, bounces or destroys, from the box it had when the batch arrived to the box of its target, on the layer above the strip for `--fx-arrow-ms` plus `--fx-mark-ms`; an object cast in the same batch has no box to start from and gives no arrow.
- The Stack tab shows the top item as a large picture with its text and one row per target (a gone one struck out), the rest as rows with their target line.

### The hand

- While the viewer holds priority, a playable card (`castable`, `playable_lands`, `faces`, `suspendable`, `foretellable`) is legal: ring and lifted 4 px. Every other hand card is dimmed and carries a reason. Without priority the hand is unmarked.
- The reason is a short label on the card's foot on hover and focus, the spoken name ("not available, sorcery speed") and the line under the name in the preview ("Only in your main phase with an empty stack."), which a tap on the card pins, so touch reaches it.

| Reason | Label | Line |
|---|---|---|
| `Timing` | Sorcery speed | Only in your main phase with an empty stack |
| `LandPlayed` | Land played | You have played a land this turn |
| `Mana` | Needs mana | Not enough mana |
| `Targets` | No targets | Nothing it can target |
| `Cost` | Can't pay | A cost cannot be paid |
| `Restricted` | Not allowed | An effect forbids it now |
| (unknown) | Not now | Not playable now |

- The reason is the server's (`PriorityOptions.unplayable`). For a card it does not name, the web reads `Timing` and `LandPlayed` off public state (types, `Flash`, step, stack, `lands_played`) and `Mana` when the mana value exceeds the untapped permanents with mana abilities plus the pool; anything else is "Not now".
- A permanent the viewer controls, or a card of its graveyard, exile or command zone, with an ability in `PriorityOptions.abilities` is legal as a hand card is, and its spoken name says "Activate {T}: Scry 1" (or "Activate one of 2 abilities") in the card's words (`ObjectView.activated`). A click activates it; a card with another play (a commander that can be cast, a permanent that taps for mana) or several abilities asks which in a small chooser at the card: one line per play, the keyword or the cost and effect as the card words them, in the look of the manual menu (a sheet at the foot of a phone). Arrows, Home and End move along the lines, Escape, Tab and a press outside close it, and a second click on the card closes it.
- A mana ability is never listed there and never a play; an opponent's card is never marked, a spectator has no marks, and the abilities of a hand card (cycling) are not offered from the table.

### What the protocol carries

- Carried: every moment above, from `SpellCast`, `AbilityActivated`, `AbilityPutOnStack` (with the source's `snapshot`), `AbilityTriggered`, `ResolutionBegan`, `ResolutionEnded`, `Entered`, `ZoneChanged`, `CreatureDied`, `CardDrawn`, `LandPlayed`, the hidden forms of the moves, `ObjectView.targets` and `source`, and the two moments of the batch. A cast object keeps its id from hand to stack to battlefield, so source and destination are its zones before and after.
- Also carried: `PriorityOptions.unplayable?: Array<UnplayableCard>`, `UnplayableCard = { card: ObjectId, reason: Unplayable }`, `Unplayable = "Timing" | "LandPlayed" | "Mana" | "Targets" | "Cost" | "Restricted"`: one entry for each card of the holder's hand and command zone that is not offered, with the first reason found in that order. Only the holder sees options, so nothing leaks.

### Code

- `fx/moves.ts` (pure): `deriveMoves(before, after, events)` gives `Move[]` (`kind`, `object`, `look`, `from`, `to`, `order`, `targets`) and the marks; `MOVE_MS` holds the tokens, which `fx.css` repeats.
- The travel layer (`fx/travel.ts`, `fx/places.ts`, `fx/FxTravel.tsx`) measures a move's source in the store subscriber of `watchBatches`, which runs before React draws the batch, so a card that left the hand is measured where it stood; it measures the destination on the next frame, a place it does not find falls back to its zone, and a move with no place on screen is left out.
- `fx/spotlight.ts` (pure) plans the spotlight and its boxes, `fx/Spotlight.tsx` draws it; `fx/marks.ts` (pure) picks the marks and arrows of a batch, `fx/FxMark.tsx` draws a badge, `fx/effectArrows.ts` holds the arrows and `combat/effectLines.ts` gives them to the combat overlay.
- `handReason.ts` (pure): `unplayableReason(object, view, options)` gives the reason of a dimmed hand card; `REASON_WORDS` its label and line, which `CardNote.line` carries to `CardPreview`.
- `prompt/activations.ts` (pure): `abilityPlays(options, objectsById, seat)` gives the cards with an ability to activate and their `Play`s, `playsOf` the plays of one card (spell, abilities, mana); `prompt/AbilityChooser.tsx` draws the chooser.
- `/dev/table?fx=` `cast`, `rival` (another seat casts), `activate`, `resolve`, `counter`, `land` and `draw` play these moments on the fixture table, beside `attack`, `block`, `damage`, `death` (the die moment), `life` and `enter`.

## Attachments

An Aura, Equipment or Fortification is read at its host, as Arena and MTGO draw it: the attachment lies behind the host with its name line showing above it, the host carries a mark for what each attachment gives, and a move between hosts is drawn as an arrow and said in the log.

| Part | Shows | Data |
|---|---|---|
| Tuck | every attachment on a permanent lies behind its host, whoever controls it, each showing its top `tuck` px (the name line) above the one in front; the host stands at the foot of its box; an attachment takes no box of its own | `ObjectView.attachments`, `attached_to` |
| Fit | a lane's line is `height + depth × tuck` tall, `depth` the most attachments tucked at one host of the lane; `tuck` is the land fan's head (`max(fanHeadMin, 0.14 h)`) while the fan stays under half the card's height, then `max(12, h / 2 / depth)`; a lands lane with a tuck is not fanned | `LaneCounts.tucked` |
| Order | Auras, then Equipment, then Fortifications, each in id order; the first lies deepest, the last right behind the host | view only |
| Grant marks | one mark per attachment on the host's foot stack, read top to bottom as the tucked cards are: its gives in whole words, the set value, the change ("+2/+2"), the rules ("Umbra": the host survives destruction; "Can't attack or block"), then the keywords ("Flying", "Pro red"), in one line while the face is wide enough, else in the short words of each rule ("Pacified", "No attack", `GiveItem.brief`), else in two lines that break at spaces, else the first items and "…" (never a cut word); an attachment that gives nothing is marked by its name; attachments that give the same, and are controlled alike, are one mark with "×2"; past the lines the foot stack has (one line each when two lines would not fit), the first `room - 2` marks stay and the rest are one mark that sums what the other attachments give ("+5/+3 Haste Shroud": power and toughness added, rules and keywords once each; "+N" while they give nothing) in the two lines left, in a neutral colour, with all of them in its title; a compact face, or one lying on its side, shows the count and one icon; tokens `success` for a raise, `destructive` for a lowering or a restriction, `foreground` on `surface` for the rest | `ObjectView.gives` |
| Preview | the host's preview lists each attachment as one line: its name, its controller when it is not the host's, and what it gives ("Bo's Pacifism: can't attack or block"); the attachment's own preview names its host or player and what it gives it ("Attached to Hill Giant, giving +2/+0, trample") | `gives`, `attached_to` |
| Foreign | an attachment another seat controls carries a 2 px dashed edge in `border-strong` around its tucked card and "Bo's" on its name line (the edge alone below 76 px; the spoken name says "Controlled by Bo"), and "Bo's" before its name in the host's spoken name and preview (Pacifism on your creature, your Aura on an opponent's), with the same dashed outline on its grant mark | `controller` |
| On a player | an Aura or Curse on a player stays in its controller's other lane with "On Bo" (`card-attached-to`); the enchanted player's pod and tile show a chip for each name and controller, "×3" when several share one: a button in the pod that previews the Aura, an image in the tile whose spoken name lists them; the preview of either names the other | `attached_player` |
| Unattached | an Equipment or Fortification on no host stays in its controller's other lane with a dashed "Unattached" chip at its foot (the icon alone, the word spoken, below 76 px) | `attached_to` missing |
| Move | `Attached` draws an arrow from the old host to the new host with the combat overlay's `connect` (`combat/geometry.ts`), from inside one box to inside the other since hosts of a lane stand a few pixels apart; the log and the ticker say "Bonesplitter moves from Hill Giant to Llanowar Elves", "Pacifism is attached to Hill Giant" when it hung on nothing and "Pacifism enchants Bo" on a player; `Unattached` says "Bonesplitter becomes unattached"; an attachment that enters attached has no old host and draws no arrow | the previous view's `attached_to`, kept on the log entry as `hostBefore` |
| Reduced motion | the arrow draws in over 450 ms and stands for 1 s; under reduced motion it is drawn still for the same 1 s and fades in 100 ms; the tuck has no motion | none |

- Code: `play/hosts.ts` (pure: `hostOf`, `hostsBefore`, `isUnattached`), `fx/derive.ts` `deriveAttaches`, `fx/arrows.ts` (the arrows playing, 1 s each), `combat/attachLines.ts` (the arrows as lines of the combat overlay's layer, styled by `combat/attach.css`), `TuckedCard.tsx`, `enchantChips.ts` with `EnchantChips.tsx`, `attachMarks.ts` (pure: `marksOf`, `foldMarks`, `sumGives`, `roomFor`, the words) over `markLayout.ts` (the measured glyph widths and the lines) with `GrantMarks.tsx`, `previewText.ts` `attachLine` and `hostLine`, and `/dev/table?fx=equip`, which moves the viewer's Equipment to its next creature.

- Many: five attachments on one host keep a name line each of 21 px at 150 px cards and 14 px at 100 px; the foot stack shows the first marks and sums the rest into one (a host of 100 px cards with five attachments shows "+8/+4 Umbra First strike …" in its two lines), and the preview always lists them all.
- Phone (390 px): a tuck shows the attachment's colour edge only below 76 px cards; the marks are the count and an icon, and the long-press preview carries the list.
- `gives` is the engine's account of each attachment's continuous effects on its host as the layers apply them now: `{ power, toughness, set, keywords, rules }` with `set` the power and toughness it sets ("is a 1/1") and `rules` the other changes it makes (`CantAttack`, `CantBlock`, `CantBeBlocked`, `DoesntUntap`, `LoseAbilities`, `SetController`, `UmbraArmor` and the rest of `GivenRule`); left out while the attachment gives nothing or is unattached. The view cannot work it out: `abilities` carries mana abilities only and a card's words do not say what a condition or a layer leaves of them.

## Manual commands panel

The seat does by hand what the engine does not run from one persistent panel at the right of the table, laid out as Table Commander lays out its tools: the seat's own numbers on top with quick amounts, then the library, then what it makes, then the card it picked. The card menu stays as the shortcut for one card.

| Size | Where | Why |
|---|---|---|
| L, XL | the first tab of the side column, "Commands", before Log, Chat and Stack, and the tab on show when the table opens | the column keeps its 280 to 320 px, so the card sizes the layout gate protects do not change; the log keeps its unread count and the panel's own feedback says what an action did |
| M | the first icon of the rail, opening a 320 px panel over the boards, from under the top bar down to the hand and controls row | the same 44 px rail; the hand, the pass control and your life stay in reach, and the same icon closes the panel |
| S, XS | the first tab of the side sheet, a non-modal sheet that hangs from the top bar over the opponents and the shared strip (and over your board where those are under 300 px high), full width at XS and 24 rem from S; the top bar's sheet button reads "Commands, log, chat and stack" and toggles it | your board with your life, the hand and the pass control stay in reach; a press on the tab on show keeps the sheet open, Close shuts it |

| Group | Commands, in order | Action |
|---|---|---|
| You | the Life counter and "More life options"; Poison; Other counters (energy, experience, a named kind) | `Life`, `PlayerCounters` |
| Library | Draw; Look at the top (scry, surveil, reorder); Mill; Exile from the top; Put on the bottom; Shuffle; Reveal the top card | `Draw`, `MoveTop`; new: `LookTop`, `ArrangeTop`, `Shuffle`, `Reveal` |
| Make | Tokens (the common list, a custom token); Mana | `CreateToken`, `AddMana` |
| Card | the picked card's commands, as its card menu lists them: Move to, Counters, Tap, Attach, Deal damage, Pump, Reveal, Note | `Move`, `Counters`, `Tap`, `Untap`, `Attach`, `Unattach`, `Damage`, `Pump`; new: `Reveal`, `Note` |

- A group is a heading and its rows; a row is a button with the command's icon at the left, its name and one line of what it does, and a chevron at the right that turns down while the row is open (it does not animate under reduced motion) ("Draw: put the top cards of your library into your hand"). A press opens the row in place (one open at a time): an amount with presets (life and counters −5 −1 +1 +5, cards 1 2 3 and a field), the preview line in the seat's words ("Ana: 40 → 35 life", "Draw 2: library 52 → 50, hand 4 → 6", "Mill 3: the top 3 cards go to your graveyard"), and one Apply button that names the change ("Lose 5 life"). Nothing is sent before Apply. A row that opens scrolls the panel (and only the panel) by the least that shows its Apply, keeping its head in view. Mill, Exile from the top, Top to the bottom and Shuffle say "This cannot be undone: the library's order is lost." above Apply; a change of life or counters is corrected by the opposite change. In the look-at-the-top list the up and down moves sit on the card's name line and Top, Bottom and Graveyard on one line under it.
- Life is an open counter at the top of You, as in Table Commander: the seat's name and total in large figures, and -5, -1, +1 and +5 under it, with no press to open it. A press adds to a pending change and the total shows the running total; the line under the buttons reads "Not sent yet: -5 (40 → 35 life)" with "Undo -5". After 600 ms without a press the pending change is sent as one `Life` action, so a run of presses is one log line, and the line says "Sending: Lose 5 life…", then "Done: Lose 5 life." or "Refused: <reason>." as for any row. One change is in flight at a time and presses made meanwhile wait for its answer; a change still pending when the tab changes is sent, not lost.
- Undo: while a change is pending it drops it and sends nothing; once the server took a change it is offered for 8 s, only while life is where that change left it, and sends the opposite change as one action (that action has no Undo of its own); the first press of a new run takes the link away. "More life options", under the counter, is the Life row: any amount, the preview and Apply.
- Keyboard: the four buttons are the counter's tab stops, a `group` named "Change life" that says its keys; from any of them Arrow Up and Arrow Down change the pending amount by 1, Page Up and Page Down by 5 and Escape forgets it (Enter and Space press the button as usual). The total is no `spinbutton`, so a prompt's number field is the only one on the table. A polite live region says "Ana: 35 life, not sent yet" and then "Ana: 35 life"; the buttons are 40 px with a mouse and 44 px on a touch screen.
- Look at the top: Apply sends `LookTop`; while the seat's last look is open (no `ArrangeTop` or `Shuffle` since, read from the log) the row lists the cards looked at (`PlayerView.known_top`), each with its picture when the table has one, in three lists (stays on top, goes to the bottom, goes to the graveyard) with Top, Bottom and Graveyard and up and down moves. Apply names the change ("Put 2 on the bottom, 1 in the graveyard") and sends one `ArrangeTop` of exactly those cards, a surveil when nothing goes to the bottom and a scry otherwise; nothing moving is "Keep all 3 on top" and sends nothing; Cancel and Escape close the row and keep the cards looked at; "Look at more" opens the amount form. Alt with the arrows moves a card; T, B and G send it. The seat's own log names the cards ("Ana looks at the top 3 cards of their library: A, B and C"; "puts back the top 3 cards: A on top, B and C on the bottom"); the other seats read "Ana looks at the top 3 cards" and "Ana puts back the top 3 cards: 1 on top, 2 on the bottom".
- Card: "Pick a card" arms the next press on the table: a banner "Pick a card for its commands. Escape cancels." with a Cancel button, every card ringed as a target, and a click, Enter, Space, a tap or a right click on a card chooses it without doing what the card does otherwise. A rail or sheet that covers the table steps aside while the card is waited for and returns with it. The card menu's "In the panel" chooses without arming. The picked card shows as a line with its name, place and a clear button, then its commands as the card menu lists them (Move to, Discard, Tap or Untap, Counters, Attach to, Unattach, Deal damage, Pump), each a row with the same choices, preview line and Apply; a command the card cannot take is listed inert with its reason. "Pick another card" arms again and Escape returns to the card it had. Without a pick the group says "Pick a card on the table to act on it".
- After Apply the row says "Sending: Lose 5 life…" until the server answers, then "Done: Lose 5 life." or "Refused: <the engine's reason>." in the row's own feedback (`role="status"`, red on a refusal), and keeps its choices on a refusal; the table notice is not used. A second Apply waits for the answer. The log says the action as for the card menu.
- Manual actions are allowed at any time, whoever holds priority; the panel is inert with the reason once the seat has lost or the game is over. A spectator has no Commands tab.
- Keyboard: the panel is a `region` named "Commands"; Tab walks the rows, Enter or Space opens one, the amount is a `spinbutton` (arrows ±1, Page Up and Page Down ±5), Enter applies, Escape closes the row and returns to it. The preview is `aria-live="polite"`. Targets are 44 px on a touch screen and 32 px with a mouse.
- The card menu keeps the commands about its card (Move to, Counters, Tap, Attach, Deal damage, Pump, Reveal, a note); life, draws, library cards, tokens and mana are the panel's, and the menu's last entry, "In the panel", opens the panel on that card. The menu stays up with "Sending…" until the server takes the action and shows a refusal as "Refused: …" in its header.
- Reveal and Note show on the table to every seat: a seat's revealed hand cards and top card are an eye chip with their count beside its hand and library chips, opening the cards face up; a revealed card carries a "Revealed" mark and a note its first words on the card's face (the icon alone on a compact face), and the preview lists both in full.
- Dice and a coin are not in the panel: a roll needs the engine's random stream (an item of its own).

## Game browser and lobby

`/` becomes Play: open games, Create game and Join with a code. Create moves to `/new`, an invite opens `/join/$code`, My games stays; a signed-in user also gets "Continue" for a running game. Visibility follows Steamworks lobbies (private, public, a joinable flag) [S23] and Unity's, which hide private lobbies from queries and join them by code [S29].

| Column | Content |
|---|---|
| Host | account or guest name |
| Format | Normal (Commander with M4) |
| Seats | "2 of 4" with dots; a full row dims and says "Full" |
| Decks | "Your own deck"; a rule column once deck rules exist |
| Age | "3 min ago" in a `<time>`, refreshed every 30 s |
| Action | Join, the one primary button per row |

- States: skeleton rows only after 300 ms (S9); empty ("No open games. Create one, or join with a code.") with both actions; error alert with Retry, stale rows kept and dimmed; filters (seats, format, hide full) above 8 rows.
- Updates: poll `GET /api/lobbies` every 5 s while the tab is visible, with `ETag`, and a Refresh button; server-sent events can replace it. A row that fills before the click says "Someone took that seat" and the list reloads.
- Join: a guest gets a name dialog (the name is kept in `localStorage`, in try/catch) and nothing about a deck, since the player chooses it in the lobby; a signed-in user joins in one click and the seat binds to the account. A row and the code field join the same way, and success opens `/game/{id}#seat=n&token=…` with the id from the answer. A code no open game has reads "No open game with that code."
- Create: seats from 2 with no most (a number field; default 4 in Commander, 2 in Normal), your name and deck, Visibility with its consequence beside it ("Listed in the game browser" or "Only people with your link or code"; default Listed), an Advanced disclosure for today's flow. The lobby shows invite link, code and a visibility toggle with Close for the host, Leave for others.
- Server: `visibility` (`public` or `invite`) on `POST /api/games` and in the record; a join takes a name (a guest needs one) and may name a deck, so an open seat takes a name and a token when claimed and plays that deck, else the one it was made with; `GET /api/lobbies` lists public lobby-phase games, newest first, at most 50: id, host name, format, seats filled and total, age, never a token or code; `POST /api/games/{id}/join` and `POST /api/join` (a code) answer `{game, seat, token}` or 409 `full` or `started`; `leave` and a host `release` free a seat before the start.
- Privacy: an invite game is never listed; an unknown id, an invite game without its code and a wrong code give one 404. A code is 8 characters of a 32-letter alphabet (40 bits), stored as a digest, 429 after 5 misses a minute from one address. An account holds one seat per game, so a signed-in repeat join returns its own.
- Accessibility: a real `<table>` with a caption from M up, stacked rows below; the button name is "Join Bo's game, 2 of 4 seats"; a polite "3 open games" line; polling never moves focus; a removed focused row sends it to the next Join.

## Definition of done

- Fits: the table never scrolls the page at the six sizes; other pages never scroll sideways; nothing is cut at 390 px; no text is under 11 px; every primary action is in the window and not covered.
- Contrast: token pairs pass in both themes; focus is visible and uncovered.
- Targets: 24 px floor, 32 px on desktop, 44 px under `pointer: coarse`.
- Keyboard: tab order follows reading order; every action works without a mouse; no trap.
- States: empty, loading, error and offline are designed.
- Motion: inside `no-preference`, a CSS test per file, nothing delays an answer.
- Evidence: screenshots at 1920x1080, 1440x900, 1366x768, 1280x720, 1024x768 and 390x844 in both themes, looked at, not committed; tests; `docs/web.md` updated.
- Gate: `web/e2e/layout.spec.ts` holds every page that needs no account, and the dev routes that stand for the pages that do, to Fits at the six sizes in both themes in Chromium, and `web/e2e/accounts.spec.ts` does the same for Decks, My games and the builder; `scripts/ui-audit.mjs` takes the same measurements on any URL of a running dev server. A changed page runs the script and a new page joins the spec. The rules and the regions allowed to scroll are in `docs/web.md`, End-to-end.

## Tasks

Each is 45 to 60 minutes; "After" names what must be done first and what runs alongside.

| # | Task | Files | After | Done when |
|---|---|---|---|---|
| U1 | Tokens and shell | `index.css`, `AppShell.tsx`, `ui/*`, `components/{Pill,EmptyState,Kbd}`, `hooks/useTheme.ts`, `tokens.test.ts` | none | contrast test passes both themes; theme follows the system, persists; nav Play, Create, My games |
| U2 | Fit engine | `table/{fit,group}.ts`, tests | none | 150-case table: fits or scrolls, never under the floor; grouping keeps state apart |
| U3 | Table grid | `GameTable.tsx`, `TableGrid.tsx`, `TopBar.tsx`, `SideTabs.tsx`, `layout.ts`, `e2e/fit.spec.ts` | U1, U2 | no page scroll at 2, 4, 6 seats, six sizes; hand and pass inside the viewport |
| U4 | Pods, rows, hand | `SeatPod.tsx`, `Battlefield.tsx`, `HandDock.tsx`, `CardStack.tsx`, `cardSize.ts` | U3 | 40 permanents a seat at 1280x720 without zone scroll; no text under 11 px; `perf.test.tsx` green |
| U5 | Controls dock | `ControlsDock.tsx`, `PassControls.tsx`, `StopLadder.tsx`, `TopBarMenu.tsx` | U3; with U4 | pass 44 px bottom right; at most 5 visible controls; key tests green |
| U6 | Turn feedback | `table/turn/*`, `SharedStrip.tsx` | U3; with U4 | the Turn and focus table; feed leak test; CSS test |
| U7 | Server model | `registry.rs`, `seats.rs`, `lobby.rs`, a migration, `docs/server.md` | none | visibility, open seats, claim and code, tested |
| U8 | Server endpoints | `api.rs`, `server.rs`, `rest.rs`, bindings | U7 | lobbies, join, leave, release; privacy and throttle tests |
| U9 | Browser page | `features/browser/*`, `routes/index.tsx`, `net/api.ts` | U1, U8 | the browser section, tested and photographed |
| U10 | Create and lobby | `features/create/*`, `game/{Lobby,Invites}.tsx`, `routes/{new,join.$code}.tsx` | U9 | visibility, code and link, leave and close; `pnpm e2e` follows `/new` |
| U11 | Page polish | `GameOver.tsx`, `MyGamesPage.tsx`, auth pages, `PageShell` | U1; with U3 | one width scale; designed states; rematch and way back |
| U12 | Gate, cleanup | `scripts/ui-audit.mjs`, `e2e/layout.spec.ts`, `e2e/layoutAudit.ts`, `docs/web.md` | all | six sizes, both themes pass in Chromium |

## Sources

- S1 [official] NN/g heuristics: https://www.nngroup.com/articles/ten-usability-heuristics/
- S2 [official] NN/g visual hierarchy: https://www.nngroup.com/articles/visual-hierarchy-ux-definition/
- S3 [official] NN/g proximity: https://www.nngroup.com/articles/gestalt-proximity/
- S4 [official] NN/g Fitts's law: https://www.nngroup.com/articles/fitts-law/
- S5 [official] NN/g touch targets, 1 cm: https://www.nngroup.com/articles/touch-target-size/
- S6 [official] NN/g animation duration: https://www.nngroup.com/articles/animation-duration/
- S7 [official] NN/g response times: https://www.nngroup.com/articles/response-times-3-important-limits/
- S8 [official] NN/g progressive disclosure: https://www.nngroup.com/articles/progressive-disclosure/
- S9 [official] NN/g empty states, skeletons: https://www.nngroup.com/articles/empty-state-interface-design/ · https://www.nngroup.com/articles/skeleton-screens/
- S10 [official] NN/g dark mode, offer both: https://www.nngroup.com/articles/dark-mode/
- S11 [official] NN/g confirmation dialogs: https://www.nngroup.com/articles/confirmation-dialog/
- S12 [official] WCAG 2.5.8 and 2.5.5 target size: https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html · https://www.w3.org/WAI/WCAG22/Understanding/target-size-enhanced.html
- S13 [official] WCAG 1.4.3 contrast: https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
- S14 [official] WCAG 1.4.11 non-text contrast: https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
- S15 [official] WCAG 2.4.7 and 2.4.11 focus: https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html · https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html
- S16 [official] WCAG 2.3.3, MDN reduced motion: https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html · https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion
- S17 [official] Tailwind theme variables: https://tailwindcss.com/docs/theme
- S18 [official] shadcn theming: https://ui.shadcn.com/docs/theming
- S19 [official] Radix Colors scale: https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale
- S20 [official] Material color roles, motion tokens, and the dark theme page (a search summary only): https://raw.githubusercontent.com/material-components/material-web/main/docs/theming/color.md · https://raw.githubusercontent.com/material-components/material-components-android/master/docs/theming/Motion.md · https://m2.material.io/design/color/dark-theme
- S21 [official] MDN container queries, `clamp()`: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_containment/Container_queries · https://developer.mozilla.org/en-US/docs/Web/CSS/clamp
- S22 [official] web.dev viewport units: https://web.dev/blog/viewport-units
- S23 [official] Steamworks lobby types: https://partner.steamgames.com/doc/api/ISteamMatchmaking
- S24 [community] Arena in-match screen: https://www.hipstersofthecoast.com/2018/03/how-to-play-mtg-arena-the-in-match-screen/
- S25 [community] Hearthstone turn: https://hearthstone.wiki.gg/wiki/Turn
- S26 [community] Marvel Snap screen: https://pixeltwelve.com/guides/marvel-snap-beginner-guide-cards-locations-energy-snap
- S27 [community] Legends of Runeterra UI: https://www.ludo.guide/guide/legends-of-runeterra/understanding-the-ui
- S28 [community] Slay the Spire UI: https://www.ludo.guide/guide/slay-the-spire/getting-started/understanding-the-user-interface
- S29 [official] Unity Lobby glossary (a search summary only): https://docs.unity.com/en-us/lobby/glossary
- S30 [official] WCAG 1.4.1 use of colour: https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html
- S31 [official] Okabe and Ito, Color Universal Design: https://jfly.uni-koeln.de/color/
- S32 [official] Machado, Oliveira and Fernandes 2009, colour vision deficiency simulation: https://www.inf.ufrgs.br/~oliveira/pubs_files/CVD_Simulation/CVD_Simulation.html
- S33 [official] Sharma, Wu and Dalal, the CIEDE2000 colour difference with test data: https://hajim.rochester.edu/ece/sites/gsharma/ciede2000/ciede2000noteCRNA.pdf
