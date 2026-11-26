/**
 * Where a decision is answered (`docs/ui-design.md`, Decisions): its home (the Stack panel, the controls dock or a
 * phone's bar) and where its detail stands in that home. Pure: `useDecisionHome` joins it with the table.
 */
import type { SideKind } from '../layout'
import type { SideTab } from '../sideCounts'

/** The box that holds a decision's question and answers. */
export type Home = 'panel' | 'dock' | 'bar'

/** Where a decision's detail stands: beside the question, a list under it, a window over the table, a phone's sheet. */
export type DetailPlace = 'inline' | 'list' | 'window' | 'sheet'

/** What the side area shows, as far as a home depends on it. */
export interface SideView {
  kind: SideKind
  /** The panel is up: always in a column, on request in a rail or a sheet. */
  open: boolean
  tab: SideTab
}

/** What a home depends on of the table's plan. */
export interface HomePlan {
  /** A narrow window (XS): everything is answered in the bar under the hand. */
  phone: boolean
}

/** The game's questions: answered in the dock (the bar on a phone) whatever the side area shows. */
const ASKED_BY_GAME: ReadonlySet<string> = new Set([
  'Priority',
  'Mulligan',
  'BottomCards',
  'Discard',
  'DeclareAttackers',
  'DeclareBlockers',
  'R509BlockPiles',
])

/** Short details that stand beside the question in every home. */
const INLINE: ReadonlySet<string> = new Set(['YesNo', 'ChooseNumber', 'Loop', 'R706ChooseDice'])

/** Details that are a list one column wide in the panel and a window elsewhere. */
const LIST: ReadonlySet<string> = new Set([
  'ChooseMode',
  'ChooseCosts',
  'OrderTriggers',
  'DivideAmount',
  'AssignDamage',
  'Scry',
  'OrderCards',
  'ChooseReplacement',
  'ChooseName',
])

/** Up to this many options stand as buttons beside the question; more are a list. */
export const INLINE_OPTIONS = 4

/** More cards than this are chosen in a window wherever the home is. */
export const WINDOW_FROM = 12

/** The decisions answered by pressing the table: everything else on it dims while they wait. */
const BOARD_CHOICES: ReadonlySet<string> = new Set([
  'ChooseTargets',
  'ChooseCards',
  'PayMana',
  'DeclareAttackers',
  'DeclareBlockers',
  'AssignDamage',
])

/** Whether a card asks a decision of this kind (a spell, an ability, a trigger); the game asks the rest. */
export function askedByCard(kind: string): boolean {
  return kind !== '' && !ASKED_BY_GAME.has(kind)
}

/**
 * The home of a decision of `kind`: a phone's bar; the dock for the game's questions; the panel for a card's question
 * while the side panel is up on its Stack tab, else the dock.
 */
export function decisionHome(kind: string, side: SideView | null, plan: HomePlan): Home {
  if (plan.phone) return 'bar'
  if (!askedByCard(kind) || side === null) return 'dock'
  return side.open && side.tab === 'stack' ? 'panel' : 'dock'
}

/**
 * Where the detail of a decision of `kind` stands in `home`, for `count` things to choose among (options, cards),
 * `offTable` when the cards are not on the table (a graveyard, a library, revealed cards).
 */
export function detailPlace(kind: string, home: Home, count: number, offTable = false): DetailPlace {
  if (kind === 'PayMana') return home === 'bar' ? 'sheet' : home === 'panel' ? 'list' : 'inline'
  if (kind === 'ChooseTargets' || kind === 'ChooseCards' || kind === 'Discard' || kind === 'BottomCards') {
    return offTable || count > WINDOW_FROM ? 'window' : 'inline'
  }
  if (INLINE.has(kind) || !askedByCard(kind)) return 'inline'
  if (kind === 'ChooseOption')
    return count <= INLINE_OPTIONS ? 'inline' : home === 'panel' ? 'list' : 'window'
  if (LIST.has(kind)) return home === 'panel' ? 'list' : 'window'
  return 'inline'
}

/** Whether a decision of `kind` is answered by pressing the table (`offTable`: its cards are in a window instead). */
export function isBoardChoice(kind: string, offTable = false): boolean {
  return BOARD_CHOICES.has(kind) && !(kind === 'ChooseCards' && offTable)
}

/** Whether a choice of targets is answered by the press itself: exactly one target, or up to one. */
export function answersOnPress(options: { min: number; max: number }): boolean {
  return options.max === 1
}
