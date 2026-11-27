import type { Target } from '@/features/learn/anchors'
import { resolve } from '@/features/learn/resolve'
import type { CardLookup } from '@/features/learn/resolve'
import type { SideTab } from '../sideCounts'

/** One stop of the table tour: what it explains and the part of the table it lights. */
export interface TourStep {
  id: string
  /** At most 28 characters. */
  title: string
  /** At most 160 characters; `{look}` is the verb of a look, as in a lesson's text. */
  text: string
  /** The anchors that may be lit, the first drawn wins; none drawn centres the card. */
  at: readonly Target[]
  /** The side tab to bring up before lighting it. */
  tab?: SideTab
}

/** The table tour for a new player, in reading order of the table. */
export const TOUR: readonly TourStep[] = [
  {
    id: 'welcome',
    title: 'Welcome to the table',
    text: 'A short look at where everything is. Next and Back move through it, as do the arrow keys; Skip ends it. Help in the top bar brings it back.',
    at: [],
  },
  {
    id: 'hand',
    title: 'Your hand',
    text: 'Your cards sit here. Choose one to play a land or cast a spell when you have priority.',
    at: ['hand'],
  },
  {
    id: 'battlefield',
    title: 'Your battlefield',
    text: 'Your permanents, in lanes of creatures, other permanents and lands. The opponents sit above you, laid out the same way.',
    at: ['own.board'],
  },
  {
    id: 'mana',
    title: 'Mana',
    text: 'Your untapped mana sources and floating mana. When a spell needs paying, choose lands to tap them or press Pay for me.',
    at: ['own.mana', 'own.pod'],
  },
  {
    id: 'middle',
    title: 'The middle bar',
    text: 'Whose turn and step it is, what is on the stack, and what the game asks of you now. Read here first when unsure.',
    at: ['shared'],
  },
  {
    id: 'priority',
    title: 'Priority and passing',
    text: 'With priority you may act. This button passes: the game moves to the next step or the top of the stack resolves. Stops choose where it waits for you.',
    at: ['pass.primary'],
  },
  {
    id: 'combat',
    title: 'Attacking and blocking',
    text: 'In your declare attackers step choose creatures to attack, then confirm. When attacked, choose a blocker, then the attacker it blocks.',
    at: ['opponents'],
  },
  {
    id: 'log',
    title: 'The log',
    text: 'Every play and every result, in order. Underlined words show their rules.',
    at: ['log'],
    tab: 'log',
  },
  {
    id: 'manual',
    title: 'Commands',
    text: 'The manual panel moves cards, adds counters, makes tokens and changes life by hand, for any card the game does not play for you yet.',
    at: ['commands'],
    tab: 'commands',
  },
  {
    id: 'cards',
    title: 'Reading cards',
    text: '{look} a card to see it large with its rules, its counters and what affects it.',
    at: ['board:*', 'hand:*'],
  },
  {
    id: 'help',
    title: 'Help',
    text: 'The rules lexicon explains every keyword and rules term, and this tour is here again whenever you want it.',
    at: ['help.menu'],
  },
]

/** What `step` lights now: the first of its anchors that resolves to a drawn element (or its opener), else nothing. */
export function tourTarget(step: TourStep, lookup: CardLookup, doc: Document = document): Target[] {
  const found = step.at.find((target) => resolve(target, lookup, doc).kind === 'found')
  return found === undefined ? [] : [found]
}
