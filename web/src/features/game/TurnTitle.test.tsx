// Goal: on the real game page the tab title carries "Your turn · " for as long as the viewer is the active seat, shows "● Your move" for a backgrounded tab that holds a decision, stays plain on the lobby, connection and result screens, and puts the title it found back when the page goes.
import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ServerMessage, StateView } from '@/protocol/generated'
import { clone } from '@/test-utils/batches'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderGame } from '@/test-utils/gameHarness'
import { lateJoin } from '@/test-utils/transcripts'

const names = {
  started: true,
  seats: [
    { seat: 0, name: 'Ana' },
    { seat: 1, name: 'Bo' },
  ],
  connected: [0, 1],
  result: null,
}

/** The transcript's own Snapshot with the turn handed to `active`, in a game that is still running. */
function snapshot(active: number, seq: number): ServerMessage {
  const message = clone(lateJoin.messages[1])
  if (message?.type !== 'Snapshot')
    throw new Error('the transcript has no Snapshot where the test expects it')
  const state: StateView = message.state
  state.stage = 'Turns'
  state.result = null
  state.turn = { ...state.turn, active }
  for (const player of state.players) player.lost = null
  return { ...message, seq, state }
}

const decision = (id: number): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player: 0,
    kind: { Priority: { playable_lands: [], castable: [], mana_abilities: [], can_pass: true } },
  },
})

let hidden = false
function setHidden(next: boolean): void {
  hidden = next
  act(() => {
    document.dispatchEvent(new Event('visibilitychange'))
  })
}

async function game() {
  return renderGame({ script: { game: () => jsonResponse(200, names) } })
}

beforeEach(() => {
  hidden = false
  document.title = 'Elsewhere'
  vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => (hidden ? 'hidden' : 'visible'))
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('the title of the game page', () => {
  it('has no prefix on an opponent’s turn, takes it on the viewer’s turn and drops it when the turn passes', async () => {
    const h = await game()
    h.join(0, 2)
    h.deliver(snapshot(1, 1))
    expect(document.title).toBe('Game · Magic')
    h.deliver(snapshot(0, 2))
    expect(document.title).toBe('Your turn · Game · Magic')
    h.deliver(snapshot(1, 3))
    expect(document.title).toBe('Game · Magic')
  })

  it('keeps the prefix through the viewer’s own decisions', async () => {
    const h = await game()
    h.join(0, 2)
    h.deliver(snapshot(0, 1), decision(7))
    expect(document.title).toBe('Your turn · Game · Magic')
  })

  it('calls a backgrounded tab with "● Your move" while a decision waits, and settles when the tab returns', async () => {
    const h = await game()
    h.join(0, 2)
    h.deliver(snapshot(1, 1))
    setHidden(true)
    expect(document.title).toBe('Game · Magic')
    h.deliver(decision(7))
    expect(document.title).toBe('● Your move · Magic')
    setHidden(false)
    expect(document.title).toBe('Game · Magic')
  })

  it('stays plain in the lobby, with or without a hidden tab', async () => {
    const h = await game()
    h.join(0, 2)
    expect(document.title).toBe('Lobby · Magic')
    setHidden(true)
    expect(document.title).toBe('Lobby · Magic')
  })

  it('stays plain on the result screen even on the viewer’s turn', async () => {
    const h = await game()
    h.join(0, 2)
    h.deliver(snapshot(0, 1), decision(7))
    expect(document.title).toBe('Your turn · Game · Magic')
    h.deliver({ type: 'GameOver', result: { Winner: 1 } })
    await h.settle()
    expect(document.title).toBe('Game over · Magic')
    setHidden(true)
    expect(document.title).toBe('Game over · Magic')
  })

  it('puts the title it found back when the page goes, flashing or not', async () => {
    const h = await game()
    h.join(0, 2)
    h.deliver(snapshot(0, 1), decision(7))
    setHidden(true)
    expect(document.title).toBe('● Your move · Magic')
    h.unmount()
    expect(document.title).toBe('Elsewhere')
  })
})
