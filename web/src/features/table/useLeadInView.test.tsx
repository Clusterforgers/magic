// Goal: a sideways strip scrolls back to its start when the item that leads it changes, so the lead is in view, and never when it is drawn the first time, when the lead stays, or when the strip is already at its start.
import { render } from '@testing-library/react'
import { useRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { useLeadInView } from './useLeadInView'

function Strip({ lead }: { lead: number | undefined }) {
  const strip = useRef<HTMLUListElement>(null)
  useLeadInView(strip, lead)
  return <ul data-testid="strip" ref={strip} />
}

/** Renders a strip led by `first`, gives it a scroll position, then leads it with `next`. */
function lead(
  first: number | undefined,
  next: number | undefined,
  scrollLeft: number,
): ReturnType<typeof vi.fn> {
  const view = render(<Strip lead={first} />)
  const scrollTo = vi.fn()
  Object.defineProperties(view.getByTestId('strip'), {
    scrollLeft: { value: scrollLeft },
    scrollTo: { value: scrollTo },
  })
  view.rerender(<Strip lead={next} />)
  return scrollTo
}

describe('useLeadInView', () => {
  it.each([
    [1, 4, 120],
    [4, 1, 1],
    [1, undefined, 300],
    [undefined, 2, 300],
  ])(
    'scrolls a strip led by %s and now led by %s, at %i px, back to its start',
    (first, next, scrollLeft) => {
      expect(lead(first, next, scrollLeft)).toHaveBeenCalledExactlyOnceWith({ left: 0 })
    },
  )

  it('leaves the strip alone while the same item leads it', () => {
    expect(lead(3, 3, 200)).not.toHaveBeenCalled()
  })

  it('leaves a strip that is at its start alone, as the lead is in view', () => {
    expect(lead(1, 4, 0)).not.toHaveBeenCalled()
  })

  it('does not move the strip when it is drawn, so a tile that was scrolled to stays in view', () => {
    const scrollTo = vi.fn()
    const original = Object.getOwnPropertyDescriptor(Element.prototype, 'scrollTo')
    Object.defineProperty(Element.prototype, 'scrollTo', { value: scrollTo, configurable: true })
    try {
      render(<Strip lead={1} />)
      expect(scrollTo).not.toHaveBeenCalled()
    } finally {
      if (original === undefined) Reflect.deleteProperty(Element.prototype, 'scrollTo')
      else Object.defineProperty(Element.prototype, 'scrollTo', original)
    }
  })
})
