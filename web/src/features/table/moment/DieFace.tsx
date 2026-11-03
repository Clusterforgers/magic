import { useEffect, useMemo, useState } from 'react'
import { cn } from '@/lib/utils'
import { dieShape, diePoints, ROLL_TICK_MS, rollFaces } from './moments'

/** The number a die shows now and whether it has landed: its tumbling faces one after the other and then the result; the result at once when `still`. */
export function useRollingFace(
  sides: number,
  result: number,
  rollKey: string,
  still: boolean,
): { face: number; landed: boolean } {
  const [tick, setTick] = useState(0)
  const faces = useMemo(() => rollFaces(sides, result, rollKey), [sides, result, rollKey])
  const last = faces.length - 1
  useEffect(() => {
    if (still || tick >= last) return undefined
    const timer = window.setTimeout(() => setTick((now) => Math.min(now + 1, last)), ROLL_TICK_MS)
    return () => window.clearTimeout(timer)
  }, [still, tick, last])
  const landed = still || tick >= last
  return { face: landed ? result : (faces[tick] ?? result), landed }
}

/**
 * A die that tumbles through faces and lands on its result (`data-landed`): an outline by its number of sides with the
 * number over it. The tumble is CSS (`moment.css`, only where motion is welcome) and the faces change a tick at a time;
 * a `still` die shows the result and nothing else. The number is text, so a screen reader and a test read it.
 */
export function DieFace({
  sides,
  result,
  rollKey,
  still,
}: {
  sides: number
  result: number
  rollKey: string
  still: boolean
}) {
  const { face, landed } = useRollingFace(sides, result, rollKey, still)
  const shape = dieShape(sides)
  const points = diePoints(shape)
  return (
    <span
      data-slot="die"
      data-shape={shape}
      data-sides={sides}
      data-result={result}
      data-still={still || undefined}
      data-landed={landed || undefined}
      className="relative grid size-32 place-items-center"
    >
      <svg
        aria-hidden="true"
        viewBox="0 0 100 100"
        className="die-body absolute inset-0 size-full fill-card stroke-primary drop-shadow-md"
        strokeWidth={4}
        strokeLinejoin="round"
      >
        {points === null ? <circle cx="50" cy="50" r="46" /> : <polygon points={points} />}
      </svg>
      <span
        data-slot="die-number"
        className={cn(
          'die-number relative font-bold text-foreground tabular-nums',
          String(face).length > 2 ? 'text-2xl' : 'text-4xl',
        )}
      >
        {face}
      </span>
    </span>
  )
}
