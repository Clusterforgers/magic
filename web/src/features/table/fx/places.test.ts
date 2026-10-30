// Goal: a place of a move is found on screen in a fixed order (the card itself, else the anchor of its zone in the hand dock, the shared strip, a pod or a tile, else the seat), the stack is always a chip, an element that is not on screen is passed over, and a place the page does not show is none.
import { beforeEach, describe, expect, it } from 'vitest'
import { place, setBox } from '@/test-utils/fxDom'
import type { TestBox } from '@/test-utils/fxDom'
import { measurePlace, STACK_REACH_PX, zoneSelectors } from './places'
import type { AnchorZone, FxPlace } from './moves'

const BOX: TestBox = { x: 10, y: 20, width: 100, height: 140 }
const OTHER: TestBox = { x: 300, y: 400, width: 50, height: 30 }

let page: HTMLElement
beforeEach(() => {
  page = document.createElement('main')
  document.body.replaceChildren(page)
})

const card = (object: number, seat = 0): FxPlace => ({ seat, zone: 'hand', object })

describe('a card', () => {
  it('is the element with its object id, the card itself', () => {
    place(page, { 'data-object-id': '5' }, BOX, 'button')
    expect(measurePlace(page, card(5))).toEqual({
      box: { x: 10, y: 20, width: 100, height: 140 },
      card: true,
    })
  })

  it('is measured by its face when it lies on its side', () => {
    const button = place(page, { 'data-object-id': '5' }, BOX, 'button')
    place(button, { 'data-slot': 'card-face' }, OTHER)
    expect(measurePlace(page, card(5))?.box).toEqual({ x: 300, y: 400, width: 50, height: 30 })
  })

  it('gives way to its zone when the table does not draw it', () => {
    place(page, { 'data-fx-anchor': '0:hand' }, OTHER)
    expect(measurePlace(page, card(5))).toEqual({
      box: { x: 300, y: 400, width: 50, height: 30 },
      card: false,
    })
  })

  it.each([
    ['no size', { x: 0, y: 0, width: 0, height: 0 }],
    ['all of it left of the page', { x: -200, y: 10, width: 100, height: 100 }],
    ['all of it below the page', { x: 10, y: 5000, width: 100, height: 100 }],
  ])('is passed over with %s, for the next element that has one', (_name, hidden) => {
    place(page, { 'data-object-id': '5' }, hidden, 'button')
    place(page, { 'data-object-id': '5' }, BOX, 'button')
    expect(measurePlace(page, card(5))?.box).toEqual(BOX)
  })
})

describe('the stack', () => {
  const stack = (object?: number): FxPlace => ({ stack: true, ...(object === undefined ? {} : { object }) })

  it('is the chip of the object inside the stack area, never a copy outside it', () => {
    place(page, { 'data-object-id': '5' }, OTHER, 'button')
    const area = place(page, { 'data-fx-anchor': 'stack' }, { x: 0, y: 0, width: 800, height: 60 })
    place(area, { 'data-object-id': '5' }, BOX, 'button')
    expect(measurePlace(page, stack(5))).toEqual({ box: BOX, card: false })
  })

  it('is the start of its area when no chip is there', () => {
    place(page, { 'data-fx-anchor': 'stack' }, { x: 40, y: 8, width: 800, height: 60 })
    expect(measurePlace(page, stack(5))).toEqual({
      box: { x: 40, y: 8, width: STACK_REACH_PX, height: 60 },
      card: false,
    })
  })

  it('is the whole of a narrow area', () => {
    place(page, { 'data-fx-anchor': 'stack' }, { x: 0, y: 0, width: 90, height: 60 })
    expect(measurePlace(page, stack())?.box.width).toBe(90)
  })

  it('is none when the page has no stack area', () => {
    place(page, { 'data-object-id': '5' }, BOX, 'button')
    expect(measurePlace(page, stack(5))).toBeNull()
  })
})

describe('a zone of a seat', () => {
  const zone = (seat: number, name: AnchorZone): FxPlace => ({ seat, zone: name })

  it('lists the anchor of the hand dock first, then the chips of the pod and the tile, then the seat itself', () => {
    expect(zoneSelectors(2, 'graveyard')).toEqual([
      '[data-fx-anchor="2:graveyard"]',
      '[data-seat="2"] [data-zone="graveyard"]',
      '[data-tile-seat="2"] [data-zone="graveyard"]',
      '[data-seat="2"]',
      '[data-tile-seat="2"]',
    ])
    expect(zoneSelectors(2, 'battlefield')).toContain('[data-seat-area="2"]')
  })

  it('is the hand dock for the viewer and the pod chip for another seat', () => {
    place(page, { 'data-fx-anchor': '0:hand' }, BOX)
    const pod = place(page, { 'data-seat': '1' }, { x: 0, y: 0, width: 168, height: 300 })
    place(pod, { 'data-zone': 'hand' }, OTHER, 'span')
    expect(measurePlace(page, zone(0, 'hand'))?.box).toEqual(BOX)
    expect(measurePlace(page, zone(1, 'hand'))?.box).toEqual(OTHER)
  })

  it('is the chip of a tile when the seat has no pod', () => {
    const tile = place(page, { 'data-tile-seat': '3' }, { x: 0, y: 0, width: 200, height: 60 }, 'button')
    place(tile, { 'data-zone': 'graveyard' }, OTHER, 'span')
    expect(measurePlace(page, zone(3, 'graveyard'))?.box).toEqual(OTHER)
  })

  it('is the seat itself when its pod draws no chip for the zone', () => {
    place(page, { 'data-seat': '1' }, BOX)
    expect(measurePlace(page, zone(1, 'exile'))).toEqual({ box: BOX, card: false })
  })

  it('is the battlefield area for the battlefield, before the pod', () => {
    place(page, { 'data-seat': '1' }, OTHER)
    place(page, { 'data-seat-area': '1' }, BOX)
    expect(measurePlace(page, zone(1, 'battlefield'))?.box).toEqual(BOX)
  })

  it('is none for a seat the page does not show', () => {
    place(page, { 'data-seat': '1' }, BOX)
    expect(measurePlace(page, zone(2, 'hand'))).toBeNull()
  })

  it('follows the box the page gives now, not the one it gave before', () => {
    const chip = place(page, { 'data-fx-anchor': '0:hand' }, BOX)
    setBox(chip, OTHER)
    expect(measurePlace(page, zone(0, 'hand'))?.box).toEqual(OTHER)
  })
})
