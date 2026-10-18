// Goal: a sideways strip scrolls just far enough to show its active item, in either direction, and not at all when the item is already in view or not active.
import { render } from '@testing-library/react'
import { useRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { useKeepInView } from './useKeepInView'

function Strip({ active }: { active: boolean }) {
  const item = useRef<HTMLButtonElement>(null)
  useKeepInView(item, active)
  return (
    <ul data-testid="strip">
      <li data-testid="slot">
        <button ref={item} type="button">
          tile
        </button>
      </li>
    </ul>
  )
}

interface Geometry {
  offsetLeft: number
  offsetWidth: number
  scrollLeft: number
  clientWidth: number
}

/** Renders an inactive strip, gives its elements this geometry, then makes the item active. */
function activate(geometry: Geometry): ReturnType<typeof vi.fn> {
  const view = render(<Strip active={false} />)
  const strip = view.getByTestId('strip')
  const slot = view.getByTestId('slot')
  const scrollTo = vi.fn()
  Object.defineProperties(slot, {
    offsetLeft: { value: geometry.offsetLeft },
    offsetWidth: { value: geometry.offsetWidth },
  })
  Object.defineProperties(strip, {
    scrollLeft: { value: geometry.scrollLeft },
    clientWidth: { value: geometry.clientWidth },
    scrollTo: { value: scrollTo },
  })
  view.rerender(<Strip active />)
  return scrollTo
}

describe('useKeepInView', () => {
  it('scrolls back to an item that is off the left edge, to its left edge', () => {
    const scrollTo = activate({ offsetLeft: 10, offsetWidth: 150, scrollLeft: 100, clientWidth: 300 })
    expect(scrollTo).toHaveBeenCalledExactlyOnceWith({ left: 10 })
  })

  it('scrolls forward to an item that is off the right edge, to its right edge', () => {
    const scrollTo = activate({ offsetLeft: 400, offsetWidth: 100, scrollLeft: 0, clientWidth: 300 })
    expect(scrollTo).toHaveBeenCalledExactlyOnceWith({ left: 200 })
  })

  it('leaves the strip alone when the item is fully in view', () => {
    const scrollTo = activate({ offsetLeft: 120, offsetWidth: 100, scrollLeft: 100, clientWidth: 300 })
    expect(scrollTo).not.toHaveBeenCalled()
  })

  it('leaves the strip alone while the item is not the active one', () => {
    const view = render(<Strip active={false} />)
    const scrollTo = vi.fn()
    Object.defineProperties(view.getByTestId('strip'), {
      scrollTo: { value: scrollTo },
      scrollLeft: { value: 500 },
    })
    Object.defineProperty(view.getByTestId('slot'), 'offsetLeft', { value: 0 })
    view.rerender(<Strip active={false} />)
    expect(scrollTo).not.toHaveBeenCalled()
  })
})
