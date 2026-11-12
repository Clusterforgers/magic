# UI design

The rules the web client is built to: principles, design system, a fit-to-viewport table, turn feedback, legible play, the manual panel, the game browser, how priority and pace stay private and the definition of done. Implementation is in [web.md](web.md).

## Principles

| Principle | Applied here |
|---|---|
| Visibility of system status | Turn, phase, connection and automatic passes are always on screen |
| Consistency, real-world words | One token set, button hierarchy and row pattern; Magic's words, Arena's pass labels |
| Control and freedom | Concede, Stop passing, Not ready and Leave are one reversible step |
| Error prevention | Confirm only what cannot be taken back: Concede, and in the lobby Close game, Leave game and Remove; a disabled Join says why; illegal cards dim |
| Minimalism, disclosure | One primary action per region; the rest one menu level down, never two; key hints on buttons |
| Visual hierarchy | Hand, pass button and life are biggest; state colour only for turn, phase, priority, legal |
| Proximity | A seat pod groups name, life, zones; 8 px inside a group, 16 between |
| Fitts's law | Pass is the largest target, bottom right; frequent controls 32 px or more, 44 on touch |
| Contrast and focus | 4.5:1 text, 3:1 edges and rings, focus never covered, colour never alone |
| Motion and timing | 100 to 300 ms, ease-out in; nothing delays an answer; a working state past 1 s |

## Design system

Tokens are CSS variables in `index.css`, mapped by `@theme inline`; shadcn's names stay. `tokens.test.ts` computes the contrast of every text, edge and ring pair in both themes and holds the scales below; `tokenUse.test.ts` keeps hard-coded colours, sizes and off-scale text out of `components/` and the pages beside the table. A state tint (`bg-success/12`) sits on the page, panels and menus, not on the table.

| Role | Use |
|---|---|
| `background`, `surface`, `table`, `raised` | page; panels and rows; boards and sunken zones; menus and dialogs |
| `foreground`, `muted-foreground` | text (15:1 or more); secondary text (5.7:1) |
| `border`, `border-strong`, `ring` | separators; field and control edges (3.7:1); focus, 2 px plus 2 px offset, set once for `:focus-visible` |
| `primary`, `on-primary` | main action, current page, selection |
| `turn-you`, `on-turn-you`, `turn-you-edge`, `turn-them` | your-turn pill and banner (gold); pod edge and ring; opponent turn marker, pod and tile edge |
| `phase-<id>`, `-soft`, `on-phase-<id>` | accent, tint and text of the four phases ([Phase themes](#phase-themes)); the live `phase`, `phase-soft`, `on-phase` follow the table root's `data-phase` |
| `success`, `warning`, `destructive` | ready; urgent clock; lethal and Concede |
| `selected`, `on-selected` | a chosen card or target: crimson, at least 12° of hue from `destructive` (damage, loss) and apart from the gold of attackers, with a check badge so colour is never the only sign |
| `mana-<colour>`, `on-mana-<colour>` | pip fill and ink, the same in both themes, 5.6:1 or more |

| Token | Scale |
|---|---|
| Spacing | 4 px base: 4, 8, 12, 16, 24, 32, 48, 64 |
| Type | Geist Variable. `text-label` 11/16 card labels only, `text-xs` 12/16, `text-sm` 14/20 UI, `text-base` 16/24, `text-lg` 18/28, `text-2xl` 24/32, `text-life` 36/40 with tabular digits, `text-display` 32 to 48 by width; weights 400, 500, 600. Card text on the table is 12 px or more |
| Radius | `rounded-md` card 6, `rounded-lg` control 8, `rounded-xl` panel 12, `rounded-full` pill |
| Elevation | e1 `0 1px 2px` 8%; e2 `0 4px 12px` 12% menus; e3 `0 12px 32px` 20% dialogs; dark lifts the `raised` surface instead |
| Motion | `duration-fast` 100 ms (hover, toggle), `base` 200 (menu, row), `slow` 300 (sheet, banner); table effects stay at most 600 ms (`fx.css`); `ease-out` in, `ease-in` out; linear for clocks only; under `reduce` opacity only |
| Icons | lucide-react: 16 px in dense rows, 20 in controls, 24 in menus; always a label or `aria-label`; mana symbols stay in `ManaPips` |
| Breakpoints | XS under 640, S 640 to 1023, M 1024 to 1279, L 1280 to 1599, XL 1600 up; container queries and `clamp()` inside the table |
| Page width | `narrow` 28rem (a form), `page` 48rem (reading, lists of cards), `wide` 72rem (a table of rows), `wider` 90rem (the builder) |

- Button heights: 24, 32, 36 and 44 px (44 under `pointer: coarse`). A component draws no focus style of its own.
- States are designed for every control: default, hover, active, focus-visible, `aria-disabled` (a control that must keep focus when it cannot act), loading, invalid with text, empty, error and offline.

## Table layout

The table is a `100dvh` grid that never scrolls the page. The opponent is on top, the hand on the bottom edge and the pass buttons bottom right, as in Arena.

```
+-----------------------------------------------+------+
| top bar: turn badge, phase pips, menu          |      |
+-----------------------------------------------+ side |
| opponents: pods and boards                     | tabs |
+-----------------------------------------------+ Cmds |
| shared: stack chips, ticker, prompt line       | Log  |
+-----------------------------------------------+ Chat |
| own: pod, lanes: creatures; lands, other      | Stack|
+-----------------------------------------------+      |
| hand dock                    | controls dock   |      |
+-----------------------------------------------+------+
```

| Region | Content |
|---|---|
| top bar | home, turn badge, step ladder (from 1024 px), menu (layout, theme, card images, hand order, Concede); 44 px |
| opponents, own | a pod and board per seat; the rows share the room by what their boards hold, not by a fixed ratio |
| shared | stack chips, ticker, prompt line. A long prompt grows up into a sheet of at most 40 dvh into room the opponents' area keeps free, so it covers no card of a battlefield and never your board (its cards are what a prompt asks you to click), the hand or the controls |
| hand dock | whole cards in one row, overlapping in rows only when they do not fit; hover or focus lifts a card and opens the preview |
| controls dock | the server's passing line, the stops (one row with a Mine or Theirs switch and Full Control from 1600 px, else a Stops popover), pass buttons; the primary button is bottom right, 44 px or more |
| side | Commands, Log, Chat, Stack tabs with unread counts. L, XL a 280 to 320 px column; M a 44 px rail opening a panel over the table; S, XS a sheet from the top bar |

- Lanes: a board draws its permanents in three lanes: lands, creatures (any creature) and other permanents. Creatures face the middle of the table, lands sit left and other permanents right, an opponent's board mirrored top to bottom. Lanes never overlap, an empty lane takes no room, and each lane has a faint ground (`--lane`). The command zone, graveyard and exile are the pod's corner.
- Fit: `fitLanes` (pure) takes the largest card width at which some shape of bands fits every lane (one band, creatures over lands and others, or a band each; ties go to fewer bands). Cards shrink as counts grow, and at the 44 px floor the zone scrolls inside, creatures first, with an edge fade and a "+N" count. Creatures are the largest cards on a board.
- Stacks: permanents equal in everything a card shows merge into one stack with a ×N badge. Lands and tokens group at once, other cards when a zone would fall under 64 px. A stack opens in place on click or Enter, and stays open while a prompt needs each member.
- Teams: a pod and a tile carry the seat's team as a chip ("Team 1") in the team's colour (`team-1` to `team-4`, then repeating; the label always tells them apart). In Two-Headed Giant the shared life total is large on the team's first seat; the teammate's pod shows it small and muted with "shared", and both name it "Team life, shared with Ann".
- Seats: 2 draws one wide opponent board beside a 168 px pod column; 3 and 4 draw two or three columns of one-line pods; 5 or more, or any column under 280 px, draws a tile strip and one focused board that follows the active seat (Focus mode); the menu overrides the mode. Opponent boards shrink before yours.
- S (tablet): the same grid, a side sheet, 44 px targets. XS (phone, portrait): top bar, a pod strip (active first), the focused opponent board, your board, the shared line, the hand dock; a tap on a pod focuses it.

## Turn and focus

Whose turn it is stays on screen as a token. Cues decorate the batch that arrived: none waits, delays an answer or adds a cadence, and their text comes from public events, identical for every seat (see [Priority, stops and information leaks](#priority-stops-and-information-leaks)).

| Cue | When | Shows | Reduced motion |
|---|---|---|---|
| Turn badge | always, in the top bar | the phase's icon and name, then "Your turn · 5 · Precombat main" (accent fill) or "Bo's turn · 5 · Declare blockers" (tint, accent edge), or "Bo left · 5 · Declare blockers" while the turn goes on without its player (`TurnView.active_left`, no seat lit) | unchanged |
| Turn banner | turn number or active seat changes | a band in the free room of the top bar, "Your turn" or "Bo's turn" and the turn; 250 ms in, 600 held, 150 out; no pointer | fades of 100 ms, held 1.5 s |
| Active seat | during its turn | a 3 px edge inside the pod or tile (turn-you-edge or turn-them), a faint tint on a tile; a dot after the name of a priority holder who is not active; your pod adds `your-turn-cue`, pulsing twice (5.6 s) and resting; in Focus mode the strip leads with the active seat and the large board follows it with a 150 ms cross-fade | static ring, no fade |
| Ticker | every batch | one line: the newest non-minor log line, plus "Skipped Upkeep, Draw" when steps passed with no window for you; three lines, 3 s each | static |
| Ladder | a step changes | current step lit in the phase's accent, passed steps filled, a skipped step dashed for 1 s | no sweep |

- Only your own skips are explained ("You had no plays"). Another seat's pass never says if it was automatic, how long it took or who is deciding.
- The feed is pure over the batch, and a leak test requires identical feeds for games that differ in one hidden card ([web.md](web.md), Turn cues).
- A polite live region announces the banner. The tab title carries "Your turn · ".

### Phase themes

Four themes mark the phase at a glance; a step maps to one by the step alone, which every seat sees (`turn/phase.ts`). The hue changes with the phase, and an icon and a name sit beside it.

| Phase | Steps | Icon | Hue |
|---|---|---|---|
| Beginning | untap, upkeep, draw | sunrise | blue |
| Main | precombat and postcombat main | wand | bluish green |
| Combat | beginning of combat to end of combat, first-strike damage included | swords | vermillion |
| Ending | end step, cleanup | moon | reddish purple |

The table root carries `data-phase`; `phase.css` points `phase`, `phase-soft` and `on-phase` at that set, neutral outside the turns. The theme reaches the top bar's 3 px rule, the turn badge (accent fill on your turn, tint with an accent edge otherwise), the ladder (current step in the accent, an icon per phase run, the current phase named), the shared strip's 2 px edge and the table's main area as a wash of at most 6 %.

- Never colour alone: the badge and the ladder carry the icon and the name; whose turn it is stays in words and in fill against tint.
- The hues follow Okabe and Ito; none is yellow or orange, which is the gold of your turn.
- Contrast: text on the accent and on the tint 4.5:1; the accent against every surface, its tint and the washed table 3:1; text on the washed table 4.5:1. Any two accents, and an accent and the gold, differ by CIEDE2000 12 or more under full vision, protanopia, deuteranopia and tritanopia (`phaseTokens.test.ts`).
- A change of theme fades in 200 ms and is immediate under reduced motion. Nothing waits for a theme.

## Playing is legible

Every play reads on the table without the log: what was played, from where, at what, and what it did. A spell travels from where it was to the stack, stands large for a moment, points at its targets and travels on to where it ends. Like the effects of [web.md](web.md) (Table feedback), the moves decorate the batch that arrived: nothing waits for them, and they come from public events only, so every seat sees the same.

### Moments

A move is a picture of the card (a card back for a card the viewer may not identify) flying on an `aria-hidden` overlay without a pointer. A place is a card on the table (`data-object-id`) or a zone anchor (`data-fx-anchor` on the hand dock and the stack, a pod's or tile's `data-zone` chip). A mark is a still attribute on a card (`data-fx`).

| Moment | Comes from | Moves | Marks |
|---|---|---|---|
| Cast | `SpellCast`; the zone before the batch, else the caster's hand | source to stack | spotlight, then target arrows |
| Activate | `AbilityActivated`, `AbilityPutOnStack` | an ability chip from the source to the stack | `activated` on the source |
| Trigger | `AbilityTriggered`, `AbilityPutOnStack` with `cause` | an ability chip from the source to the stack | `triggered` on the source |
| Target | `ObjectView.targets`, else `choices.targets` of the event | none | `targeted` on each target while the item is on the stack; arrows |
| Resolve | `ResolutionEnded` `Resolved` | stack to the battlefield or the owner's graveyard | `resolved` |
| Counter | `ResolutionEnded` `Countered` | stack to graveyard | `countered` on the chip before it leaves |
| Fizzle | `ResolutionEnded` `Fizzled` | stack to graveyard | `fizzled` |
| Enter | `Entered`, a battlefield object new to the batch | from its last zone unless that was the stack | `entered` |
| Die, exile, bounce | `CreatureDied`, `ZoneChanged` (`cause`) | the ghost, then battlefield to graveyard, exile or hand | `died`, `exiled`, `bounced`; an arrow from the resolving object |
| Draw | `CardDrawn`, public or hidden | library to hand, a card back for another seat | `drawn` on the viewer's new hand card |
| Play land | `LandPlayed` | hand to battlefield | `entered` |
| Other moves | `ZoneChanged` (mill, discard, return) | old zone to new zone | none |

- The resolving object at an event is the last `ResolutionBegan` without its `ResolutionEnded`; what it destroys, exiles, bounces or counters gets a short arrow from it. An object cast in the same batch has no box to start from and gives no arrow.
- A spell cast and resolved in one batch plays both legs with the spotlight between, so a spell never happens unseen.
- A batch plays its moves in event order, `--fx-stagger-ms` apart, at most `MAX_MOVES` (8); the rest are marks only. A newer batch finishes the moves of an older one at once. A cast object keeps its id from hand to stack to battlefield.

### Motion tokens

| Token | ms | Use |
|---|---|---|
| `--fx-travel-ms` | 380 | one leg of a move; the picture takes the size of where it lands |
| `--fx-spotlight-ms` | 900 | the large spell after a cast by another seat or one that passes through the stack in one batch, 150 ms fades; the viewer's own cast stands 0.6 of it |
| `--fx-stagger-ms` | 120 | between the moves of one batch |
| `--fx-arrow-ms` | 250 | an arrow draws in |
| `--fx-mark-ms` | 600 | `activated`, `triggered`, `resolved`, `countered`, `fizzled`, `exiled`, `bounced`, `drawn` |

A whole sequence stays under 1,700 ms. `--fx-slow` stretches all of them. Under `prefers-reduced-motion: reduce` nothing travels or scales: source and destination take their marks as still outlines at once, the spotlight appears and goes without motion, marks appear and go as badges, arrows appear without drawing in.

### The stack

- The shared strip's stack chips stay; the top chip gains a targets line ("→ Hill Giant, Bo", at most 3 names, then "+N more") and a dot in its controller's colour. Every chip's label for a screen reader ends with ", targeting Hill Giant and Bo".
- A crowded stack stays one row: a run of the same ability of one source folds into one chip with "×N", and chips past the strip's width give way to one "+N more" button that opens the Stack tab. A seat attacking with more than five creatures draws one heavier line per target with a pill "N attackers → Cy" ([web.md](web.md), Combat overlay).
- Arrows run from the top chip to each target (a card, a player's pod, or the chip of another stack item) while it is on the stack, at most 6; hover or focus on any chip draws that item's arrows instead. They share the combat arrow layer, in `--primary` for the viewer's items and `--destructive` for another seat's.
- A target that is gone gets no arrow and drops out of the line; an item with none left says "no legal target" on a dashed chip.
- The spotlight is the card at 1.6 times the hand card's height (168 px to 40 % of the window), centred over the shared strip with a caption: who casts, the name and the targets line. It takes no pointer, shows a card back for a card the viewer may not identify, is replaced by a newer batch with moves, and never stays, so the layout gate measures the table without it. The viewer's own cast stands 0.6 of the time, in lower contrast, captioned "You cast".
- A mark is a badge with an icon and a short word (`FxMark`) on the card or chip it names, its edge and icon in a token colour (primary: activated, bounced; warning: triggered; success: resolved, drawn; destructive: countered; border-strong: fizzled; turn-them: exiled) and its words in `--foreground` on `--card`; `markTokens.test.ts` holds the contrast.
- The Stack tab shows the top item as a large picture with its text and one row per target (a gone one struck out), the rest as rows with their target line.

### The hand

- While the viewer holds priority, a playable card (`castable`, `playable_lands`, `faces`, `suspendable`, `foretellable`, or an ability in `abilities`) is legal: ring and lifted 4 px. Every other hand card is dimmed and carries a reason. Without priority the hand is unmarked. An opponent's card is never marked and a spectator has no marks.
- The reason is a short label on the card's foot on hover and focus, the spoken name and the line under the name in the preview, which a tap pins so touch reaches it.

| Reason | Label | Line |
|---|---|---|
| `Timing` | Sorcery speed | Only in your main phase with an empty stack |
| `LandPlayed` | Land played | You have played a land this turn |
| `Mana` | Needs mana | Not enough mana |
| `Targets` | No targets | Nothing it can target |
| `Cost` | Can't pay | A cost cannot be paid |
| `Restricted` | Not allowed | An effect forbids it now |
| (unknown) | Not now | Not playable now |

- The reason is the server's (`PriorityOptions.unplayable`: one entry for each card of the holder's hand and command zone that is not offered; only the holder sees options, so nothing leaks). For a card it does not name, the web reads `Timing` and `LandPlayed` off public state and `Mana` when the mana value exceeds the untapped mana sources plus the pool; anything else is "Not now".
- With the server's detail (`why`) the preview line is one plain sentence from `whyWords.ts`: what the target must be and where ("Double Major needs a target: a creature spell you control on the stack."), the cost against the mana at hand and the colour nothing makes, the printed timing sentence, the permanent that forbids it, the split second spell, summoning sickness, a tapped source, a spent limit or the additional cost. A target on the stack adds a hint: "Cast a creature first and keep priority (Full Control) to copy it." for the viewer's own spells (the card's verb), "Wait until one is cast, then counter it while it is on the stack." for any spell.
- A permanent of the viewer whose activated abilities are all in `PriorityOptions.inactive` carries the same sentence in its preview and spoken name; the card's chooser lists those abilities disabled, the sentence under each.
- A play the server refuses (`Error.refusal`) and a play taken back (`AnnouncementRewound.why`) say the same sentence in the notice.
- A permanent the viewer controls, or a card of its hand, graveyard, exile or command zone, with an ability in `PriorityOptions.abilities` is legal as a hand card is. A click activates it; a card with several plays asks which in a small chooser at the card, in the look of the manual menu. A mana ability is never a play. A card legal for its ability alone (cycling) says why it cannot be cast.
- A graveyard or exile chip or picture with a card in it that is legal to play or use wears the ring of a legal card and a play badge, and its name says how many ("2 can be played or used"); the command zone does not, its commanders are cast from the hand dock.

## Attachments

An Aura, Equipment or Fortification is read at its host, as Arena and MTGO draw it: the attachment lies behind the host with its name line showing above it, the host carries a mark for what each attachment gives, and a move between hosts is drawn as an arrow and said in the log.

| Part | Shows |
|---|---|
| Tuck | every attachment on a permanent lies behind its host, whoever controls it, each showing its top `tuck` px (the name line) above the one in front; the host stands at the foot of its box; an attachment takes no box of its own |
| Fit | a lane's line is `height + depth × tuck` tall, `depth` the most attachments tucked at one host of the lane; a lands lane with a tuck is not fanned |
| Order | Auras, then Equipment, then Fortifications, each in id order; the first lies deepest |
| Grant marks | one mark per attachment on the host's foot stack, in tuck order, from `ObjectView.gives` (power and toughness, a set value, rules, keywords; an Aura on a player names the rules it gives that player, such as "Can't cast" or "Skips draw"): whole words while the face is wide enough, else short words, else two lines, else the first items and "…", never a cut word. An attachment that gives nothing is marked by its name; equal marks of one controller merge with "×2"; past the stack's room the rest are one neutral summed mark. A compact or turned face shows the count and one icon. Tone: `success` for a raise, `destructive` for a lowering or restriction, neutral otherwise |
| Preview | the host's preview lists each attachment (its name, its controller when it is not the host's, and what it gives); the attachment's own preview names its host or player and what it gives it |
| Foreign | an attachment another seat controls carries a 2 px dashed edge in `border-strong` around its tucked card and "Bo's" on its name line (the edge alone below 76 px), and "Bo's" before its name in the host's spoken name and preview |
| On a player | an Aura or Curse on a player stays in its controller's other lane with "On Bo"; the enchanted player's pod and tile show a chip for each name and controller, "×3" when several share one |
| Unattached | an Equipment or Fortification on no host stays in its controller's other lane with a dashed "Unattached" chip at its foot |
| Move | `Attached` draws an arrow from the old host to the new with the combat overlay's `connect` (450 ms in, standing 1 s; still under reduced motion). The log and ticker say "Bonesplitter moves from Hill Giant to Llanowar Elves", "Pacifism enchants Bo" or "Bonesplitter becomes unattached"; an attachment that enters attached draws no arrow |

- `gives` is the engine's account of each attachment's continuous effects on its host as the layers apply them now: `{ power, toughness, set, keywords, rules }`, left out while the attachment gives nothing or is unattached. The view cannot work it out: `abilities` carries mana abilities only and a card's words do not say what a condition or a layer leaves of them.
- Many: the foot stack shows the first marks and sums the rest into one; the preview always lists them all. On a phone a tuck shows the attachment's colour edge only below 76 px cards; the marks are the count and an icon, and the long-press preview carries the list.

## Manual commands panel

The seat does by hand what the engine does not run from one persistent panel at the right of the table, laid out as Table Commander lays out its tools: the seat's own numbers on top with quick amounts, then the library, then what it makes, then the card it picked. The card menu stays as the shortcut for one card.

| Size | Where |
|---|---|
| L, XL | the first tab of the side column, "Commands", the tab on show when the table opens; the column keeps its width, so the card sizes the layout gate protects do not change |
| M | the first icon of the rail, opening a 320 px panel over the boards, from under the top bar down to the hand and controls row; the same icon closes it |
| S, XS | the first tab of the side sheet, a non-modal sheet that hangs from the top bar (the top bar's button reads "Commands, log, chat and stack"); a press on the tab on show keeps it open, Close shuts it |

| Group | Commands, in order |
|---|---|
| To do by hand | read-only: the seat's by-hand lines that came due (`ByHandDue`), each as its card and its quoted words, and Done; only while one is due |
| You | the Life counter and "More life options"; Poison; Other counters |
| Library | Draw; Look at the top (scry, surveil, reorder); Search for a card; Mill; Exile from the top; Put on the bottom; Shuffle; Reveal the top card |
| Make | Tokens (the common list, a custom token); Mana; Roll a die / Flip a coin |
| Card | the picked card's commands, as its card menu lists them: Move to, Counters, Tap, Attach, Deal damage, Pump, Reveal, Note; then Remove summoning sickness, Sacrifice and Give control |

- A group is a heading and its rows; a row is a button with icon, name, one line of what it does and a chevron. A press opens the row in place (one at a time): an amount with presets, a preview line in the seat's words ("Draw 2: library 52 → 50, hand 4 → 6") and one Apply button that names the change. Nothing is sent before Apply. Commands that lose the library's order say "This cannot be undone: the library's order is lost." above Apply.
- After Apply the row says "Sending: Lose 5 life…" until the server answers, then "Done: Lose 5 life." or "Refused: <the engine's reason>." in the row's own feedback (`role="status"`, red on a refusal), and keeps its choices on a refusal; the table notice is not used. A second Apply waits for the answer. The log says the action as for the card menu.
- Life is an open counter at the top of You: name and total in large figures over -5, -1, +1 and +5. Presses add to a pending change shown as a running total ("Not sent yet: -5 (40 → 35 life)") and are sent as one `Life` action after 600 ms of quiet, so a run is one log line; one change is in flight at a time and a pending change is sent, not lost, when the tab changes. Undo drops a pending change; a taken change can be undone for 8 s while life is where it left it, as one opposite action.
- Look at the top: Apply sends `LookTop`; while the last look is open the row lists the cards (`PlayerView.known_top`) in three lists (top, bottom, graveyard) and Apply sends one `ArrangeTop`. The seat's own log names the cards; other seats read only counts.
- Search for a card: Apply sends `Search`, which shows the seat its whole library as a grid sorted by name, so its order is not on show. A press chooses a card, Apply sends `Tutor` and the library is shuffled; "Shuffle without a card" ends the search.
- Card: "Pick a card" arms the next press on the table: a banner "Pick a card for its commands. Escape cancels.", every card ringed, and any press chooses a card without its usual action; a covering rail or sheet steps aside meanwhile. A command the card cannot take is inert with its reason. The card menu's "In the panel" chooses without arming.
- Roll a die / Flip a coin: Coin, d6, d20 and d N (2 to 100 faces) sending one `Roll`; the engine draws the result from the game's random stream, and every seat reads it in the log.
- Manual actions are allowed at any time, whoever holds priority; the panel is inert with the reason once the seat has lost or the game is over. A spectator has no Commands tab.
- Keyboard: the panel is a `region` named "Commands"; Tab walks the rows, Enter or Space opens one, the amount is a `spinbutton` (arrows ±1, Page Up and Down ±5), Enter applies, Escape closes. The Life buttons are a `group` with the same keys. The preview is polite. Targets are 44 px on touch and 32 px with a mouse.
- The card menu keeps the commands about its card; life, draws, library cards, tokens and mana are the panel's, and the menu's last entry, "In the panel", opens the panel on that card.
- Reveal and Note show on the table to every seat: a seat's revealed hand cards and top card are an eye chip with their count beside its hand and library chips, opening the cards face up; a revealed card carries a "Revealed" mark and a note its first words on the card's face.

## Game browser and lobby

`/` is Play: open games, Create game and Join with a code. Create is `/new`, an invite opens `/join/$code`, My games stays; a signed-in user also gets "Continue" for a running game. Visibility follows Steamworks and Unity lobbies: private lobbies are hidden from queries and joined by code.

| Column | Content |
|---|---|
| Host | account or guest name |
| Format | Normal or Commander |
| Seats | "2 of 4" with dots; a full row dims and says "Full" |
| Age | "3 min ago" in a `<time>`, refreshed every 30 s |
| Action | Join, the one primary button per row |

- States: skeleton rows only after 300 ms; empty ("No open games. Create one, or join with a code.") with both actions; error alert with Retry, stale rows kept and dimmed.
- Updates: poll `GET /api/lobbies` every 5 s while the tab is visible, with `ETag`, and a Refresh button. A row that fills before the click says "Someone took that seat" and the list reloads.
- Join: a guest gets a name dialog and nothing about a deck, since the player chooses it in the lobby; a signed-in user joins in one click and the seat binds to the account. A row and the code field join the same way. A code no open game has reads "No open game with that code."
- Create: seats from 2 to 256 (default 4 in Commander, 2 in Normal), your name and deck, Visibility with its consequence beside it ("Listed in the game browser" or "Only people with your link or code"; default Listed), an Advanced disclosure for a link per seat. A Game mode choice under Format: Free-for-all (default), Two-Headed Giant (4 seats, the count is read only) or Teams (an even count from 4); a count the mode cannot play is named under the choice and blocks Create. The lobby shows invite link, code and a visibility toggle with Close for the host, Leave for others.
- Lobby teams: the mode follows the format line ("Commander game · 4 seats · Two-Headed Giant"); each seat row has a left edge in its team's colour and the team label, a select "Team of Ann" for the host. A change shows at once, takes every Ready back and is said in a polite line; teams the server would refuse are not sent, and a refusal puts the old teams back with the reason.
- Game over for teams: "Team 1 wins — Ann & Bo"; every seat of the winning team gets the burst and "Your team won", the others the banner naming the team and its players; each seat row carries its team chip.
- Privacy ([server.md](server.md)): an invite game is never listed; an unknown id, an invite game without its code and a wrong code give one 404; the list never carries a token or code.
- Accessibility: a real `<table>` with a caption from M up, stacked rows below; the button name is "Join Bo's game, 2 of 4 seats"; a polite "3 open games" line; polling never moves focus; a removed focused row sends it to the next Join.

## Priority, stops and information leaks

The pace of a game must not tell a seat what another holds. As in Arena, when a seat is asked follows from the table (step, stack, stops, mode), never from its hand; only the default windows read the hand (`can_act`), and the dwell hides that. The wire rules are in [protocol.md](protocol.md) (Pace of play, Stops and Full Control) and the server's in [server.md](server.md).

- Windows: a seat is asked only at the default windows (its own main phases, declare attackers and blockers, an object of another seat on the stack) and at its stops; elsewhere it is passed for at once. Its own upkeep and end step are no windows unless it sets a stop.
- `can_act`: a play is a land to drop, a spell to cast, an ability that is not a mana ability and can be paid, or a special action. A mana ability never is one, a filter land that costs mana included; a Clue, an equip or a scry land is.
- Dwell: at a default window a seat that can act makes the table wait for a human; a seat that cannot waits 250 to 700 ms (`MTG_DWELL_MS`, 0 = off) and passes as a step and batch of its own, so the other seats see the same stream. Yields, Full Control and its own empty main phase are exempt. Only the wall clock changes, so the log and the hash do not.
- Full Control: the seat is asked at every window and never passed or yielded for; the response clock never reads `can_act`. A stop is asked whether or not the seat can act.
- The client never paces steps: the fx layer must not wait for a cadence.
- Payments: a request to pay is a question with a clock for a seat that can pay or whose payment needs a hand card; a seat out of Full Control that cannot pay from what is in sight is not asked and the table answers No, which shows only what the battlefield shows; in Full Control it is asked and may only say No (`pay_leak.rs`).
- Single answers: a question with one legal answer, all of it in sight, is answered for a seat out of Full Control; one whose answer could come from a hidden zone is always asked. A seat may let the table order its triggers by timestamp and assign combat damage lethal in order (`AutoChoices`, off by default); both read only public objects.
- Smart stops (`AutoChoices.smart_stops`, off by default): a step becomes a window where a permanent of the seat has an ability its timing allows. It is chosen from the battlefield without costs, never the hand; a seat that cannot pay there waits a dwell, so the pace shows only the board and the setting.
- Timer bank: the bank and the spend of an extension are public (`PlayerView.bank`); they follow from how long a seat takes to answer, never from the cards. The clock fields (`deadline_ms`, `show_after_ms`) read the decision kind and the seat alone.
- Turn time: each seat's time on its own turns is public (`PlayerView.turn_time_ms`); it follows from how long a seat's turns take, never from the cards.
- The log is uniform: `PriorityGranted` and `PriorityPassed` are public events for every window, skipped or not.
- What remains visible by design: how long a seat that can act takes, a pass at the response-clock deadline, and the choices of stops, Full Control and yields.
- Never add a hover or selection relay, a thinking cue or the kind of decision owed to the views. Clocks, previews, `Welcome` and the `Stops`, `FullControl` and `Yielding` notices reach only their seat.
- `pace_leak.rs` (`crates/mtg-server/tests/it/`) requires two seeded games that differ by one hidden card to send the other seat the same messages in the same batches, through combat windows and spells and triggers on the stack; with the dwell off it finds the difference. `MTG_AUTO_PASS=0` turns auto-pass off for every seat.
- Keys follow Arena: Space passes once, Enter asks the end-of-turn yield, Ctrl is Full Control for the next window and Ctrl+Shift holds it. The primary button relabels with the state ("Next: <step>", "Resolve <spell>", "End turn", "Continue"). A yield is ended by a new object of another seat; the strict pass of Arena (Shift+Enter) is not built.

## Definition of done

- Fits: the table never scrolls the page at the six sizes (1920x1080, 1440x900, 1366x768, 1280x720, 1024x768, 390x844); other pages never scroll sideways; nothing is cut at 390 px; no text is under 12 px; every primary action is in the window and not covered.
- Contrast: token pairs pass in both themes; focus is visible and uncovered.
- Targets: 24 px floor, 32 px on desktop, 44 px under `pointer: coarse`.
- Keyboard: tab order follows reading order; every action works without a mouse; no trap.
- States: empty, loading, error and offline are designed.
- Motion: inside `no-preference`, a CSS test per file, nothing delays an answer.
- Evidence: screenshots at the six sizes in both themes, looked at, not committed; tests; `docs/web.md` updated.
- Gate: `web/e2e/layout.spec.ts` holds every page that needs no account, and the dev routes for those that do, to Fits at the six sizes in both themes in Chromium, and `web/e2e/accounts.spec.ts` does the same for Decks, My games and the builder; `scripts/ui-audit.mjs` measures any URL of a running dev server. A changed page runs the script and a new page joins the spec ([web.md](web.md), End-to-end).
