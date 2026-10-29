import { CircleAlertIcon, PencilIcon, SaveIcon } from 'lucide-react'
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { SaveStatus } from './saveStatus'

interface SaveBarProps {
  status: SaveStatus
  onSave: () => void
}

/**
 * The foot of the deck pane, always in the window: what saving would do or what stands in the way, and Save. Save
 * stays focusable when there is nothing to save (`aria-disabled`) and is described by the sentence beside it.
 */
export function SaveBar({ status, onSave }: SaveBarProps) {
  const id = useId()
  const blocked = status.kind === 'blocked'
  return (
    <div
      data-slot="save-bar"
      data-status={status.kind}
      className="sticky bottom-0 z-10 flex items-center justify-between gap-3 rounded-b-[inherit] border-t bg-surface p-3"
    >
      <p
        id={id}
        className={cn(
          'flex min-w-0 items-start gap-1.5 text-sm',
          blocked ? 'text-destructive' : 'text-muted-foreground',
        )}
      >
        {blocked ? (
          <CircleAlertIcon aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
        ) : status.kind === 'ready' ? (
          <PencilIcon aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
        ) : null}
        <span>{status.text}</span>
      </p>
      <Button
        type="button"
        loading={status.kind === 'saving'}
        aria-disabled={status.kind === 'saved' || undefined}
        aria-describedby={id}
        className="shrink-0"
        onClick={() => status.kind !== 'saved' && onSave()}
      >
        <SaveIcon aria-hidden="true" />
        Save
      </Button>
    </div>
  )
}
