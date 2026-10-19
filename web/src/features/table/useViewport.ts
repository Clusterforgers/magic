import { useEffect, useState } from 'react'
import type { Viewport } from './layout'

function readWindow(): Viewport {
  return { width: window.innerWidth, height: window.innerHeight }
}

/** The window's size in CSS px, kept current while the window is resized or turned; a caller may fix a size instead. */
export function useViewport(fixed?: Viewport): Viewport {
  const [size, setSize] = useState(readWindow)
  useEffect(() => {
    if (fixed !== undefined) return
    const update = (): void => {
      const next = readWindow()
      setSize((was) => (was.width === next.width && was.height === next.height ? was : next))
    }
    window.addEventListener('resize', update)
    // The window may have changed between the render and this effect.
    update()
    return () => window.removeEventListener('resize', update)
  }, [fixed])
  return fixed ?? size
}
