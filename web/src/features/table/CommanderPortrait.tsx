/** A seat's commander as a small art crop beside its name, so a pod reads as that player at a glance. */
import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cardImageUrl } from '@/features/cards/images'
import { cn } from '@/lib/utils'
import type { CardIdentity, ObjectId, ObjectView } from '@/protocol/generated'

/** The identities of the first two of these objects that have one, in order: a commander and its partner. */
export function portraitIdentities(
  ids: readonly ObjectId[],
  objects: Readonly<Record<ObjectId, Pick<ObjectView, 'identity'> | undefined>>,
): CardIdentity[] {
  return ids
    .map((id) => objects[id]?.identity ?? undefined)
    .filter((identity) => identity !== undefined)
    .slice(0, 2)
}

/**
 * The art crops of a seat's commanders (two side by side for partners) in a square edged in the seat's hue, which the
 * pod sets as `--seat-hue`. Decoration: the command zone chip names the commanders. Nothing is drawn for a seat whose
 * commanders have no picture, or when the picture fails.
 */
export function CommanderPortrait({ ids, className }: { ids: readonly ObjectId[]; className?: string }) {
  const identities = usePlay(useShallow((state) => portraitIdentities(ids, state.objectsById)))
  const [missed, setMissed] = useState(false)
  if (identities.length === 0 || missed) return null
  return (
    <span
      aria-hidden="true"
      data-slot="commander-portrait"
      className={cn(
        'flex size-10 shrink-0 overflow-hidden rounded-md ring-2 ring-[oklch(0.6_0.14_var(--seat-hue))] dark:ring-[oklch(0.74_0.13_var(--seat-hue))]',
        className,
      )}
    >
      {identities.map((identity) => (
        <img
          key={identity.oracle_id}
          src={cardImageUrl({ kind: 'card', identity }, 0, 'art_crop')}
          alt=""
          loading="lazy"
          decoding="async"
          onError={() => setMissed(true)}
          className="size-full min-w-0 flex-1 object-cover"
        />
      ))}
    </span>
  )
}
