import type { Target } from './anchors'
import type { Box } from './resolve'
import './coach.css'

/** The scrim over the table with a hole at each lit box; it takes no pointer. */
function Scrim({ holes, size }: { holes: readonly Box[]; size: { width: number; height: number } }) {
  const path = [
    `M0 0H${size.width}V${size.height}H0Z`,
    ...holes.map((box) => `M${box.x} ${box.y}v${box.height}h${box.width}v${-box.height}Z`),
  ].join('')
  return (
    <svg aria-hidden="true" className="pointer-events-none fixed inset-0 size-full" data-slot="coach-scrim">
      <path d={path} fillRule="evenodd" className="fill-scrim opacity-60" />
    </svg>
  )
}

/** The coach's light: the scrim with its holes, a ring on each lit box and the pointer that bobs toward the first. */
export function Light({
  rings,
  targets,
  size,
}: {
  rings: readonly Box[]
  /** The target of each ring, which keys it. */
  targets: readonly Target[]
  size: { width: number; height: number }
}) {
  const pointer = rings[0]
  const below = pointer !== undefined && pointer.y < 24
  return (
    <>
      <Scrim holes={rings} size={size} />
      {rings.map((ring, at) => (
        <div
          key={`${targets[at] ?? at}`}
          aria-hidden="true"
          data-slot="coach-ring"
          className="coach-ring pointer-events-none fixed rounded-lg"
          style={{ left: ring.x, top: ring.y, width: ring.width, height: ring.height }}
        />
      ))}
      {pointer === undefined ? null : (
        <div
          aria-hidden="true"
          data-slot="coach-pointer"
          data-side={below ? 'below' : 'above'}
          className="coach-pointer pointer-events-none fixed size-0"
          style={{
            left: pointer.x + pointer.width / 2 - 8,
            top: below ? pointer.y + pointer.height + 4 : pointer.y - 14,
          }}
        />
      )}
    </>
  )
}
