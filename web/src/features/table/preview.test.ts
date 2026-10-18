// Goal: the card preview docks away from the card it shows, hover and focus come and go while a click pins, and the keyboard helpers move along a row from the right place; the dev route reads its address defensively.
import { describe, expect, it, vi } from 'vitest'
import { parseDevSearch } from './devSearch'
import { createPreviewStore, dockOf, SIDE_DOCK_FROM } from './preview'
import type { Anchor } from './preview'
import { objectIdOf, rovingTarget } from './useRoving'

const box = (left: number, top: number): Anchor => ({ left, top, width: 100, height: 140 })

describe('dockOf', () => {
  const wide = { width: 1400, height: 900 }
  const narrow = { width: 390, height: 844 }

  it.each([
    [box(100, 300), 'right'],
    [box(1200, 300), 'left'],
    [box(650, 300), 'right'],
    [box(700, 300), 'left'],
  ])("docks a wide screen's preview away from a card at %j: %s", (anchor, dock) => {
    expect(dockOf(anchor, wide)).toBe(dock)
  })

  it.each([
    [box(20, 100), 'bottom'],
    [box(20, 700), 'top'],
    [box(280, 352), 'bottom'],
    [box(280, 353), 'top'],
  ])("docks a phone's preview away from a card at %j: %s", (anchor, dock) => {
    expect(dockOf(anchor, narrow)).toBe(dock)
  })

  it('switches from the side to the top or bottom at the small breakpoint', () => {
    const anchor = box(100, 100)
    expect(dockOf(anchor, { width: SIDE_DOCK_FROM, height: 800 })).toBe('right')
    expect(dockOf(anchor, { width: SIDE_DOCK_FROM - 1, height: 800 })).toBe('bottom')
  })
})

describe('the preview store', () => {
  const a = box(10, 10)

  it('starts with nothing shown and tells its listeners about every change', () => {
    const store = createPreviewStore()
    const listener = vi.fn()
    store.subscribe(listener)
    expect(store.get()).toBeNull()
    store.show(5, a)
    expect(store.get()).toEqual({ id: 5, anchor: a, pinned: false })
    store.hide(5)
    expect(store.get()).toBeNull()
    expect(listener).toHaveBeenCalledTimes(2)
  })

  it('stops telling a listener that unsubscribed', () => {
    const store = createPreviewStore()
    const listener = vi.fn()
    const stop = store.subscribe(listener)
    stop()
    store.show(1, a)
    expect(listener).not.toHaveBeenCalled()
  })

  it('does not notify for a change that changes nothing', () => {
    const store = createPreviewStore()
    const listener = vi.fn()
    store.subscribe(listener)
    store.hide(1)
    store.dismiss()
    expect(listener).not.toHaveBeenCalled()
  })

  it('leaves a card on show when another card is left', () => {
    const store = createPreviewStore()
    store.show(1, a)
    store.hide(2)
    expect(store.get()?.id).toBe(1)
  })

  it('replaces one hover with the next', () => {
    const store = createPreviewStore()
    store.show(1, a)
    store.show(2, a)
    expect(store.get()?.id).toBe(2)
  })

  it('keeps a pinned card through leaving, until it is pinned again or dismissed', () => {
    const store = createPreviewStore()
    store.show(1, a)
    store.pin(1, a)
    expect(store.get()).toEqual({ id: 1, anchor: a, pinned: true })
    store.hide(1)
    expect(store.get()?.pinned).toBe(true)
    store.show(1, a)
    expect(store.get()?.pinned).toBe(true)
    store.pin(1, a)
    expect(store.get()).toBeNull()
    store.pin(1, a)
    store.dismiss()
    expect(store.get()).toBeNull()
  })

  it('lets hovering another card take over from a pinned one', () => {
    const store = createPreviewStore()
    store.pin(1, a)
    store.show(2, a)
    expect(store.get()).toEqual({ id: 2, anchor: a, pinned: false })
  })

  it('pins a different card without unpinning it first', () => {
    const store = createPreviewStore()
    store.pin(1, a)
    store.pin(2, a)
    expect(store.get()).toEqual({ id: 2, anchor: a, pinned: true })
  })
})

describe('rovingTarget', () => {
  it.each([
    ['ArrowRight', 2, 5, 3],
    ['ArrowRight', 4, 5, 4],
    ['ArrowDown', 0, 5, 1],
    ['ArrowLeft', 2, 5, 1],
    ['ArrowLeft', 0, 5, 0],
    ['ArrowUp', 3, 5, 2],
    ['Home', 3, 5, 0],
    ['End', 1, 5, 4],
  ])('moves %s from %i of %i to %i', (key, from, count, to) => {
    expect(rovingTarget(key, from, count)).toBe(to)
  })

  it('ignores other keys and an empty row', () => {
    expect(rovingTarget('a', 1, 5)).toBeNull()
    expect(rovingTarget('Enter', 1, 5)).toBeNull()
    expect(rovingTarget('ArrowRight', 0, 0)).toBeNull()
  })
})

describe('objectIdOf', () => {
  it('reads the id from the nearest card element', () => {
    const outer = document.createElement('button')
    outer.dataset.objectId = '42'
    const inner = document.createElement('span')
    outer.append(inner)
    expect(objectIdOf(inner)).toBe(42)
    expect(objectIdOf(outer)).toBe(42)
  })

  it.each([[undefined], ['x'], ['']])('is null for the id %j', (value) => {
    const element = document.createElement('button')
    if (value !== undefined) element.dataset.objectId = value
    expect(objectIdOf(element)).toBeNull()
  })
})

describe('parseDevSearch', () => {
  it.each([
    [{ seats: 2 }, { seats: 2 }],
    [
      { seats: 12, mode: 'grid' },
      { seats: 12, mode: 'grid' },
    ],
    [
      { seats: 5, mode: 'focus', focus: 3 },
      { seats: 5, mode: 'focus', focus: 3 },
    ],
    [{}, { seats: 4 }],
    [{ seats: 1 }, { seats: 4 }],
    [{ seats: 13 }, { seats: 4 }],
    [{ seats: 'four' }, { seats: 4 }],
    [{ seats: 3.5 }, { seats: 4 }],
    [{ seats: 6, mode: 'wide' }, { seats: 6 }],
    [{ seats: 6, focus: -1 }, { seats: 6 }],
    [{ seats: 6, focus: '2' }, { seats: 6 }],
    [
      { seats: 2, prompt: 'priority' },
      { seats: 2, prompt: 'priority' },
    ],
    [{ seats: 2, prompt: 7 }, { seats: 2 }],
    [
      { seats: 2, turn: 'you' },
      { seats: 2, turn: 'you' },
    ],
  ])('reads %j as %j', (search, parsed) => {
    expect(parseDevSearch(search)).toMatchObject(parsed)
  })

  it.each([undefined, 'them', 'YOU', 1, true])('leaves out the turn %j, which is not "you"', (turn) => {
    expect(parseDevSearch({ seats: 2, turn }).turn).toBeUndefined()
  })

  it('leaves out a prompt that is not a word', () => {
    expect(parseDevSearch({ prompt: 7 }).prompt).toBeUndefined()
    expect(parseDevSearch({}).prompt).toBeUndefined()
  })

  it('leaves out a mode and a focus it did not understand', () => {
    const parsed = parseDevSearch({ seats: 6, mode: 'wide', focus: -1 })
    expect(parsed.mode).toBeUndefined()
    expect(parsed.focus).toBeUndefined()
  })
})
