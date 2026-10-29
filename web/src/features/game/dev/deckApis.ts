/** The deck and precon calls of `/dev/lobby`: lists, cards and the table's verdicts from memory, with switches for the states. */
import { createDecksApi } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import { PreconError } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import type { DeckListItem, SeatDeck } from '@/protocol/generated'
import {
  commanderHit,
  fixtureDeckView,
  fixtureListItem,
  fixturePreconItem,
  fixturePreconView,
  manyPrecons,
  manyStoredDecks,
} from './deckFixture'
import type { FixtureDeck } from './deckFixture'
import { STORED_DECKS, tableReport } from './lobbyGame'
import type { DevLobbySearch } from './lobbySearch'

/** The switches of the fixture's deck calls. */
export type DevDeckSwitches = Pick<DevLobbySearch, 'decks' | 'list' | 'detail'>

const answer = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

const never = <T>(): Promise<T> => new Promise<T>(() => undefined)
const later = async <T>(ms: number, value: T): Promise<T> => {
  await new Promise((resolve) => setTimeout(resolve, ms))
  return value
}

const BASE: readonly FixtureDeck[] = [
  { id: 'stored-burn', name: 'Burn', format: 'modern', commanders: [], size: 60, hand: ['Lightning Bolt'] },
  { id: 'stored-pauper', name: 'Pauper Affinity', format: 'pauper', commanders: [], size: 60 },
  { id: 'stored-rakdos', name: 'Rakdos Midrange', format: 'standard', commanders: [], size: 60 },
  {
    id: 'stored-atraxa',
    name: 'Atraxa Superfriends',
    format: 'commander',
    commanders: [['Atraxa, Praetors’ Voice', ['W', 'U', 'B', 'G']]],
    hand: ['Rhystic Study', 'Beast Within'],
  },
]

const PRECONS: readonly (FixtureDeck & { set: string; state?: 'verified' | 'manual_play' | 'blocked' })[] = [
  {
    id: 'c19-merciless-rage',
    set: 'c19',
    name: 'Merciless Rage',
    format: 'commander',
    commanders: [['Kediss, Emberclaw Familiar', ['R']]],
    hand: ['Cyclonic Rift'],
  },
  {
    id: 'c18-draconic-domination',
    set: 'c18',
    name: 'Draconic Domination',
    format: 'commander',
    commanders: [['The Ur-Dragon', ['W', 'U', 'B', 'R', 'G']]],
  },
  {
    id: 'c17-arcane-wizardry',
    set: 'c17',
    name: 'Arcane Wizardry',
    format: 'commander',
    commanders: [['Inalla, Archmage Ritualist', ['U', 'B', 'R']]],
    state: 'blocked',
  },
  {
    id: 'st-mono-white',
    set: 'st',
    name: 'Mono White Starter',
    format: 'freeform',
    commanders: [],
    size: 60,
  },
]

function storedFixtures(many: boolean): FixtureDeck[] {
  return [...BASE, ...(many ? manyStoredDecks() : [])]
}

function preconFixtures(many: boolean) {
  return [...PRECONS, ...(many ? manyPrecons() : [])]
}

/** What the table is told of a deck chosen from the fixture's lists: its name, format and cards played by hand. */
export function deckFacts(id: string, switches: DevDeckSwitches = {}): SeatDeck | undefined {
  const many = switches.decks === 'many'
  const precon = preconFixtures(many).find((candidate) => `precon:${candidate.id}` === id)
  const deck = precon ?? storedFixtures(many).find((candidate) => candidate.id === id)
  if (deck === undefined) return undefined
  return {
    name: deck.name,
    format: deck.format === 'freeform' ? 'freeform' : deck.format,
    manual_cards: (deck.hand ?? []).length,
  }
}

/** The signed-in player's deck calls: the list, a deck's cards and the table's verdict on a deck. */
export function lobbyDecks(switches: DevDeckSwitches = {}): DecksApi {
  const many = switches.decks === 'many'
  return createDecksApi({
    fetch: async (url, init) => {
      if (url.endsWith('/api/decks/check')) {
        const asked = JSON.parse(String(init?.body)) as { deck: string }
        return answer(tableReport(asked.deck))
      }
      if (url.endsWith('/api/decks')) {
        if (switches.list === 'loading') return never<Response>()
        if (switches.list === 'failed')
          return answer({ error: 'internal', message: 'The server is busy.' }, 500)
        if (switches.list === 'empty') return answer({ decks: [] })
        const fixed = storedFixtures(many).map(fixtureListItem)
        // The four base decks keep the check state the verdict tests rely on.
        const decks: DeckListItem[] = fixed.map(
          (item) => STORED_DECKS.find((base) => base.id === item.id) ?? item,
        )
        return answer({
          decks: decks.map((deck) =>
            deck.id === 'stored-atraxa'
              ? { ...deck, commanders: [commanderHit('Atraxa, Praetors’ Voice', ['W', 'U', 'B', 'G'])] }
              : deck,
          ),
        })
      }
      const id = decodeURIComponent(url.slice(url.lastIndexOf('/') + 1))
      const deck = storedFixtures(many).find((candidate) => candidate.id === id)
      if (switches.detail === 'failed' || deck === undefined) {
        return answer({ error: 'internal', message: 'The server is busy.' }, 500)
      }
      const view = fixtureDeckView(deck)
      return switches.detail === 'slow' ? later(2500, answer(view)) : answer(view)
    },
  })
}

/** The precons' calls: the list, and a precon's cards. */
export function lobbyPrecons(switches: DevDeckSwitches = {}): Pick<PreconsApi, 'list' | 'get'> {
  const many = switches.decks === 'many'
  return {
    list: async () => ({ precons: preconFixtures(many).map(fixturePreconItem) }),
    get: async (code) => {
      const deck = preconFixtures(many).find((candidate) => candidate.id === code)
      if (switches.detail === 'failed' || deck === undefined) {
        throw new PreconError(
          'network',
          0,
          'Could not reach the server. Check your connection and try again.',
        )
      }
      const view = fixturePreconView(deck)
      return switches.detail === 'slow' ? later(2500, view) : view
    },
  }
}
