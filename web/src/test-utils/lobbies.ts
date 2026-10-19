import { vi } from 'vitest'
import type { Mock } from 'vitest'
import type { Api, LobbyListing } from '@/net/api'
import type { LobbyGame } from '@/protocol/generated'

/** One open game as the server lists it; `game` is the id and `Bo` hosts it unless told otherwise. */
export const lobbyRow = (over: Partial<LobbyGame> = {}): LobbyGame => ({
  game: 'g1',
  host: 'Bo',
  format: 'normal',
  filled: 2,
  total: 4,
  created_at: '2026-10-19T12:00:00Z',
  age_secs: 180,
  ...over,
})

/** `count` games `g1`, `g2`, ... hosted by `Host 1`, `Host 2`, ... with two of four seats taken. */
export const lobbyRows = (count: number): LobbyGame[] =>
  Array.from({ length: count }, (_, index) => lobbyRow({ game: `g${index + 1}`, host: `Host ${index + 1}` }))

/**
 * A fake `listLobbies` that answers like the server (the same listing object when the `ETag` still matches) and a
 * `joinGame` that gives seat 1 and the token `tok-1` unless a test says otherwise.
 */
export interface FakeLobbies {
  api: Pick<Api, 'listLobbies' | 'joinGame'>
  /** The mock behind `api.listLobbies`; `mock.calls[n][0]` is the listing call n was given. */
  listLobbies: Mock<Api['listLobbies']>
  /** The mock behind `api.joinGame`; give it `mockRejectedValueOnce` to make a join fail. */
  joinGame: Mock<Api['joinGame']>
  /** Changes what the next answers carry (a new `ETag`). */
  set(games: LobbyGame[]): void
  /** Makes every answer fail with `error` until `heal`. */
  fail(error: Error): void
  /** Ends the failure. */
  heal(): void
  /** Makes the next answer wait; the returned function lets it go. */
  hold(): () => void
}

/** Builds the fake with the games it starts out listing. */
export function fakeLobbies(initial: LobbyGame[] = []): FakeLobbies {
  let games = initial
  let version = 1
  let failure: Error | null = null
  let gate: Promise<void> | null = null
  const listLobbies = vi.fn<Api['listLobbies']>(async (previous) => {
    // What the server held when the request arrived, which a held answer still carries when it is let go.
    const wait = gate
    const asked = { games, etag: `"v${version}"`, failure }
    gate = null
    if (wait !== null) await wait
    if (asked.failure !== null) throw asked.failure
    if (previous?.etag === asked.etag) return previous
    return { games: asked.games, etag: asked.etag, fetchedAt: Date.now() }
  })
  const joinGame = vi.fn<Api['joinGame']>(async () => ({ seat: 1, token: 'tok-1' }))
  return {
    api: { listLobbies, joinGame },
    listLobbies,
    joinGame,
    set(next) {
      games = next
      version += 1
    },
    fail(error) {
      failure = error
    },
    heal() {
      failure = null
    },
    hold() {
      let release: () => void = () => undefined
      gate = new Promise<void>((resolve) => {
        release = resolve
      })
      return release
    },
  }
}

/** A listing as the client holds it, stamped now. */
export const listingOf = (games: LobbyGame[], etag = '"v1"'): LobbyListing => ({
  games,
  etag,
  fetchedAt: Date.now(),
})
