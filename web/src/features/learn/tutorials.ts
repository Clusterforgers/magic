import { createStore } from 'zustand/vanilla'

/** The tutorials of the Learn page. */
export type TutorialId = 'game' | 'table' | 'builder' | 'lobby' | 'review' | 'manual' | 'lexicon'

/** How a tutorial runs: on the tutorial table (a lesson or the table tour) or as a tour of a page. */
export type TutorialKind = 'table' | 'page'

/** One entry of the Learn page. */
export interface Tutorial {
  id: TutorialId
  title: string
  text: string
  kind: TutorialKind
  /** About how long it takes, in minutes. */
  minutes: number
}

/** Every tutorial, in the order a new player takes them. */
export const TUTORIALS: readonly Tutorial[] = [
  {
    id: 'game',
    title: 'Playing a game',
    text: 'A guided game against a practice bot: lands, casting, priority, attacking, blocking, the stack, your commander and winning.',
    kind: 'table',
    minutes: 8,
  },
  {
    id: 'table',
    title: 'The table',
    text: 'Where everything is: your hand and battlefield, mana, the middle bar, the pass button, the log and the side panel.',
    kind: 'table',
    minutes: 2,
  },
  {
    id: 'builder',
    title: 'Building a deck',
    text: 'Make a deck or import a list, add cards, and read the deck check that says what runs automatically.',
    kind: 'page',
    minutes: 3,
  },
  {
    id: 'lobby',
    title: 'Choosing a deck and seats',
    text: 'Create a game, invite friends to its seats, choose your deck in the lobby and press Ready.',
    kind: 'page',
    minutes: 3,
  },
  {
    id: 'review',
    title: 'Reviewing cards',
    text: 'Read how the game runs a card and say whether it reads right, or check the cards you play by hand.',
    kind: 'page',
    minutes: 2,
  },
  {
    id: 'manual',
    title: 'The manual panel',
    text: 'Play a card by hand: the Commands panel, picking a card, and the card menu for one quick action.',
    kind: 'table',
    minutes: 3,
  },
  {
    id: 'lexicon',
    title: 'The rules lexicon',
    text: 'Look up any keyword or rules term by name or by what it does, and follow the words that link to others.',
    kind: 'page',
    minutes: 1,
  },
]

/** The `localStorage` key of the tutorials finished in this browser. */
export const DONE_KEY = 'learn-done'

function load(): readonly TutorialId[] {
  try {
    const raw = window.localStorage.getItem(DONE_KEY)
    const parsed: unknown = raw === null ? [] : JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return TUTORIALS.map((tutorial) => tutorial.id).filter((id) => parsed.includes(id))
  } catch {
    return []
  }
}

/** The tutorials finished in this browser. */
export const doneStore = createStore<{ done: readonly TutorialId[] }>(() => ({ done: load() }))

/** Marks `id` finished, in this browser. */
export function markDone(id: TutorialId): void {
  const done = doneStore.getState().done
  if (done.includes(id)) return
  const next = TUTORIALS.map((tutorial) => tutorial.id).filter((each) => each === id || done.includes(each))
  doneStore.setState({ done: next })
  try {
    window.localStorage.setItem(DONE_KEY, JSON.stringify(next))
  } catch {
    // Storage refused: the mark lasts until the page reloads.
  }
}

/** The `sessionStorage` key that says which tutorial a game of this tab plays. */
export const tutorialKey = (game: string): string => `tutorial:${game}`

/** The tutorial the game `game` plays in this tab, or null for a real game. */
export function tutorialOf(game: string): TutorialId | null {
  try {
    const value = window.sessionStorage.getItem(tutorialKey(game))
    return TUTORIALS.some((tutorial) => tutorial.id === value) ? (value as TutorialId) : null
  } catch {
    return null
  }
}

/** Remembers that the game `game` plays tutorial `id` in this tab. */
export function rememberTutorial(game: string, id: TutorialId): void {
  try {
    window.sessionStorage.setItem(tutorialKey(game), id)
  } catch {
    // Storage refused: the game plays without its coach.
  }
}
