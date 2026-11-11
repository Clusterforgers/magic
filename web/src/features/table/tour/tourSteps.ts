import type { SideTab } from '../sideCounts'

/** One stop of the table tour: what it explains and where on the table it points. */
export interface TourStep {
  id: string
  title: string
  text: string
  /** Selectors of the part to light up, the first one in view wins; none in view centres the card. */
  targets: readonly string[]
  /** The side tab to bring up before pointing at it. */
  tab?: SideTab
}

/** The table tour for a new player, in reading order of the table. */
export const TOUR: readonly TourStep[] = [
  {
    id: 'welcome',
    title: 'Welcome to the table',
    text: 'A short look at where everything is. Use Next and Back, or the arrow keys; Skip ends the tour. Help in the top bar brings it back.',
    targets: [],
  },
  {
    id: 'hand',
    title: 'Your hand',
    text: 'Your cards sit at the bottom. Click one to play a land or cast a spell when you have priority.',
    targets: ['[data-region="hand"]'],
  },
  {
    id: 'battlefield',
    title: 'Your battlefield',
    text: 'Your permanents in lanes of creatures, other permanents and lands. The opponents sit above you, laid out the same way.',
    targets: ['[data-area="own"]'],
  },
  {
    id: 'mana',
    title: 'Mana',
    text: 'Your untapped mana sources and mana pool are shown beside your name. When a spell needs paying, click lands to tap them or press Pay automatically.',
    targets: [
      '[data-area="own"] [data-slot="pod-top"]',
      '[data-area="own"] [data-slot="mana-slot"]',
      '[data-area="own"]',
    ],
  },
  {
    id: 'middle',
    title: 'The middle bar',
    text: 'Whose turn and step it is, what is on the stack, and what the game asks of you right now. Read here first when unsure.',
    targets: ['[data-region="shared"]'],
  },
  {
    id: 'priority',
    title: 'Priority and passing',
    text: 'When you have priority you may act; the main button passes, moving to the next step or letting the top of the stack resolve. Stops choose where the game waits for you.',
    targets: ['[data-region="controls"]'],
  },
  {
    id: 'combat',
    title: 'Attacking and blocking',
    text: 'In your declare attackers step click your creatures to attack, then confirm in the middle bar. When attacked, click a blocker of yours, then the attacker it blocks.',
    targets: ['[data-area="opponents"]'],
  },
  {
    id: 'log',
    title: 'The log',
    text: 'Every play and every result, in order. Underlined words show their rules on hover.',
    targets: ['[aria-label="Game log"]', '[data-area="side"]'],
    tab: 'log',
  },
  {
    id: 'manual',
    title: 'Commands',
    text: 'The manual panel moves cards, adds counters, makes tokens and changes life by hand, for any card the game does not play for you yet.',
    targets: ['[data-slot="commands-panel"]', '[data-area="side"]'],
    tab: 'commands',
  },
  {
    id: 'cards',
    title: 'Reading cards',
    text: 'Hover or focus a card to see it large with its rules, counters and what affects it; click a card you are not playing to keep it open.',
    targets: [
      '[data-area="own"] [data-slot="battlefield-zone"] [data-slot="card-face"]',
      '[data-region="hand"] [data-slot="card-face"]',
    ],
  },
  {
    id: 'help',
    title: 'Help',
    text: 'The rules lexicon explains every keyword and rules term, and this tour is here again whenever you want it.',
    targets: ['[data-slot="help-menu"]'],
  },
]
