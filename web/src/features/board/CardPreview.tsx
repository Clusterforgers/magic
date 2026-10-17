import { useEffect } from 'react'
import { useGameStore } from '@/game/store'

/** Large preview of the hovered card; records hover-to-paint latency as a User Timing measure. */
export function CardPreview() {
  const object = useGameStore((state) => (state.hovered === null ? undefined : state.objects[state.hovered]))
  const shownId = object?.id

  useEffect(() => {
    if (shownId === undefined) return
    const frame = requestAnimationFrame(() => performance.measure('hover-latency', 'card-hover'))
    return () => cancelAnimationFrame(frame)
  }, [shownId])

  if (!object) return null
  return (
    <aside
      data-testid="card-preview"
      className="fixed bottom-4 right-4 z-10 flex h-72 w-52 flex-col justify-between rounded-xl border bg-popover p-3 shadow-lg"
    >
      <h3 className="font-semibold">{object.name}</h3>
      <p className="self-end text-xl tabular-nums">
        {object.power}/{object.toughness}
      </p>
    </aside>
  )
}
