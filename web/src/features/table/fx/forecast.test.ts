/** Goal: the stack forecast is read off the top stack item only, per card and per seat, for every seat count; a target preview's lines count only while their decision is pending; and nothing is said with an empty stack. */
import { describe, expect, it } from 'vitest'
import { fixtureGame } from '@/features/table/dev/devGame'
import type { PlayState } from '@/play/store'
import type { ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { previewForecastWords, stackForecastWords, topForecast } from './forecast'

const scene = (seats: number): PlayState =>
  fixtureGame({ seats, prompt: 'priority-forecast' }).session.store.getState()

/** The creature the scene's Giant Growth aims at. */
const target = (state: PlayState): ObjectId => {
  const view = must(state.view)
  const top = must(view.stack.cards?.at(-1))
  const aim = must(state.objectsById[top]?.targets?.[0]?.[0])
  if (!('Object' in aim)) throw new Error('the scene aims at a creature')
  return aim.Object.id
}

describe.each([2, 4, 12])('at %i seats', (seats) => {
  it('says the pump on its target and nothing on any other card or seat', () => {
    const state = scene(seats)
    const aimed = target(state)
    expect(stackForecastWords(state, { Object: aimed })).toBe('+3/+3 → 7/7')
    for (const object of must(state.view).objects) {
      if (object.id !== aimed) expect(stackForecastWords(state, { Object: object.id })).toBeNull()
    }
    for (const player of must(state.view).players) {
      expect(stackForecastWords(state, { Player: player.seat })).toBeNull()
    }
  })

  it('reads only the top item, so a forecast under it says nothing', () => {
    const state = scene(seats)
    const view = must(state.view)
    const cards = must(view.stack.cards)
    const under: PlayState = { ...state, view: { ...view, stack: { count: 1, cards: cards.slice(0, 1) } } }
    expect(topForecast(under)).toEqual([])
    expect(stackForecastWords(under, { Object: target(state) })).toBeNull()
  })
})

describe('a target preview', () => {
  const state = scene(2)
  const pending = must(
    fixtureGame({ seats: 2, prompt: 'priority-forecast' }).session.store.getState().pending,
  )
  const changes = [
    { subject: { Object: 7 }, change: { Damage: { amount: 3 } } },
    { subject: { Object: 7 }, change: { Leaves: { fate: 'Dies' } } },
  ] as const

  it('says the reply for the pending decision', () => {
    const asked: PlayState = {
      ...state,
      pending,
      preview: { decision: pending.id, key: 'k', forecast: null, changes: [...changes] },
    }
    expect(previewForecastWords(asked, { Object: 7 })).toBe('3 damage · dies')
    expect(previewForecastWords(asked, { Object: 8 })).toBeNull()
  })

  it('says nothing for a reply to another decision or without lines', () => {
    const stale: PlayState = {
      ...state,
      pending,
      preview: { decision: pending.id + 1, key: 'k', forecast: null, changes: [...changes] },
    }
    expect(previewForecastWords(stale, { Object: 7 })).toBeNull()
    const empty: PlayState = {
      ...state,
      pending,
      preview: { decision: pending.id, key: 'k', forecast: null },
    }
    expect(previewForecastWords(empty, { Object: 7 })).toBeNull()
  })
})
