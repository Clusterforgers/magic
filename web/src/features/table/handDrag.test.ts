// Goal: a drag of a hand card is classified by where the pointer is and what the card can do: along the hand it reorders, carried up out of the dock past the rise it plays a playable card and returns any other, and a drop that plays goes to the prompt's drop handler alone (never the click's, which may offer an ability), with nothing called for a card the prompt does not play on a drop.
import { describe, expect, it, vi } from 'vitest'
import { dragOutcome, dragZone, dropCard, eventY, PLAY_RISE_PX, playableBy } from './handDrag'
import type { DragOutcome, Drop } from './handDrag'
import { createMarksStore } from './prompt/marks'

describe('the zone of a drag', () => {
  it.each<[string, number, number, number, 'hand' | 'board']>([
    ['no move', 700, 700, 600, 'hand'],
    ['a move along the hand', 700, 690, 600, 'hand'],
    ['up but still in the dock', 700, 610, 600, 'hand'],
    ['just above the dock', 700, 599, 600, 'board'],
    ['far up the table', 700, 200, 600, 'board'],
    ['a lifted card grabbed above the dock, not risen', 590, 590 - PLAY_RISE_PX + 1, 600, 'hand'],
    ['a lifted card grabbed above the dock, risen', 590, 590 - PLAY_RISE_PX, 600, 'board'],
    ['down below the hand', 700, 760, 600, 'hand'],
    ['an unknown start', Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, 600, 'hand'],
  ])('%s', (_name, startY, y, dockTop, zone) => {
    expect(dragZone(startY, y, dockTop)).toBe(zone)
  })
})

describe('what a drop does', () => {
  const base: Drop = { zone: 'hand', playable: true, sortable: true, from: 2, to: 0 }
  it.each<[string, Partial<Drop>, DragOutcome]>([
    ['over another card of the hand it reorders', {}, { kind: 'reorder', from: 2, to: 0 }],
    ['a card that cannot be played still reorders', { playable: false }, { kind: 'reorder', from: 2, to: 0 }],
    ['over its own place it does nothing', { to: 2 }, { kind: 'cancel' }],
    ['over no card it does nothing', { to: -1 }, { kind: 'cancel' }],
    ['a card at the head of the dock takes no new place', { sortable: false }, { kind: 'cancel' }],
    ['on the table a playable card plays', { zone: 'board' }, { kind: 'play' }],
    ['on the table it plays whatever card lies nearest', { zone: 'board', to: -1 }, { kind: 'play' }],
    ['a head card plays from the table too', { zone: 'board', sortable: false }, { kind: 'play' }],
    [
      'on the table a card that cannot be played returns',
      { zone: 'board', playable: false },
      { kind: 'cancel' },
    ],
  ])('%s', (_name, change, outcome) => {
    expect(dragOutcome({ ...base, ...change })).toEqual(outcome)
  })
})

describe('a drop on the table', () => {
  it.each<[string, number[] | null, boolean]>([
    ['a card the prompt plays on a drop', [7], true],
    ['a legal card with only an ability to activate', [8], false],
    ['a prompt that plays no card on a drop', [], false],
    ['a prompt that says nothing of drops (a pick among the hand)', null, false],
  ])('%s', (_name, drops, plays) => {
    const marks = createMarksStore()
    const onSelect = vi.fn()
    const onDrop = vi.fn()
    // The card is legal for a click in every row: only the drops decide.
    marks.publish(new Map([[7, 'legal']]), onSelect, false)
    if (drops !== null) marks.publishDrops(new Set(drops), onDrop)
    const element = document.createElement('button')
    expect(playableBy(marks, 7)).toBe(plays)
    expect(dropCard(marks, 7, element)).toBe(plays)
    expect(onDrop.mock.calls).toEqual(plays ? [[7, element]] : [])
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('plays nothing once the prompt has left', () => {
    const marks = createMarksStore()
    const onDrop = vi.fn()
    marks.publishDrops(new Set([7]), onDrop)
    marks.publishDrops(new Set(), undefined)
    expect(dropCard(marks, 7, undefined)).toBe(false)
    expect(onDrop).not.toHaveBeenCalled()
  })
})

describe('where an event happened', () => {
  it('reads a mouse, a touch and nothing', () => {
    expect(eventY(new MouseEvent('mousedown', { clientY: 412 }))).toBe(412)
    expect(eventY({ touches: [{ clientY: 77 }] } as unknown as Event)).toBe(77)
    expect(eventY({ touches: [] } as unknown as Event)).toBeNull()
    expect(eventY(new Event('keydown'))).toBeNull()
    expect(eventY(null)).toBeNull()
  })
})
