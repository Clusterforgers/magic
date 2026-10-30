// Goal: the pick of the Commands panel moves as one state machine (idle, armed, picked): arming keeps the chosen card for a cancel, a choice always wins, clearing and a card that left the table forget it, and the side area is told when to step aside and when to return.
import { describe, expect, it, vi } from 'vitest'
import { createPickStore, IDLE, nextPick, pickedCard } from './pick'
import type { PickEvent, PickState } from './pick'

const armed = (was: number | null = null): PickState => ({ phase: 'armed', for: 'its commands', was })
const picked = (card: number): PickState => ({ phase: 'picked', card })
const arm: PickEvent = { type: 'arm', for: 'its commands' }

describe('the pick state machine', () => {
  it.each<[string, PickState, PickEvent, PickState]>([
    ['idle arms', IDLE, arm, armed()],
    ['armed arms again', armed(), arm, armed()],
    ['picked arms and remembers the card', picked(7), arm, armed(7)],
    ['armed chooses', armed(), { type: 'choose', card: 4 }, picked(4)],
    ['armed with a card in hand chooses another', armed(7), { type: 'choose', card: 4 }, picked(4)],
    ['idle chooses (the menu\'s "In the panel")', IDLE, { type: 'choose', card: 4 }, picked(4)],
    ['picked chooses another', picked(7), { type: 'choose', card: 4 }, picked(4)],
    ['armed cancels to idle', armed(), { type: 'cancel' }, IDLE],
    ['armed over a card cancels back to it', armed(7), { type: 'cancel' }, picked(7)],
    ['idle cancels to idle', IDLE, { type: 'cancel' }, IDLE],
    ['picked ignores a cancel', picked(7), { type: 'cancel' }, picked(7)],
    ['picked clears', picked(7), { type: 'clear' }, IDLE],
    ['armed clears', armed(7), { type: 'clear' }, IDLE],
    ['the picked card leaving the table is forgotten', picked(7), { type: 'gone', card: 7 }, IDLE],
    ['another card leaving changes nothing', picked(7), { type: 'gone', card: 8 }, picked(7)],
    ['the card behind an armed pick leaving is forgotten', armed(7), { type: 'gone', card: 7 }, armed()],
  ])('%s', (_name, from, event, to) => {
    const after = nextPick(from, event)
    expect(after).toEqual(to)
    if (after.phase === 'armed') expect(after.for).toBe('its commands')
  })

  it('says which card it holds, also behind an armed pick', () => {
    expect(pickedCard(IDLE)).toBeNull()
    expect(pickedCard(armed())).toBeNull()
    expect(pickedCard(armed(7))).toBe(7)
    expect(pickedCard(picked(4))).toBe(4)
  })
})

describe('the pick store', () => {
  it('tells the side area to step aside while it waits and to return with a card or a cancel', () => {
    const store = createPickStore()
    const said: boolean[] = []
    store.watch((up) => said.push(up))
    store.dispatch(arm)
    store.dispatch({ type: 'choose', card: 3 })
    store.dispatch(arm)
    store.dispatch({ type: 'cancel' })
    expect(said).toEqual([false, true, false, true])
  })

  it('stays quiet for a cancel with nothing to wait for, a clear and a card that left', () => {
    const store = createPickStore()
    const said = vi.fn()
    store.watch(said)
    store.dispatch({ type: 'cancel' })
    store.dispatch({ type: 'clear' })
    store.dispatch({ type: 'gone', card: 1 })
    expect(said).not.toHaveBeenCalled()
  })

  it('notifies its readers on a change and not on a repeat that changes nothing', () => {
    const store = createPickStore()
    const heard = vi.fn()
    store.subscribe(heard)
    store.dispatch({ type: 'choose', card: 3 })
    expect(heard).toHaveBeenCalledTimes(1)
    store.dispatch({ type: 'cancel' })
    expect(heard).toHaveBeenCalledTimes(1)
    expect(store.get()).toEqual(picked(3))
  })
})
