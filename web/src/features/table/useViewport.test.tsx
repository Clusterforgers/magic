// Goal: the table knows the window's size from the first render, follows a resize or a turn of the device, ignores a resize that changes nothing, and lets a caller fix a size instead (a test, a screenshot).
import { act, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import type { Viewport } from './layout'
import { useViewport } from './useViewport'

const original = { width: window.innerWidth, height: window.innerHeight }

function resizeTo(width: number, height: number): void {
  act(() => {
    window.innerWidth = width
    window.innerHeight = height
    window.dispatchEvent(new Event('resize'))
  })
}

afterEach(() => {
  window.innerWidth = original.width
  window.innerHeight = original.height
})

function Size({ fixed, onRender }: { fixed?: Viewport; onRender?: () => void }) {
  const { width, height } = useViewport(fixed)
  onRender?.()
  return <p data-testid="size">{`${width}x${height}`}</p>
}

describe('useViewport', () => {
  it('reads the window at the first render', () => {
    window.innerWidth = 1366
    window.innerHeight = 768
    render(<Size />)
    expect(screen.getByTestId('size')).toHaveTextContent('1366x768')
  })

  it('follows a resize and a turn of the device', () => {
    window.innerWidth = 1280
    window.innerHeight = 720
    render(<Size />)
    resizeTo(390, 844)
    expect(screen.getByTestId('size')).toHaveTextContent('390x844')
    resizeTo(844, 390)
    expect(screen.getByTestId('size')).toHaveTextContent('844x390')
  })

  it('does not render again for a resize that changed nothing', () => {
    window.innerWidth = 1280
    window.innerHeight = 720
    let renders = 0
    render(
      <Size
        onRender={() => {
          renders += 1
        }}
      />,
    )
    const before = renders
    resizeTo(1280, 720)
    resizeTo(1280, 720)
    expect(renders).toBe(before)
  })

  it('uses the size a caller fixes and ignores the window', () => {
    window.innerWidth = 1280
    window.innerHeight = 720
    render(<Size fixed={{ width: 1920, height: 1080 }} />)
    resizeTo(390, 844)
    expect(screen.getByTestId('size')).toHaveTextContent('1920x1080')
  })

  it('stops listening when the table goes', () => {
    window.innerWidth = 1280
    window.innerHeight = 720
    const { unmount } = render(<Size />)
    unmount()
    // A listener left behind would set state on an unmounted component and be reported by React.
    expect(() => resizeTo(390, 844)).not.toThrow()
  })
})
