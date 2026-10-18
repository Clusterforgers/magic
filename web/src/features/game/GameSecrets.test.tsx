// Goal: a seat token appears nowhere on the game page except inside the share links of the other seats, in every state the page can be in, and never in a message, the tab title or this seat's own link.
import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { saveInvites } from '@/lib/invites'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import type { GameHarness } from '@/test-utils/gameHarness'
import { SHARE_LINK, visibleOutsideShareLinks } from '@/test-utils/leaks'
import { lateJoin } from '@/test-utils/transcripts'

const MINE = tokenOf(0)
const OTHERS = [tokenOf(1), tokenOf(2)]
const EVERYONE = [MINE, ...OTHERS]
const names = {
  started: false,
  seats: [
    { seat: 0, name: 'Ana' },
    { seat: 1, name: 'Bo' },
    { seat: 2, name: 'Cy' },
  ],
  connected: [1],
  result: null,
}

// The host's tab remembers every link, which is the worst case for a leak.
beforeEach(() => {
  saveInvites({
    game: GAME_ID,
    seats: names.seats.map((seat) => ({ ...seat, token: tokenOf(seat.seat) })),
  })
})

afterEach(() => {
  window.sessionStorage.clear()
})

async function failFive(h: GameHarness): Promise<void> {
  for (let attempt = 1; attempt <= 5; attempt += 1) {
    h.fail()
    if (attempt < 5) await h.advance(15_000)
  }
}

const open = (): Promise<GameHarness> => renderGame({ script: { game: () => jsonResponse(200, names) } })

// The states that show the lobby, which is where the share links are.
const LOBBY_STATES = new Set(['the lobby', 'the lobby after Ready', 'a server notice'])

/** Every state of the page, each as a way to bring a fresh page there. */
const states: [string, (h: GameHarness) => void | Promise<void>][] = [
  ['connecting', () => undefined],
  ['reconnecting', (h) => h.fail()],
  ['cannot reach the game', failFive],
  ['the lobby', (h) => h.join(0, 3)],
  [
    'the lobby after Ready',
    async (h) => {
      h.join(0, 3)
      await h.user.click(screen.getByRole('button', { name: 'Ready' }))
    },
  ],
  [
    'the table',
    (h) => {
      h.join()
      h.deliver(lateJoin.raw[1])
    },
  ],
  [
    'game over',
    (h) => {
      h.join()
      h.deliver(...lateJoin.raw.slice(1))
    },
  ],
  [
    'a server notice',
    (h) => {
      h.join(0, 3)
      h.deliver({ type: 'Error', code: 'IllegalAnswer', message: 'that is not allowed' })
    },
  ],
  [
    'a stopped connection',
    (h) => {
      h.join()
      h.close(1008, 'rate')
    },
  ],
  [
    'a replaced seat',
    (h) => {
      h.join()
      h.close(4000)
    },
  ],
]

describe('no token on the page outside a share link', () => {
  it.each(states)('in %s', async (_state, bringTo) => {
    const h = await open()
    await bringTo(h)
    await h.settle()
    const elsewhere = visibleOutsideShareLinks(document.body)
    for (const token of EVERYONE) expect(elsewhere).not.toContain(token)
    expect(document.title).not.toMatch(/[0-9a-f]{64}/)
  })

  it.each(states.filter(([state]) => !LOBBY_STATES.has(state)))(
    'and not even in a share link in %s, which only the lobby has',
    async (_state, bringTo) => {
      const h = await open()
      await bringTo(h)
      await h.settle()
      expect(document.querySelectorAll(SHARE_LINK)).toHaveLength(0)
      for (const token of EVERYONE) expect(document.body.innerHTML).not.toContain(token)
    },
  )
})

describe('the lobby’s share links', () => {
  it('are the other seats’ only, each carrying its own token and no other', async () => {
    const h = await open()
    h.join(0, 3)
    const links = [...document.querySelectorAll(SHARE_LINK)]
    expect(links).toHaveLength(2)
    links.forEach((link, index) => {
      const own = OTHERS[index] ?? ''
      expect(link.outerHTML).toContain(own)
      for (const token of EVERYONE.filter((candidate) => candidate !== own)) {
        expect(link.outerHTML).not.toContain(token)
      }
    })
  })

  it('never include this seat’s own token anywhere in the page', async () => {
    const h = await open()
    h.join(0, 3)
    expect(document.body.innerHTML).not.toContain(MINE)
  })
})

describe('where the token does go', () => {
  it('is in the socket address and in no message the page sends', async () => {
    const h = await open()
    h.join()
    await h.user.click(screen.getByRole('button', { name: 'Ready' }))
    expect(h.socket().url).toContain(MINE)
    expect(h.socket().frames.join('\n')).not.toContain(MINE)
  })

  it('is never sent to the REST server', async () => {
    const h = await open()
    h.join()
    await h.advance(10_000)
    for (const call of h.server.calls) {
      expect(call.url).not.toContain(MINE)
      expect(String(call.init?.body ?? '')).not.toContain(MINE)
      expect(JSON.stringify(call.init?.headers ?? {})).not.toContain(MINE)
    }
  })
})
