import { Button } from '@/components/ui/button'
import type { GenericOffer } from './genericChoices'
import { usePrompt } from './PromptContext'

/** A button for each answer on offer, the one that backs out of the cast last. Draws nothing when there is none. */
export function GenericAnswers({ offer }: { offer: GenericOffer }) {
  const { answer } = usePrompt()
  if (offer.choices.length === 0) return null
  return (
    <div
      role="group"
      aria-label="Answers"
      data-slot="generic-answers"
      className="flex max-h-56 basis-full flex-wrap items-center gap-2 overflow-y-auto p-0.5"
    >
      {offer.choices.map((choice) => (
        <Button
          key={choice.key}
          variant="outline"
          className="h-auto min-h-9 px-3 py-1 text-left text-sm whitespace-normal"
          data-cancel={choice.cancel === true ? '' : undefined}
          onClick={() => void answer(choice.answer)}
        >
          {choice.label}
        </Button>
      ))}
      {offer.unlisted === 0 ? null : (
        <span data-slot="generic-unlisted" className="text-sm text-muted-foreground">
          {offer.unlisted} more not shown
        </span>
      )}
    </div>
  )
}
