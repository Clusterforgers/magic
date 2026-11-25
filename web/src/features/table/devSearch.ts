import type { PlayerId, Step } from '@/protocol/generated'
import { parseStopSpec } from './dev/stopSpec'
import type { DevPanel } from './manual/panel/devPanel'
import { panelOf } from './manual/panel/devPanel'
import type { TableMode } from './layout'
import { STEP_LABEL } from './turnText'

/** The steps `?step=` names, by their protocol names in lower case. */
const STEP_BY_NAME = new Map((Object.keys(STEP_LABEL) as Step[]).map((step) => [step.toLowerCase(), step]))

/** The moves `?fx=` can play on the fixture table, to look at the table effects (`dev/fxScenarios.ts`). */
export const DEV_FX = [
  'attack',
  'block',
  'damage',
  'death',
  'life',
  'enter',
  'cast',
  'rival',
  'activate',
  'resolve',
  'counter',
  'land',
  'draw',
  'equip',
  'strike',
] as const

/** The moves of `DEV_FX` that are moments of play (a cast and what follows), drawn by the travel layer. */
export const DEV_PLAYS = ['cast', 'rival', 'activate', 'resolve', 'counter', 'land', 'draw'] as const

/** One of `DEV_FX`. */
export type DevFx = (typeof DEV_FX)[number]

/** The longest slow-down `?slow=` accepts. */
export const MAX_DEV_SLOW = 20

/** The longest wait `?preview=` accepts, in milliseconds. */
export const MAX_DEV_PREVIEW_MS = 10_000

/** The most cards `shown` makes the dev reveal show. */
export const MAX_DEV_SHOWN = 12

/** How `?side=` sets the side column. */
export type DevSide = 'open' | 'folded' | 'log'

/** The address parameters of `/dev/table`. */
export interface DevSearch {
  /** The side column for a decision's home: `open`, `folded` to its rail, or open on its Log tab (`log`). */
  side?: DevSide
  /** How many seats the fixture table has, 2 or more; 4 when missing or wrong. */
  seats: number
  /** `duel`: two seats with 12 and 10 permanents of an early Commander game; `heavy`: two seats with 40 each. */
  board?: 'duel' | 'heavy'
  /** The layout to open in; the default for the seat count when missing. */
  mode?: TableMode
  /** The opponent to open focused; the one following the turn when missing. */
  focus?: PlayerId
  /** The decision to face, by name in `dev/prompts.ts` (such as `priority`); none when missing or unknown. */
  prompt?: string
  /** `you` makes it the viewer's own turn, which lights the "your turn" cue; the opponent after the viewer otherwise. */
  turn?: 'you' | 'left'
  /** `1` opens the Effects bar, which is folded behind one button otherwise (`dev/DevFxControls.tsx`). */
  bar?: true
  /** A move to play on the table once it has loaded, so its effect can be looked at; also on buttons in the page. */
  fx?: DevFx
  /** `1` passes the turn to the other side once the table has loaded, so the turn banner and the badge can be looked at (`dev/turnScene.ts`); `slow` stretches the banner. */
  newturn?: true
  /** Makes every table effect this many times slower (above 1, up to 20), to look at one before it is gone. */
  slow?: number
  /** Milliseconds the dev table takes to answer a preview of blocks or attacks (0 to 10 000), to look at "Checking outcome…". */
  preview?: number
  /** Stops the viewer already holds, as a spec such as `own:main1,opp:end:keep` (`dev/stopSpec.ts`); missing when it names none. */
  stops?: string
  /** The Full Control mode the viewer already holds, `next` or `held`; off when missing. */
  fc?: 'next' | 'held'
  /** A seat shown away, with 12 s before the game acts for it. */
  away?: number
  /** Cards in the viewer's hand (0 to 24), to look at a large hand; 7 when missing. */
  hand?: number
  /** Cards in the viewer's Delves Satchel (0 to 24), at the head of the hand dock; none when missing. */
  satchel?: number
  /** The step the turn is in, such as `main2` or `declareattackers`, to look at the theme of its phase; the fixture's own when missing. */
  step?: Step
  /** `1` gives the table house rules that differ from its format's own, to look at the list in the top bar. */
  rules?: true
  /** A fight against a boss: the last seats are the boss and this many minions (0 to 2), a team against the rest. */
  boss?: number
  /** `1` with `boss` makes the viewer a fallen hero's Spirit with no permanents, no hand and no commander (`withSpirit`). */
  spirit?: true
  /** `1` or `2` with `boss`: how far the fight has come (`withReworkedBoss`), before Phase 1 or with Phase 1 in force. */
  rework?: 1 | 2
  /** `1` leaves every battlefield empty (`withBareBoards`), as at the start of a fight, for the opening hands. */
  bare?: true
  /** `1` has every opponent but the last out of the game (conceded, then out of life) and leaves the last one's board empty (`withLastStanding`). */
  alone?: true
  /** `1` offers the viewer "Take back" for a spell, as the server does after a cast, to look at the fullest controls dock. */
  takeback?: true
  /** The deck lists the server tells (`dev/deckListScene.ts`): every opponent's `public` whole, or `hidden` to its commander. */
  decks?: 'public' | 'hidden'
  /** `1` gives the viewer and the rivals a permanent for every state a card shows (`dev/states.ts`), to look at them all at once. */
  states?: true
  /** `1` adds the attachments scene (`dev/equip.ts`): five attachments on one creature, an opponent's Aura on yours, yours on theirs, a Curse and an unattached Equipment. */
  equip?: true
  /** `1` adds the crowded scene (`dev/crowdScene.ts`): fourteen items on the stack, twelve of them triggers, and twelve attackers over three defenders. */
  crowd?: true
  /** `1` adds the revealed cards and notes scene (`dev/revealScene.ts`): the first opponent shows two hand cards and its top card, the viewer one hand card, and two permanents carry a note. */
  reveal?: true
  /** `1` puts counters on every player (`dev/countersScene.ts`): rad, poison, energy, experience and tickets, up to five kinds on one seat. */
  counters?: true
  /** `1` plays a reveal of three cards and a d20 roll once the table has loaded (`dev/momentScene.ts`), to look at the moments. */
  moments?: true
  /** With `moments`: how many cards the reveal shows (1 to 12), each with rules text as long as a real card's, to look at a reveal of many. */
  shown?: number
  /** `real` identifies the cards by the card search of a running server, so the table draws real pictures. */
  cards?: 'real'
  /** Opens the manual menu of a card once the table has loaded: `1` a permanent of the viewer, `hand` a card in the viewer's hand, `opp` a permanent of an opponent. */
  menu?: DevMenu
  /** Opens a row of the Commands panel once the table has loaded: `1` the life row, a command by its id (`draw`, `token`), `top` for the arrange list after a look at the top 3, or `card`: the first creature of the seat in the Card group, as "In the panel" picks it. */
  panel?: DevPanel
  /** Opens the table tour at the step of this id (`tour/tourSteps.ts`), `1` at its start; the tour never starts by itself here. */
  tour?: string
  /** Opens the rules lexicon, at this search; `1` with none. */
  lexicon?: string
  /** Permanents for each seat of a crowded table (8 to 60): 40 when missing, about a mid-game Commander board at 20. */
  permanents?: number
  /** Seats in two teams, even against odd seats: `2v2` (`teams`) or `2hg` (Two-Headed Giant, shared life). */
  teams?: DevTeams
  /** `layout` exposes the layout script (`dev/layoutScript.ts`) as `window.__tableScript` to step through its states. */
  script?: 'layout'
  /** Ends the game and shows the result page: the viewer `won`, `lost`, a `draw`, or the viewer's team won (`team`, with `teams`). */
  result?: DevResult
}

/** The team modes `?teams=` names. */
export type DevTeams = 'teams' | 'two_headed_giant'

/** The endings `?result=` names. */
export const DEV_RESULTS = ['won', 'lost', 'draw', 'team'] as const

/** One of `DEV_RESULTS`. */
export type DevResult = (typeof DEV_RESULTS)[number]

function teamsOf(value: unknown): DevTeams | undefined {
  if (value === '2v2' || value === 'teams') return 'teams'
  return value === '2hg' || value === 'two_headed_giant' ? 'two_headed_giant' : undefined
}

/** The cards `?menu=` opens the manual menu on. */
export type DevMenu = 'permanent' | 'hand' | 'opponent'

function menuOf(value: unknown): DevMenu | undefined {
  // The router parses an address again with what it made of it, so the names it made stay valid.
  if (value === 1 || value === true || value === '1' || value === 'permanent') return 'permanent'
  if (value === 'hand') return 'hand'
  return value === 'opp' || value === 'opponent' ? 'opponent' : undefined
}

const whole = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

const isDevFx = (value: unknown): value is DevFx => DEV_FX.some((name) => name === value)

/** Reads `?seats=&boss=&board=&mode=&focus=&prompt=&turn=&step=&hand=&satchel=&fx=&newturn=&bar=&slow=&preview=&stops=&fc=&rules=&spirit=&rework=&bare=&alone=&states=&cards=&menu=&teams=&result=&permanents=&script=`, ignoring whatever does not make sense. The router has already parsed numbers. */
export function parseDevSearch(search: Record<string, unknown>): DevSearch {
  const seats = whole(search.seats)
  const focus = whole(search.focus)
  const mode =
    search.mode === 'grid' || search.mode === 'focus' || search.mode === 'boss' ? search.mode : undefined
  const boss = whole(search.boss)
  const slow =
    typeof search.slow === 'number' && search.slow > 1 && search.slow <= MAX_DEV_SLOW
      ? search.slow
      : undefined
  const board = search.board === 'duel' || search.board === 'heavy' ? search.board : undefined
  return {
    // A board names a two-seat table.
    seats: board !== undefined ? 2 : seats === undefined || seats < 2 ? 4 : seats,
    board,
    mode,
    focus: focus === undefined || focus < 0 ? undefined : focus,
    prompt: typeof search.prompt === 'string' ? search.prompt : undefined,
    turn: search.turn === 'you' || search.turn === 'left' ? search.turn : undefined,
    fx: isDevFx(search.fx) ? search.fx : undefined,
    bar: search.bar === true || search.bar === 1 || search.bar === '1' ? true : undefined,
    newturn: search.newturn === true || search.newturn === 1 || search.newturn === '1' ? true : undefined,
    slow,
    // The router validates an address again with what it made of it, so both keep the form the address has.
    stops:
      typeof search.stops === 'string' && parseStopSpec(search.stops).length > 0 ? search.stops : undefined,
    fc: search.fc === 'next' || search.fc === 'held' ? search.fc : undefined,
    away: whole(search.away),
    cards: search.cards === 'real' ? 'real' : undefined,
    step: typeof search.step === 'string' ? STEP_BY_NAME.get(search.step.toLowerCase()) : undefined,
    hand:
      typeof search.hand === 'number' &&
      Number.isInteger(search.hand) &&
      search.hand >= 0 &&
      search.hand <= 24
        ? search.hand
        : undefined,
    satchel:
      typeof search.satchel === 'number' &&
      Number.isInteger(search.satchel) &&
      search.satchel >= 0 &&
      search.satchel <= 24
        ? search.satchel
        : undefined,
    menu: menuOf(search.menu),
    panel: panelOf(search.panel),
    tour: search.tour === 1 ? '1' : typeof search.tour === 'string' ? search.tour : undefined,
    lexicon: search.lexicon === 1 ? '1' : typeof search.lexicon === 'string' ? search.lexicon : undefined,
    teams: teamsOf(search.teams),
    script: search.script === 'layout' ? 'layout' : undefined,
    permanents:
      typeof search.permanents === 'number' &&
      Number.isInteger(search.permanents) &&
      search.permanents >= 8 &&
      search.permanents <= 60
        ? search.permanents
        : undefined,
    result: DEV_RESULTS.find((result) => result === search.result),
    side:
      search.side === 'open' || search.side === 'folded' || search.side === 'log' ? search.side : undefined,
    boss: boss === undefined || boss > 2 ? undefined : boss,
    rules: search.rules === true || search.rules === 1 || search.rules === '1' ? true : undefined,
    spirit: search.spirit === true || search.spirit === 1 || search.spirit === '1' ? true : undefined,
    rework: search.rework === 1 || search.rework === 2 ? search.rework : undefined,
    bare: search.bare === true || search.bare === 1 || search.bare === '1' ? true : undefined,
    alone: search.alone === true || search.alone === 1 || search.alone === '1' ? true : undefined,
    takeback: search.takeback === true || search.takeback === 1 || search.takeback === '1' ? true : undefined,
    decks: search.decks === 'public' || search.decks === 'hidden' ? search.decks : undefined,
    states: search.states === true || search.states === 1 || search.states === '1' ? true : undefined,
    moments: search.moments === true || search.moments === 1 || search.moments === '1' ? true : undefined,
    shown:
      typeof search.shown === 'number' &&
      Number.isInteger(search.shown) &&
      search.shown >= 1 &&
      search.shown <= MAX_DEV_SHOWN
        ? search.shown
        : undefined,
    reveal: search.reveal === true || search.reveal === 1 || search.reveal === '1' ? true : undefined,
    counters: search.counters === true || search.counters === 1 || search.counters === '1' ? true : undefined,
    crowd: search.crowd === true || search.crowd === 1 || search.crowd === '1' ? true : undefined,
    // The equip move needs the attachments scene, so it brings it.
    equip:
      search.equip === true || search.equip === 1 || search.equip === '1' || search.fx === 'equip'
        ? true
        : undefined,
    preview:
      typeof search.preview === 'number' &&
      Number.isInteger(search.preview) &&
      search.preview >= 0 &&
      search.preview <= MAX_DEV_PREVIEW_MS
        ? search.preview
        : undefined,
  }
}
