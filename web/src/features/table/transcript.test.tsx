// Goal: replaying a recorded game through the real session and store into the table, message by message, keeps the screen equal to the state after every message (seats, battlefields, hand, stack, log, mulligan and decision lines) and ends equal to the server's own final view.
import { act, within } from '@testing-library/react'
import { beforeAll, describe, expect, it } from 'vitest'
import type { GameLookup } from '@/features/game/useGameInfo'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { transcripts } from '@/test-utils/transcripts'
import type { Transcript } from '@/test-utils/transcripts'
import { describeMulligan } from './turnText'

const lookup: GameLookup = {
  game: {
    started: true,
    seats: [
      { seat: 0, name: 'Ana' },
      { seat: 1, name: 'Bo' },
    ],
    connected: [0, 1],
    result: null,
  },
  missing: false,
  failed: false,
}

// Where each recording stops: the late join runs to GameOver, the lobby one ends in the third turn.
const FINAL_BANNER: Record<string, string> = {
  'late join': 'The game is over',
  lobby: 'Turn 3 · Ana · Upkeep',
}

const ids = (elements: Iterable<Element>): number[] =>
  [...elements].map((element) => Number(element.getAttribute('data-object-id')))
const sorted = (list: readonly number[]): number[] => [...list].sort((a, b) => a - b)

/** What the screen shows must be what the state holds; `step` names the message just delivered. */
function expectScreenToMatch(container: HTMLElement, state: PlayState, step: string): void {
  const { view } = state
  if (view === null) {
    expect(container, step).toHaveTextContent('Waiting for the table…')
    return
  }
  expect(container.querySelectorAll('[data-seat]'), step).toHaveLength(view.players.length)
  for (const player of view.players) {
    const panel = must(container.querySelector<HTMLElement>(`[data-seat="${player.seat}"]`), `${step}: panel`)
    expect(panel, step).toHaveTextContent(`Life${player.life}`)
    expect(panel, step).toHaveTextContent(`Library ${player.library.count}`)
    expect(panel, step).toHaveTextContent(`Hand ${player.hand.count}`)
    const mulligan = panel.querySelector('[data-slot="mulligan"]')
    if (view.stage === 'Mulligan') {
      expect(mulligan?.textContent, step).toBe(
        `Mulligan: ${describeMulligan(player.mulligan, player.mulligans)}`,
      )
    } else {
      expect(mulligan, step).toBeNull()
    }
    // Every permanent a seat controls is on screen once, in that seat's battlefield and nowhere else.
    const mine = view.objects.filter(
      (object) => object.zone === 'Battlefield' && object.controller === player.seat,
    )
    const shown = ids(
      container.querySelectorAll(`[data-battlefield-seat="${player.seat}"] button[data-object-id]`),
    )
    expect(sorted(shown), `${step}: battlefield of seat ${player.seat}`).toEqual(
      sorted(mine.map((object) => object.id)),
    )
  }
  const viewer = must(state.seat)
  const hand = must(view.players[viewer]?.hand.cards)
  const handRow = container.querySelector(`ul[aria-label="${must(lookup.game).seats[viewer]?.name}'s hand"]`)
  expect(ids(handRow?.querySelectorAll('button[data-object-id]') ?? []), `${step}: hand`).toEqual(hand)
  const stack = within(container).queryByRole('list', { name: 'Stack, top first' })
  expect(ids(stack?.querySelectorAll('button[data-object-id]') ?? []), `${step}: stack`).toEqual(
    must(view.stack.cards).toReversed(),
  )
  expect(
    within(container).getByRole('heading', { name: `Stack (${view.stack.count})` }),
    step,
  ).toBeInTheDocument()
  const log = within(container).getByRole('log', { name: 'Game log' })
  if (state.log.length === 0) expect(log, step).toHaveTextContent('Nothing has happened yet.')
  else expect(log.querySelectorAll('li'), `${step}: log`).toHaveLength(state.log.length)
  const decision = container.querySelector('[data-slot="decision-line"]')?.textContent ?? ''
  expect(decision.startsWith('Your decision: '), `${step}: decision "${decision}"`).toBe(
    state.pending !== null,
  )
}

interface Replay {
  /** How many messages were delivered and checked. */
  checked: number
  final: PlayState
  /** Every distinct turn banner shown on the way, and the last one. */
  banners: Set<string>
  lastBanner: string | null
  stages: Set<string>
}

/** Plays every message of a transcript into a table, checking the screen after each. */
function replay(transcript: Transcript): Replay {
  const h = startSession()
  h.session.connect()
  h.open()
  const { container } = renderLiveTable(h.session, lookup)
  expectScreenToMatch(container, h.state(), 'before the first message')
  const result: Replay = {
    checked: 0,
    final: h.state(),
    banners: new Set(),
    lastBanner: null,
    stages: new Set(),
  }
  for (const [index, message] of transcript.raw.entries()) {
    act(() => h.deliver(message))
    expectScreenToMatch(container, h.state(), `after message ${index} (${transcript.messages[index]?.type})`)
    result.checked += 1
    const banner = container.querySelector('[data-slot="banner"]')?.textContent
    if (banner !== undefined && banner !== null) {
      result.banners.add(banner)
      result.lastBanner = banner
    }
    const stage = h.state().view?.stage
    if (stage !== undefined) result.stages.add(stage)
  }
  result.final = h.state()
  return result
}

describe.each(transcripts)('replaying the $name transcript of seat 0', (transcript) => {
  let played: Replay
  // One replay serves every check below; the screen is verified message by message while it runs.
  beforeAll(() => {
    played = replay(transcript)
  })

  it('keeps the table equal to the store after every one of its messages', () => {
    expect(played.checked).toBe(transcript.raw.length)
    expect(played.checked).toBeGreaterThan(50)
  })

  it("ends on the server's own final view", () => {
    expect(played.final.view).toEqual(transcript.expected)
  })

  it('words the turn banner while it plays and where the recording ends', () => {
    expect([...played.banners].some((text) => /^Turn \d+ · (Ana|Bo) · /.test(text))).toBe(true)
    expect(played.lastBanner).toBe(FINAL_BANNER[transcript.name])
  })

  it('goes through the opening hands first when the recording starts at the lobby', () => {
    expect(played.stages.has('Turns')).toBe(true)
    expect(played.banners.has('Opening hands')).toBe(played.stages.has('Mulligan'))
    if (transcript.name === 'lobby') expect(played.stages.has('Mulligan')).toBe(true)
  })
})
