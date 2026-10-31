/** A lobby around a fixture, with no server: a store holding a roster and a session that answers Ready itself. */
import type { GameContextValue } from '@/features/game/gameContext'
import type { LobbyApi } from '@/features/game/useLobbyControls'
import type { KnownGame } from '@/features/game/useGameInfo'
import type { DeckInfo } from '@/net/api'
import { LobbyError } from '@/net/lobbyErrors'
import { createSession } from '@/play/session'
import type { Session } from '@/play/session'
import { initialPlayState } from '@/play/store'
import type { LobbyRoster } from '@/play/store'
import type { Counts, DeckListItem, DeckState, HouseRules, SeatDeck } from '@/protocol/generated'
import { legalityIssue, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { DevLobbySearch } from './lobbySearch'

/** The text the dev page prints, so a production bundle can be searched for any trace of the fixture. */
export const LOBBY_FIXTURE_MARKER = 'dev-lobby-fixture-5b7d'

/** The names of the fixture's seats, in seat order. */
const SHORT = ['Ada', 'Bo', 'Cy', 'Dee', 'Eli', 'Fay', 'Gus', 'Hana', 'Ivo', 'Jun', 'Kit', 'Lena']

/** Names of the longest length the server accepts (32 characters), the first of them without a space to break at. */
const LONG = [
  'Wolfeschlegelsteinhausenbergerdo',
  'Bartholomew Featherstonehaugh Jr',
  'Maximiliana von Hohenzollern-Sig',
  'Ünïcödé Nämé Wïth Åccents Änd Öö',
  'Zebulon the Unpronounceable Ones',
  'Hubert Blaine Wolfeschlegelstein',
]

/** The decks the fixture's seats play, in turn: built-in ones and stored ones with cards to resolve by hand. */
const SEAT_DECKS: readonly SeatDeck[] = [
  { name: 'Mono Green Stompers', format: 'normal', manual_cards: 0 },
  { name: 'Burn', format: 'modern', manual_cards: 3 },
  { name: 'Mono White Weenies', format: 'normal', manual_cards: 0 },
  { name: 'Graveyard Pauper with a Very Long Deck Name That Wraps', format: 'pauper', manual_cards: 1 },
]

/** The built-in decks the chooser lists. */
const DEMO_DECKS: DeckInfo[] = [
  { id: 'mono-green', name: 'Mono Green Stompers', color: 'Green', size: 60, cards: [] },
  { id: 'mono-red', name: 'Mono Red Giants', color: 'Red', size: 60, cards: [] },
  { id: 'mono-white', name: 'Mono White Weenies', color: 'White', size: 60, cards: [] },
]

const counts = (over: Partial<Counts>): Counts => ({
  total: 60,
  verified: 60,
  read: 0,
  partial: 0,
  manual: 0,
  stale: 0,
  unplayable: 0,
  ...over,
})

function storedDeck(
  id: string,
  name: string,
  format: string,
  state: DeckState,
  tally: Counts,
  issues = 0,
): DeckListItem {
  return {
    id,
    name,
    format,
    created_at: '2026-10-10T09:00:00Z',
    updated_at: '2026-10-17T12:00:00Z',
    state,
    verified: state === 'verified',
    changed: false,
    checked_at: '2026-10-17T12:00:00Z',
    summary: { format, state, counts: tally, zones: [{ zone: 'main', counts: tally }], issues, to_read: 0 },
    commanders: [],
  }
}

/** The signed-in player's stored decks: verified, with cards to resolve by hand, blocked, and one the game refuses. */
/** The stored decks every fixture lists first. */
export const STORED_DECKS: DeckListItem[] = [
  storedDeck('stored-burn', 'Burn', 'modern', 'manual_play', counts({ verified: 57, manual: 3 })),
  storedDeck(
    'stored-pauper',
    'Pauper Affinity',
    'pauper',
    'blocked',
    counts({ verified: 58, unplayable: 2 }),
    2,
  ),
  storedDeck('stored-rakdos', 'Rakdos Midrange', 'standard', 'verified', counts({})),
  storedDeck(
    'stored-atraxa',
    'Atraxa Superfriends',
    'commander',
    'verified',
    counts({ total: 100, verified: 100 }),
  ),
]

/** The roster `options` describe: everybody connected unless away, and the ready seats ready. */
export function fixtureRoster({ seats, ready, away, names, nodeck }: DevLobbySearch): LobbyRoster {
  const listed = Array.from({ length: seats }, (_, seat) => {
    const deck = SEAT_DECKS[seat % SEAT_DECKS.length]
    return {
      seat,
      name: (names === 'long' ? LONG[seat % LONG.length] : SHORT[seat]) ?? `Seat ${seat + 1}`,
      connected: !away.includes(seat),
      ready: ready.includes(seat),
      ...(deck === undefined || nodeck?.includes(seat) === true ? {} : { deck }),
    }
  })
  return { seats: listed, needed: listed.filter((entry) => !entry.ready).length }
}

/**
 * A session that plays the server's part for this seat: Ready and Not ready change the roster at once. It is a real
 * session that never connects, so it has every action the page may call, now and as the session grows.
 */
function lobbySession(me: number): Session {
  const inert = createSession({ game: 'dev-lobby', name: 'dev' })
  const { store } = inert
  const setReady = (ready: boolean): boolean => {
    store.setState((state) => {
      if (state.lobby === null) return {}
      const seats = state.lobby.seats.map((entry) => (entry.seat === me ? { ...entry, ready } : entry))
      return { lobby: { seats, needed: seats.filter((entry) => !entry.ready).length } }
    })
    return true
  }
  return { ...inert, ready: () => setReady(true), notReady: () => setReady(false) }
}

/** The house rules of the fixture game that differ from its format's own. */
const DEV_RULES: HouseRules = { starting_life: 30, free_mulligans: 2, mulligan_style: 'vancouver' }

/**
 * A game context holding a lobby: the roster of `options`, this page seated at `me`, the socket open. The lookup says
 * only the visibility (the roster names the seats), so the host's toggle and the controls have something to draw.
 */
export function lobbyGame(options: DevLobbySearch): GameContextValue {
  const session = lobbySession(options.me)
  session.store.setState({
    ...initialPlayState(),
    status: 'open',
    seat: options.me,
    seatCount: options.seats,
    lobby: fixtureRoster(options),
  })
  const game: KnownGame = {
    started: false,
    seats: [],
    connected: [],
    ready: [],
    result: null,
    visibility: options.visibility ?? 'public',
    format: options.format ?? 'normal',
    ...(options.rules === true ? { house_rules: DEV_RULES } : {}),
  }
  return { game: 'dev-lobby', session, lookup: { game, missing: false, failed: false } }
}

/** What the table is told of a stored deck: the cards that are not verified are the ones to resolve by hand. */
const factsOf = (deck: DeckListItem): SeatDeck => ({
  name: deck.name,
  format: deck.format,
  manual_cards:
    deck.summary.counts.manual +
    deck.summary.counts.partial +
    deck.summary.counts.stale +
    deck.summary.counts.unplayable,
})

/** Setting a seat's deck in the roster, as the server's next `Lobby` message would. */
function setDeck(session: Session, seat: number, deck: SeatDeck): void {
  session.store.setState((state) => {
    if (state.lobby === null) return {}
    const seats = state.lobby.seats.map((entry) => (entry.seat === seat ? { ...entry, deck } : entry))
    return { lobby: { seats, needed: state.lobby.needed } }
  })
}

/**
 * The calls of the lobby's controls with no server behind them: every one succeeds, and a removal opens the seat in the
 * roster as the server's next `Lobby` message would. A deck is chosen after a short wait and shows in the roster; the
 * Commander deck is refused with its issues, as a Normal game refuses it. Close and Leave then take the page to Play.
 */
export function lobbyApi(
  session: Session,
  options: Pick<DevLobbySearch, 'format'> & { lookup?: (id: string) => SeatDeck | undefined } = {},
): LobbyApi {
  return {
    setVisibility: async (_game, visibility) => visibility,
    closeGame: async () => undefined,
    leaveGame: async () => undefined,
    releaseSeat: async (_game, seat) => {
      session.store.setState((state) => {
        if (state.lobby === null) return {}
        const seats = state.lobby.seats.map((entry) =>
          entry.seat === seat ? { ...entry, name: '', connected: false, ready: false } : entry,
        )
        return { lobby: { seats, needed: seats.filter((entry) => !entry.ready).length } }
      })
    },
    listDecks: async () => DEMO_DECKS,
    chooseDeck: async (_game, seat, id) => {
      await new Promise((resolve) => setTimeout(resolve, 400))
      const demo = DEMO_DECKS.find((deck) => deck.id === id)
      const stored = STORED_DECKS.find((deck) => deck.id === id)
      if (stored?.format === 'commander' && options.format !== 'commander') {
        throw new LobbyError('deck_blocked', 422, 'The deck cannot be played in this game.', null, [
          {
            code: 'no_commander_zone',
            message: 'a Normal game has no command zone, but this deck has a commander',
            zone: 'commander',
            oracle_id: null,
            name: null,
          },
          {
            code: 'copies',
            message: 'Sol Ring: 4 copies is the most a Normal deck may hold, and this deck has 5',
            zone: 'main',
            oracle_id: 'dev-sol-ring',
            name: 'Sol Ring',
          },
        ])
      }
      const facts =
        demo === undefined
          ? stored === undefined
            ? options.lookup?.(id)
            : factsOf(stored)
          : { name: demo.name, format: 'normal', manual_cards: 0 }
      if (facts === undefined) throw new LobbyError('unknown_deck', 404, 'no such deck')
      setDeck(session, seat, facts)
      return facts
    },
  }
}

/** What the table says of a deck: stored decks by their check state, precons by their code. */
export function tableReport(deck: string) {
  const stored = STORED_DECKS.find((candidate) => candidate.id === deck)
  const state = stored?.state ?? PRECON_STATES[deck] ?? 'verified'
  const cards = [reportCard('Forest', { count: 90 })]
  if (state === 'manual_play') cards.push(reportCard('Mana Reflection', { count: 4, support: 'manual' }))
  if (state === 'blocked') cards.push(reportCard('Phantom Card', { count: 2, support: 'unplayable' }))
  const issues = state === 'blocked' ? [legalityIssue('the deck is not legal in this format')] : []
  return reportOf(cards, issues)
}

const PRECON_STATES: Record<string, DeckState> = {
  'precon:c18-draconic-domination': 'verified',
  'precon:c19-merciless-rage': 'manual_play',
  'precon:c17-arcane-wizardry': 'blocked',
}
