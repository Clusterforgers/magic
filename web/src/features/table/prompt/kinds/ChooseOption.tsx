import { useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '../../names'
import { ManaDot } from '../../ManaPool'
import { askerWords, numberKey, numberKeysWords, optionLabel, optionSentence } from '../choiceText'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/** More options than this get a filter field: a card name is chosen among all the cards of the game. */
const FILTER_FROM = 12

/**
 * ChooseOption: one of a short list (a color, a player, a name). Each option is a big button that answers at once,
 * numbered 1 to 9 and 0 for the keyboard, and focus starts on the first, the option the response clock takes. A color
 * carries its mana symbol and the viewer's own seat says so. The list scrolls inside the strip when it is long, and a
 * list of more than 12 options (a card name) has a filter field above it that narrows the buttons by their words.
 */
export const ChooseOptionPrompt: PromptComponent<'ChooseOption'> = ({ options }) => {
  const { answer } = usePrompt()
  const seatName = useSeatNamer()
  const me = usePlay((state) => state.seat)
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const first = useRef<HTMLButtonElement>(null)
  useDefaultFocus(first)

  const [filter, setFilter] = useState('')
  const filtering = options.options.length > FILTER_FROM
  const needle = filter.trim().toLowerCase()
  // An option keeps its index in the whole list, so the answer is the same whatever the filter hides.
  const choices = options.options
    .map((option, index) => ({ option, index, label: optionLabel(option, seatName, me) }))
    .filter((choice) => !filtering || choice.label.toLowerCase().includes(needle))
  const choose = (index: number) => () => void answer({ ChooseOption: index })
  usePromptKeys(
    Object.fromEntries(
      choices.slice(0, 10).map((choice, place) => [numberKey(place) ?? '', choose(choice.index)]),
    ),
  )

  const sentence = optionSentence(askerWords(source?.name), options.text ?? '')
  return (
    <PromptLayout
      text={
        <>
          {sentence}
          <span className="sr-only"> {numberKeysWords(choices.length)}</span>
        </>
      }
    >
      {filtering ? (
        <Input
          type="search"
          aria-label="Filter options"
          placeholder="Filter by name"
          autoComplete="off"
          value={filter}
          className="h-9 min-w-40 flex-1 basis-40"
          onChange={(event) => setFilter(event.target.value)}
        />
      ) : null}
      <div
        role="group"
        aria-label="Options"
        data-slot="option-list"
        className="flex max-h-44 basis-full flex-wrap items-center gap-2 overflow-y-auto p-0.5"
      >
        {choices.map(({ option, index, label }, place) => {
          const key = numberKey(place)
          return (
            <Button
              key={index}
              ref={place === 0 ? first : undefined}
              variant="outline"
              className="h-10 px-4"
              aria-label={label}
              aria-keyshortcuts={key}
              onClick={choose(index)}
            >
              {'Color' in option ? <ManaDot type={option.Color} /> : null}
              {label}
              {key === undefined ? null : <KeyHint>{key}</KeyHint>}
            </Button>
          )
        })}
      </div>
    </PromptLayout>
  )
}
