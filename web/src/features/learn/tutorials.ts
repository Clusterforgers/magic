import { createStore } from 'zustand/vanilla'
import { fragmentOf, seatLinkPath } from '@/lib/seatLink'
import type { Api, PracticeSeat } from '@/net/api'
import { COURSE, isLessonId } from './course'
import type { LessonId } from './step'

/** The tutorials of the app beside the course: the manual-panel lesson on a table, and the page tours. */
export type AppTutorialId = 'manual' | 'builder' | 'lobby' | 'review' | 'lexicon'

/** Every tutorial of the Learn page: a lesson of the course or one of the app's. */
export type TutorialId = LessonId | AppTutorialId

/** How an app tutorial runs: on a lesson table (the manual panel) or as a tour of a page. */
export type TutorialKind = 'table' | 'page'

/** One app tutorial of the Learn page. */
export interface Tutorial {
  id: AppTutorialId
  title: string
  text: string
  kind: TutorialKind
  /** About how long it takes, in minutes. */
  minutes: number
}

/** The app's tutorials under "The app", in the order a new player takes them. */
export const TUTORIALS: readonly Tutorial[] = [
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

/** Every tutorial id in the order of the Learn page. */
const ALL: readonly TutorialId[] = [
  ...COURSE.map((lesson) => lesson.id),
  ...TUTORIALS.map((tutorial) => tutorial.id),
]

/** The server lesson whose table the manual-panel lesson plays on. */
export const MANUAL_TABLE: LessonId = 'mana'

/** The `localStorage` key of the tutorials finished in this browser. */
export const DONE_KEY = 'learn-done'

function load(): readonly TutorialId[] {
  try {
    const raw = window.localStorage.getItem(DONE_KEY)
    const parsed: unknown = raw === null ? [] : JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return ALL.filter((id) => parsed.includes(id))
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
  const next = ALL.filter((each) => each === id || done.includes(each))
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
    return ALL.some((id) => id === value) ? (value as TutorialId) : null
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

/** The `sessionStorage` key of this tab's practice game: the one a new lesson replaces. */
export const PRACTICE_KEY = 'practice:seat'

/** This tab's practice game, or undefined when it has none or left it. */
export function practiceSeat(): PracticeSeat | undefined {
  try {
    const value: unknown = JSON.parse(window.sessionStorage.getItem(PRACTICE_KEY) ?? 'null')
    if (typeof value !== 'object' || value === null) return undefined
    const { game, token } = value as Record<string, unknown>
    return typeof game === 'string' && typeof token === 'string' ? { game, token } : undefined
  } catch {
    return undefined
  }
}

function rememberPractice(seat: PracticeSeat | null): void {
  try {
    if (seat === null) window.sessionStorage.removeItem(PRACTICE_KEY)
    else window.sessionStorage.setItem(PRACTICE_KEY, JSON.stringify(seat))
  } catch {
    // Storage refused: the server still ends the game a while after the learner left it.
  }
}

/**
 * Ends the practice game `seat` now (the learner left the lesson), so it neither runs on nor shows among the live
 * games; a failure is left to the server, which ends a game its learner left after a short grace.
 */
export async function leaveLesson(api: Pick<Api, 'endTutorial'>, seat: PracticeSeat): Promise<void> {
  if (practiceSeat()?.game === seat.game) rememberPractice(null)
  await api.endTutorial(seat)
}

/** Where a started lesson opens: the game's page with the seat link in the fragment. */
export interface LessonStart {
  game: string
  hash: string
}

/**
 * Starts tutorial `id` on a new table (a lesson, or the manual-panel lesson) and says where it opens. The tab's earlier
 * practice game ends with it, so a learner has one at a time.
 */
export async function startLesson(
  api: Pick<Api, 'startTutorial'>,
  id: LessonId | 'manual',
): Promise<LessonStart> {
  const table = isLessonId(id) ? id : MANUAL_TABLE
  const replaces = practiceSeat()
  const seat = await (replaces === undefined ? api.startTutorial(table) : api.startTutorial(table, replaces))
  rememberPractice({ game: seat.game, token: seat.token })
  rememberTutorial(seat.game, id)
  return { game: seat.game, hash: fragmentOf(seatLinkPath(seat.game, seat.seat, seat.token)) }
}
