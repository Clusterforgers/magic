//! Goal: a combat line that would run over your life figure bows to its other side (further where that is not enough), and a line clear of it keeps its shape.
import { describe, expect, it } from 'vitest'
import { connect } from './geometry'
import type { Rect } from './geometry'
import type { LineSpec } from './plan'
import { crosses, shapesOf } from './rects'

const box = (x: number, y: number, width = 40, height = 40): Rect => ({ x, y, width, height })

describe('crosses', () => {
  it('tells a curve through a box from one beside it', () => {
    const shape = connect(box(0, 0), box(400, 0), { bow: 0 })
    expect(shape).not.toBeNull()
    expect(crosses(shape!, box(180, 0))).toBe(true)
    expect(crosses(shape!, box(180, 200))).toBe(false)
  })
})

describe('shapesOf with your life to keep clear', () => {
  const line: LineSpec = {
    kind: 'block',
    key: 'block:1',
    from: { kind: 'card', id: 1 },
    to: { kind: 'card', id: 2 },
    mine: true,
    pending: false,
  } as unknown as LineSpec
  const rects = new Map([
    ['card:1', box(0, 0)],
    ['card:2', box(400, 0)],
  ])

  it('bows a line that would cross it to its other side', () => {
    const [plain] = shapesOf([line], rects)
    expect(plain).toBeDefined()
    const life = box(195, plain!.shape.mid.y - 4, 10, 8)
    expect(crosses(plain!.shape, life)).toBe(true)
    const [kept] = shapesOf([line], rects, life)
    expect(crosses(kept!.shape, life)).toBe(false)
  })

  it('keeps a line clear of it as it was', () => {
    const [plain] = shapesOf([line], rects)
    const [kept] = shapesOf([line], rects, box(0, 400))
    expect(kept?.shape.d).toBe(plain?.shape.d)
  })
})
