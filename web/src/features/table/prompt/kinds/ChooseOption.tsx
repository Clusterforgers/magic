import { useMemo, useRef } from 'react'
import { Button } from '@/components/ui/button'
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

/**
 * ChooseOption: one of a short list (a color, a player, a name). Each option is a big button that answers at once,
 * numbered 1 to 9 and 0 for the keyboard, and focus starts on the first, the option the response clock takes. A color
 * carries its mana symbol and the viewer's own seat says so. The list scrolls inside the strip when it is long.
 */
export const ChooseOptionPrompt: PromptComponent<'ChooseOption'> = ({ options }) => {
  const { answer } = usePrompt()
  const seatName = useSeatNamer()
  const me = usePlay((state) => state.seat)
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const first = useRef<HTMLButtonElement>(null)
  useDefaultFocus(first)

  const choices = options.options
  const choose = (index: number) => () => void answer({ ChooseOption: index })
  usePromptKeys(
    Object.fromEntries(choices.slice(0, 10).map((_, index) => [numberKey(index) ?? '', choose(index)])),
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
      <div
        role="group"
        aria-label="Options"
        data-slot="option-list"
        className="flex max-h-44 basis-full flex-wrap items-center gap-2 overflow-y-auto p-0.5"
      >
        {choices.map((option, index) => {
          const key = numberKey(index)
          const label = optionLabel(option, seatName, me)
          return (
            <Button
              key={index}
              ref={index === 0 ? first : undefined}
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
