import { FxMark } from './fx/FxMark'
import { useCardFx } from './fx/hooks'
import { isMarkFx } from './fx/model'
import type { Ghost } from './fx/model'

/**
 * The chip of an ability that has just left the stack, drawn where it stood while its end mark plays. It is
 * `aria-hidden` and has no `data-object-id`: the game has already moved on.
 */
export function StackGhostChip({ ghost }: { ghost: Ghost }) {
  const fx = useCardFx(ghost.object.id)
  return (
    <li aria-hidden="true" data-stack-ghost={ghost.object.id} className="shrink-0">
      <span className="relative flex h-8 max-w-48 items-center gap-1.5 rounded-lg border bg-background px-2 text-sm font-medium">
        <span className="truncate">{ghost.object.name}</span>
        {fx.map((entry) =>
          isMarkFx(entry.kind) ? <FxMark key={entry.token} kind={entry.kind} inline /> : null,
        )}
      </span>
    </li>
  )
}
