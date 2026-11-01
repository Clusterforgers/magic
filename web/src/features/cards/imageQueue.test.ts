// Goal: the image queue grants at most `limit` slots at once, in the order asked, frees a slot on done, and drops a request that left the queue.
import { describe, expect, it } from 'vitest'
import { createImageQueue } from './imageQueue'

describe('the image queue', () => {
  it.each([
    [1, 5, 1],
    [4, 10, 4],
    [4, 2, 2],
    [0, 3, 1],
  ])('with %i slots and %i requests holds %i at once', (limit, asked, held) => {
    const queue = createImageQueue(limit)
    for (let i = 0; i < asked; i += 1) queue.request(() => undefined)
    expect(queue.active).toBe(held)
    expect(queue.waiting).toBe(asked - held)
  })

  it('grants waiting requests in the order asked as slots free', () => {
    const queue = createImageQueue(1)
    const order: number[] = []
    const tickets = [1, 2, 3].map((n) => queue.request(() => order.push(n)))
    expect(order).toEqual([1])
    tickets[0]?.done()
    expect(order).toEqual([1, 2])
    tickets[1]?.done()
    expect(order).toEqual([1, 2, 3])
  })

  it('frees a slot once however often done is called', () => {
    const queue = createImageQueue(1)
    const first = queue.request(() => undefined)
    queue.request(() => undefined)
    first.done()
    first.done()
    expect(queue.active).toBe(1)
    expect(queue.waiting).toBe(0)
  })

  it('lets a waiting request leave by cancel, but a granted one keeps its slot', () => {
    const queue = createImageQueue(1)
    const granted = queue.request(() => undefined)
    const waiting = queue.request(() => undefined)
    waiting.cancel()
    granted.cancel()
    expect(queue.waiting).toBe(0)
    expect(queue.active).toBe(1)
  })
})
