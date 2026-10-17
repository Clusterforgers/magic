import { useDraggable, useDroppable } from '@dnd-kit/core'
import { motion } from 'motion/react'
import { memo } from 'react'
import { cn } from '@/lib/utils'
import { useGameStore, useObject } from '@/game/store'
import type { ObjectId } from '@/game/types'

interface CardProps {
  id: ObjectId
  onActivate: (id: ObjectId) => void
}

/** One card or permanent; subscribes only to its own object and its targeting state. */
export const Card = memo(function Card({ id, onActivate }: CardProps) {
  const object = useObject(id)
  const legal = useGameStore((state) => state.decision?.legal.has(id) ?? false)
  const dimmedByChoice = useGameStore(
    (state) => state.decision !== null && state.decision.source !== id && !state.decision.legal.has(id),
  )
  const setHovered = useGameStore((state) => state.setHovered)
  const draggable = useDraggable({ id, disabled: !object?.playable })
  const droppable = useDroppable({ id, disabled: !legal })

  if (!object) return null
  const dragProps = object.playable ? { ...draggable.listeners, ...draggable.attributes } : { role: 'button' }

  return (
    <motion.div
      layout="position"
      layoutId={`card-${id}`}
      ref={(node: HTMLDivElement | null) => {
        draggable.setNodeRef(node)
        droppable.setNodeRef(node)
      }}
      data-testid={`card-${id}`}
      data-legal={legal}
      data-playable={object.playable}
      onClick={() => onActivate(id)}
      onPointerEnter={() => {
        performance.mark('card-hover')
        setHovered(id)
      }}
      onPointerLeave={() => setHovered(null)}
      {...dragProps}
      className={cn(
        'relative flex h-20 w-14 shrink-0 select-none flex-col justify-between rounded-md border bg-card p-1 text-[9px] leading-tight',
        object.tapped && 'rotate-90',
        object.attacking && '-translate-y-3',
        object.playable && 'ring-2 ring-emerald-400',
        legal && 'ring-2 ring-amber-400',
        dimmedByChoice && 'opacity-40',
      )}
    >
      <span className="truncate font-medium">{object.name}</span>
      <span className="self-end tabular-nums">
        {object.power}/{object.toughness - object.damage}
      </span>
    </motion.div>
  )
})
