import { createStore } from 'zustand/vanilla'
import type { Target } from './anchors'
import type { TutorialId } from './tutorials'

/** One stop of a page tour: what it explains and the anchors it may light, the first drawn winning; none centres it. */
export interface TourStop {
  id: string
  /** At most 28 characters. */
  title: string
  /** At most 160 characters. */
  text: string
  at: readonly Target[]
}

/** One part of a page tour: the page it runs on and its stops. */
export interface TourPart {
  /** Whether the part runs on the page at `path`. */
  on: (path: string) => boolean
  /** What to do to reach the part's page, said while another page shows. */
  lead: string
  stops: readonly TourStop[]
}

/** A tour of one or more pages: where it starts and its parts in order. */
export interface PageTour {
  id: TutorialId
  start: string
  parts: readonly TourPart[]
}

const DECKS = (path: string): boolean => path === '/decks'
const BUILDER = (path: string): boolean => /^\/decks\/(?!unvalidated)[^/]+$/.test(path)
const NEW = (path: string): boolean => path === '/new'
const GAME = (path: string): boolean => path.startsWith('/game/')

/** The page tours of the Learn page. */
export const PAGE_TOURS: readonly PageTour[] = [
  {
    id: 'builder',
    start: '/decks',
    parts: [
      {
        on: DECKS,
        lead: 'Open Decks in the navigation; signing in comes first, since decks belong to an account.',
        stops: [
          {
            id: 'decks',
            title: 'Your decks',
            text: 'Every deck of your account, with its format and how much of it the game plays automatically.',
            at: ['decks.list', 'decks.page'],
          },
          {
            id: 'import',
            title: 'Import a list',
            text: 'Paste a list from Arena, MTG Online, Moxfield or Archidekt. Unknown lines are named, and nothing is ever refused.',
            at: ['decks.import'],
          },
          {
            id: 'new',
            title: 'Start a new deck',
            text: 'An empty deck with a name and a format. It opens in the builder.',
            at: ['decks.new'],
          },
        ],
      },
      {
        on: BUILDER,
        lead: 'Open a deck from the list, or make a new one, to see the builder.',
        stops: [
          {
            id: 'search',
            title: 'Find cards',
            text: 'Search by name, type or rules text and filter by color. Add a card with its plus button; Shift adds four.',
            at: ['builder.search'],
          },
          {
            id: 'deck',
            title: 'The deck',
            text: 'The deck by zone with counts, the commander slot of a Commander deck, and its views: list, pictures and stats.',
            at: ['builder.deck'],
          },
          {
            id: 'check',
            title: 'The deck check',
            text: 'How many cards run automatically, which wait for a read and which you play by hand. Legality only warns; every card stays usable.',
            at: ['builder.check'],
          },
          {
            id: 'save',
            title: 'Save',
            text: 'Changes stay yours until you save. Leaving with unsaved changes asks first.',
            at: ['builder.save'],
          },
        ],
      },
    ],
  },
  {
    id: 'lobby',
    start: '/new',
    parts: [
      {
        on: NEW,
        lead: 'Open Create game on the Play page.',
        stops: [
          {
            id: 'format',
            title: 'Format and seats',
            text: 'Commander or Normal, and how many seats the table has. Everyone else joins an open seat.',
            at: ['create.game'],
          },
          {
            id: 'options',
            title: 'Rules and visibility',
            text: 'House rules change the format’s numbers. A public game is listed on the Play page; an invite game is reached by its link and code.',
            at: ['create.options'],
          },
          {
            id: 'create',
            title: 'Create',
            text: 'Create opens the lobby. The tour goes on there.',
            at: ['create.footer'],
          },
        ],
      },
      {
        on: GAME,
        lead: 'Create the game to open its lobby.',
        stops: [
          {
            id: 'invite',
            title: 'Invite',
            text: 'Send the link or the code to your friends. Each join takes an open seat.',
            at: ['lobby.invite', 'lobby.game'],
          },
          {
            id: 'seats',
            title: 'The seats',
            text: 'Who sits where, whether they are connected, their deck and whether they are ready. The host can remove a player.',
            at: ['lobby.game'],
          },
          {
            id: 'deck',
            title: 'Your deck',
            text: 'Choose a deck of yours or a precon. Its check says what runs automatically at this table.',
            at: ['lobby.deck', 'lobby.chooser'],
          },
          {
            id: 'ready',
            title: 'Ready',
            text: 'Press Ready when your deck is set. The game starts when every seat is ready.',
            at: ['lobby.ready'],
          },
        ],
      },
    ],
  },
  {
    id: 'review',
    start: '/review',
    parts: [
      {
        on: (path) => path === '/review',
        lead: 'Open Review cards in the navigation; it needs an account.',
        stops: [
          {
            id: 'queue',
            title: 'What waits',
            text: 'A card waiting for your read runs on rules the game drafted and nobody verified yet; a card to check is one you play partly by hand.',
            at: ['review.queue', 'review.page'],
          },
          {
            id: 'card',
            title: 'Read a card',
            text: 'Each line of the printed text sits beside the game’s words for it. Compare them line by line.',
            at: ['review.read', 'review.check', 'review.empty'],
          },
          {
            id: 'verdict',
            title: 'Your verdict',
            text: 'Reads right or Reads wrong with a note. Each verdict opens the next card; C, W, J and K do the same from the keyboard.',
            at: ['review.verdict', 'review.actions'],
          },
        ],
      },
    ],
  },
  {
    id: 'lexicon',
    start: '/learn',
    parts: [
      {
        on: (path) => path === '/learn',
        lead: 'Open the Learn page.',
        stops: [
          {
            id: 'search',
            title: 'Search',
            text: 'Type a name such as “trample”, or what it does, such as “can’t be blocked”.',
            at: ['lexicon.search', 'lexicon'],
          },
          {
            id: 'kinds',
            title: 'Kinds',
            text: 'Show only keyword abilities, keyword actions, ability words or rules terms. Each button counts its matches.',
            at: ['lexicon.kinds'],
          },
          {
            id: 'entry',
            title: 'An entry',
            text: 'The term and its reminder text. An underlined word opens its own entry.',
            at: ['lexicon.entry', 'lexicon'],
          },
          {
            id: 'table',
            title: 'At the table',
            text: 'The question mark in the table’s top bar opens the lexicon; underlined words on cards and in the log show their rules on hover.',
            at: [],
          },
        ],
      },
    ],
  },
]

/** The page tour on show: which, its part and its stop; null while none runs. */
export const pageTourStore = createStore<{ tour: TutorialId | null; part: number; stop: number }>(() => ({
  tour: null,
  part: 0,
  stop: 0,
}))

/** The page tour called `id`, if there is one. */
export function pageTourOf(id: TutorialId | null): PageTour | undefined {
  return PAGE_TOURS.find((tour) => tour.id === id)
}

/** Starts the page tour `id` at its first stop. */
export function startPageTour(id: TutorialId): void {
  pageTourStore.setState({ tour: id, part: 0, stop: 0 })
}

/** Ends the page tour on show. */
export function endPageTour(): void {
  pageTourStore.setState({ tour: null, part: 0, stop: 0 })
}

/**
 * Moves `by` stops: past a part's last stop to the next part's first, past the tour's last stop to the end, which
 * returns the tour's id (the caller marks it done); before the first stop it stays.
 */
export function movePageTour(by: 1 | -1): TutorialId | null {
  const { tour, part, stop } = pageTourStore.getState()
  const found = pageTourOf(tour)
  if (found === undefined) return null
  const stops = found.parts[part]?.stops.length ?? 0
  if (by === -1) {
    if (stop > 0) pageTourStore.setState({ stop: stop - 1 })
    else if (part > 0)
      pageTourStore.setState({ part: part - 1, stop: (found.parts[part - 1]?.stops.length ?? 1) - 1 })
    return null
  }
  if (stop + 1 < stops) {
    pageTourStore.setState({ stop: stop + 1 })
    return null
  }
  if (part + 1 < found.parts.length) {
    pageTourStore.setState({ part: part + 1, stop: 0 })
    return null
  }
  endPageTour()
  return found.id
}

/** Where a page tour stands: the stop to show and its place, or the lead to the part's page. */
export type TourPlace =
  | { kind: 'stop'; stop: TourStop; index: number; count: number; last: boolean }
  | { kind: 'lead'; lead: string; index: number; count: number }

/** The place of the tour `id` at `part` and `stop` on the page at `path`. */
export function placeOf(id: TutorialId, part: number, stop: number, path: string): TourPlace | null {
  const tour = pageTourOf(id)
  const current = tour?.parts[part]
  if (tour === undefined || current === undefined) return null
  const before = tour.parts.slice(0, part).reduce((sum, each) => sum + each.stops.length, 0)
  const count = tour.parts.reduce((sum, each) => sum + each.stops.length, 0)
  const index = before + stop
  if (!current.on(path)) return { kind: 'lead', lead: current.lead, index, count }
  const shown = current.stops[stop]
  if (shown === undefined) return null
  return { kind: 'stop', stop: shown, index, count, last: index === count - 1 }
}
