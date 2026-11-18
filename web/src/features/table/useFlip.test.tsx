/** Goal: a list's item slides only when a draw moves it inside the list, from where it stood on screen just before that draw; a list moved as a whole, or a table that settled after the last draw, slides nothing, so a pressed card never runs from under the pointer. */
import { render } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useFlip } from './useFlip'

type Places = Record<string, { x: number; y: number }>

/** Where the list (`root`) and each item stand on screen, as the browser would lay them out now. */
let places: Places = {}
let animate: ReturnType<typeof vi.fn>

/** Two items in a list; `after` is the layout the draw's DOM changes give, read by the effects after the render. */
function List({ draw, after }: { draw: number; after: Places }) {
  const list = useRef<HTMLUListElement>(null)
  useFlip(list)
  places = after
  return (
    <ul ref={list} data-place="root" data-draw={draw}>
      {['a', 'b'].map((key) => (
        <li key={key} data-flip={key} data-place={key} />
      ))}
    </ul>
  )
}

const row = (y: number, a = 0, b = 110): Places => ({ root: { x: 0, y }, a: { x: a, y }, b: { x: b, y } })
const SETTLED = row(464)

beforeEach(() => {
  places = SETTLED
  animate = vi.fn(() => ({ id: 'flip', cancel: vi.fn() }))
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const at = places[this.getAttribute('data-place') ?? ''] ?? { x: 0, y: 0 }
    return new DOMRect(at.x, at.y, 100, 140)
  })
  Element.prototype.animate = animate as unknown as Element['animate']
  Element.prototype.getAnimations = () => []
})
afterEach(() => vi.restoreAllMocks())

/** The transform each slide of the last draw starts from. */
const slides = (): unknown[] => animate.mock.calls.map(([frames]) => (frames as Keyframe[])[0]?.transform)

describe('useFlip', () => {
  it.each<[string, Places, unknown[]]>([
    ['a press redraws the list and nothing moved', SETTLED, []],
    ['the draw moves the whole list', row(164), []],
    ['the draw swaps the items', row(464, 110, 0), ['translate(-110px, 0px)', 'translate(110px, 0px)']],
  ])('%s', (_, after, expected) => {
    const { rerender } = render(<List draw={0} after={SETTLED} />)
    // A draw whose effects run while the table around the list is still on its way (far up, laid out in another order).
    rerender(<List draw={1} after={row(-742, 370, 0)} />)
    // The table settles without a draw of the list; then a press draws it again.
    places = SETTLED
    animate.mockClear()
    rerender(<List draw={2} after={after} />)
    expect(slides()).toEqual(expected)
  })
})
