import { useLayoutEffect, useMemo, useRef } from 'react'
import type { CSSProperties } from 'react'
import { useSeatNamer } from '../names'
import { distinctTargets, targetKey } from '../prompt/targetText'
import type { StackEntry } from '../stackTargets'
import { targetsLine } from '../stackText'
import { useTargetNamer } from '../useStackEntries'
import { CardPicture } from './CardPicture'
import { spotlightKeyframes } from './spotlight'
import type { Spotlight as SpotlightPlan } from './spotlight'
import type { Travel } from './travel'

/** The targets of the spell as the chip words them ("Hill Giant, Bo"); nothing for a spell that aims at nothing. */
function useTargetsLine(spotlight: SpotlightPlan): string {
  const names = useTargetNamer()
  return useMemo(() => {
    const targets = distinctTargets([...spotlight.targets])
    const entry: StackEntry = {
      id: spotlight.object ?? 0,
      controller: spotlight.caster,
      targets: targets.map((target) => ({ key: targetKey(target), target, gone: false, onStack: false })),
    }
    return targetsLine(entry, names)
  }, [spotlight, names])
}

/**
 * The spell of a cast by another seat, or of one that passes through the stack inside the batch, standing large over
 * the shared strip: the card at a readable size and under it who casts what and at what. It comes in and goes out in
 * 150 ms each within `--fx-spotlight-ms`; under reduced motion it appears and goes without motion for the same time.
 * A card the viewer cannot identify shows its back. The picture takes no pointer and is gone when the time is up. The
 * viewer's own cast (`own`) stands shorter, in a frame of lower contrast, and says "You cast".
 */
export function Spotlight({ spotlight, travel }: { spotlight: SpotlightPlan; travel: Travel }) {
  const root = useRef<HTMLDivElement>(null)
  const seat = useSeatNamer()
  const aims = useTargetsLine(spotlight)
  const { box, card, look, faceDown, still, own } = spotlight
  useLayoutEffect(() => {
    const element = root.current
    if (element === null || typeof element.animate !== 'function') {
      travel.done(spotlight.key)
      return undefined
    }
    const timing = { duration: spotlight.duration, delay: spotlight.delay, fill: 'both' } as const
    const playing = element.animate(spotlightKeyframes(spotlight), timing)
    playing.onfinish = () => travel.done(spotlight.key)
    return () => playing.cancel()
  }, [spotlight, travel])
  const name = faceDown || look === null ? 'a card' : look.name
  const style: CSSProperties = { left: box.x, top: box.y, width: box.width }
  return (
    <div
      ref={root}
      data-slot="fx-spotlight"
      data-still={still || undefined}
      data-own={own || undefined}
      className="fx-spotlight"
      style={style}
    >
      <CardPicture
        look={faceDown ? null : look}
        size={{ width: card.width, height: card.height }}
        back={faceDown}
      />
      <div data-slot="fx-spotlight-caption" className="fx-spotlight-caption">
        <p className="truncate text-xs text-muted-foreground">
          {own ? 'You cast' : `${seat(spotlight.caster)} casts`}
        </p>
        <p className="truncate text-sm font-semibold">{name}</p>
        {aims === '' ? null : <p className="truncate text-xs">→ {aims}</p>}
      </div>
    </div>
  )
}
