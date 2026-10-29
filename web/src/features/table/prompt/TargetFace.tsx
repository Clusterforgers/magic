import { usePlay } from '@/features/game/gameContext'
import type { ObjectId } from '@/protocol/generated'
import { CardFace } from '../CardFace'
import { usePreviewHandlers } from '../usePreviewHandlers'

/**
 * A small picture of an object a prompt names (a target of a division): the card's face, or a face-down stand-in while
 * the view does not hold it. Hover shows the card large; it is not a control, so it carries `data-candidate` and never
 * `data-object-id`, which belongs to the table's card of the same object.
 */
export function TargetFace({ id }: { id: ObjectId }) {
  const object = usePlay((state) => state.objectsById[id])
  const preview = usePreviewHandlers(id)
  return (
    <span
      data-slot="target-face"
      data-candidate={id}
      onPointerEnter={preview.onPointerEnter}
      onPointerLeave={preview.onPointerLeave}
      className="relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-md ring-1 ring-border-strong [--card-h:3.85rem] [--card-w:2.75rem]"
    >
      {object === undefined ? (
        <span
          data-slot="card-face"
          data-hidden=""
          className="grid size-full place-items-center rounded-md bg-muted text-xs text-muted-foreground"
        >
          Card
        </span>
      ) : (
        <CardFace object={object} role={null} size="compact" fit />
      )}
    </span>
  )
}
