import { useMemo, useRef } from 'react'
import type { Ref } from 'react'
import { Button } from '@/components/ui/button'
import type { ObjectView, ReplacementCandidate } from '@/protocol/generated'
import { usePreviewHandlers } from '../../usePreviewHandlers'
import { numberKey } from '../choiceText'
import { KeyHint } from '../KeyHint'
import { useOracleText } from '../oracleWords'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PromptPicture } from '../PromptPicture'
import {
  replacementKeys,
  replacementLabels,
  replacementOrigin,
  replacementSentence,
} from '../replacementText'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

interface RowProps {
  index: number
  candidate: ReplacementCandidate
  object: ObjectView | undefined
  label: string
  onChoose(index: number): void
  buttonRef?: Ref<HTMLButtonElement>
}

/** One effect: its source card as a picture, its name, its wording and the key; a click, Enter or the key answers. */
function ReplacementRow({ index, candidate, object, label, onChoose, buttonRef }: RowProps) {
  const preview = usePreviewHandlers(candidate.source, () => onChoose(index))
  const oracle = useOracleText(object)
  const wording = oracle ?? replacementOrigin(candidate)
  const key = numberKey(index)
  return (
    <li className="min-w-0">
      <Button
        ref={buttonRef}
        variant="outline"
        data-slot="replacement-effect"
        data-effect={index}
        aria-label={wording === null ? label : `${label}: ${wording}`}
        aria-keyshortcuts={key}
        title={wording ?? undefined}
        className="h-auto w-full items-start justify-start gap-2 p-1.5 text-left whitespace-normal"
        {...preview}
      >
        <PromptPicture object={object} size="compact" />
        <span className="min-w-0 flex-1">
          <span className="flex items-center justify-between gap-2 text-sm font-medium">
            <span className="min-w-0 truncate">{label}</span>
            {key === undefined ? null : <KeyHint>{key}</KeyHint>}
          </span>
          {wording === null ? null : (
            <span
              data-slot="replacement-wording"
              className="mt-0.5 block line-clamp-4 text-xs font-normal text-muted-foreground"
            >
              {wording}
            </span>
          )}
        </span>
      </Button>
    </li>
  )
}

/**
 * ChooseReplacement: several replacement effects apply to one event and the player picks which applies first (CR 616.1).
 * Each effect is a row with its source card's picture, name and rules text (the effect's own wording is the card's,
 * read from the card database; a lasting effect says a spell or ability made it). A click, Enter or the number key
 * 1 to 9 (0 for the tenth) answers at once with the effect's index; focus starts on the first, the effect the
 * response clock takes. Hover and focus preview the card. The engine asks only when two or more apply.
 */
export const ChooseReplacementPrompt: PromptComponent<'ChooseReplacement'> = ({ options }) => {
  const { answer } = usePrompt()
  const candidates = options.candidates
  const sources = useMemo(() => candidates.map((candidate) => candidate.source), [candidates])
  const objects = useObjects(sources)
  const labels = useMemo(
    () =>
      replacementLabels(
        candidates,
        objects.map((object) => object?.name),
      ),
    [candidates, objects],
  )
  const first = useRef<HTMLButtonElement>(null)
  useDefaultFocus(first)

  const choose = (index: number): void => void answer({ ChooseReplacement: index })
  usePromptKeys(
    Object.fromEntries(
      candidates.slice(0, 10).map((_, index) => [numberKey(index) ?? '', () => choose(index)]),
    ),
  )

  return (
    <PromptLayout
      text={
        <>
          {replacementSentence(options.text ?? '')}
          <span className="sr-only"> {replacementKeys(candidates.length)}</span>
        </>
      }
    >
      <ol
        aria-label="Replacement effects"
        data-slot="replacement-list"
        className="grid max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_7rem))] basis-full grid-cols-1 gap-2 overflow-y-auto p-0.5 sm:grid-cols-2"
      >
        {candidates.map((candidate, index) => (
          <ReplacementRow
            key={index}
            index={index}
            candidate={candidate}
            object={objects[index]}
            label={labels[index] ?? 'A card'}
            onChoose={choose}
            buttonRef={index === 0 ? first : undefined}
          />
        ))}
      </ol>
    </PromptLayout>
  )
}
