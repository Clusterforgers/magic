import { useMemo, useState } from 'react'
import type { KeyboardEvent, ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { useObjects } from '../../prompt/useCombatNames'
import { FeedbackLine } from './FeedbackLine'
import { useManualSender } from './feedback'
import { Thumb } from './LookTopBody'
import { FIND_NOTHING, foundCards, tutorPreview } from './tutor'
import type { PanelInput } from './commands'

interface BodyProps {
  id: string
  input: PanelInput
  onClose: () => void
  /** The row's plain form, which starts the search. */
  plain: ReactNode
}

/**
 * The body of the Search for a card row: the plain form while no search is open, and once the seat looked through its
 * library the grid of its cards by name, with Apply for the card chosen and a shuffle without one.
 */
export function TutorBody({ id, input, onClose, plain }: BodyProps) {
  const searched = input.searched ?? []
  if (searched.length === 0) return <>{plain}</>
  return <SearchGrid key={searched.join(',')} id={id} input={input} searched={searched} onClose={onClose} />
}

interface GridProps {
  id: string
  input: PanelInput
  searched: readonly ObjectId[]
  onClose: () => void
}

function SearchGrid({ id, input, searched, onClose }: GridProps) {
  const { feedback, send } = useManualSender()
  const objects = useObjects(searched)
  const cards = useMemo(() => {
    const byId = new Map(searched.map((card, at) => [card, objects[at]]))
    return foundCards(searched, (card) => byId.get(card)?.name).map((card) => ({
      ...card,
      object: byId.get(card.id),
    }))
  }, [searched, objects])
  const [chosen, setChosen] = useState<ObjectId | null>(null)
  const preview = tutorPreview(
    cards.find((card) => card.id === chosen),
    input.library,
  )
  const ready = 'action' in preview
  const busy = feedback?.state === 'sending'
  const apply = (): void => {
    if (ready && !busy) send(preview.action, preview.apply)
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    event.stopPropagation()
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    }
  }
  return (
    <div id={id} role="group" aria-label="Search your library" onKeyDown={onKeyDown} className="grid gap-2 px-2 pt-1 pb-2">
      <p className="text-xs text-muted-foreground">
        {`Your library: ${cards.length} cards by name. Choose one for your hand.`}
      </p>
      <ul data-slot="tutor-grid" className="grid max-h-80 grid-cols-2 gap-1 overflow-y-auto sm:grid-cols-3">
        {cards.map((card) => (
          <li key={card.id}>
            <button
              type="button"
              data-card={card.id}
              aria-pressed={chosen === card.id}
              onClick={() => setChosen(card.id)}
              className={cn(
                'flex w-full min-w-0 items-center gap-1.5 rounded-md border border-transparent p-1 text-left text-xs outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring pointer-coarse:min-h-11',
                chosen === card.id && 'border-primary bg-accent',
              )}
            >
              <Thumb object={card.object} />
              <span className="min-w-0 truncate">{card.name}</span>
            </button>
          </li>
        ))}
      </ul>
      <p data-slot="command-preview" aria-live="polite" className={cn('text-sm', !ready && 'text-muted-foreground')}>
        {ready ? preview.preview : preview.problem}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" data-slot="command-apply" aria-disabled={!ready || undefined} onClick={ready ? apply : undefined}>
          {ready ? preview.apply : 'Apply'}
        </Button>
        <Button
          size="sm"
          variant="outline"
          data-slot="tutor-nothing"
          onClick={busy ? undefined : () => send(FIND_NOTHING.action, FIND_NOTHING.label)}
        >
          {FIND_NOTHING.label}
        </Button>
      </div>
      <FeedbackLine feedback={feedback} />
    </div>
  )
}
