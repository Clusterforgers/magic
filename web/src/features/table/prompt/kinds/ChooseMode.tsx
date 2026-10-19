import { useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  askerWords,
  modeKeysWords,
  modeLabel,
  modesAnswer,
  modesSentence,
  numberKey,
  pressedMode,
} from '../choiceText'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/**
 * ChooseMode: the modes of a modal spell or ability. A choice of exactly one answers at once from a button or from
 * its number key; a choice of several toggles modes (again, a mode that may repeat is added again) and Confirm modes
 * sends them in the order they happen, which is the order on the card. A cast can be backed out of with Cancel, and
 * a triggered ability that must choose has none. Focus starts on the answer the response clock gives: Cancel where
 * there is one, else the first mode.
 */
export const ChooseModePrompt: PromptComponent<'ChooseMode'> = ({ options }) => {
  const { answer } = usePrompt()
  const { modes, min, max, repeat, can_cancel: canCancel } = options
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const single = min === 1 && max === 1
  // The indices of the chosen modes in the order they were chosen.
  const [chosen, setChosen] = useState<readonly number[]>([])
  const full = chosen.length >= max
  const first = useRef<HTMLButtonElement>(null)
  const cancel = useRef<HTMLButtonElement>(null)
  useDefaultFocus(canCancel ? cancel : first)

  const press = (index: number): void => {
    if (single) {
      void answer({ ChooseMode: { Modes: [index] } })
      return
    }
    setChosen((current) => pressedMode(current, index, { max, repeat }))
  }
  usePromptKeys(
    Object.fromEntries(
      modes.slice(0, 10).flatMap((mode, place) => {
        const key = numberKey(place)
        return key === undefined ? [] : [[key, () => press(mode.index)] as const]
      }),
    ),
  )

  const text = modesSentence({
    asker: askerWords(source?.name),
    min,
    max,
    repeat,
    chosen: chosen.length,
  })
  const cancelWords = source?.zone === 'Battlefield' ? 'Cancel the ability' : 'Cancel the spell'
  const times = (index: number): number => chosen.filter((mode) => mode === index).length

  return (
    <PromptLayout
      text={
        <>
          {text}
          <span className="sr-only"> {modeKeysWords(modes.length, single)}</span>
        </>
      }
    >
      {single ? null : (
        <Button
          className="h-9 px-4"
          disabled={chosen.length < min}
          onClick={() => void answer({ ChooseMode: { Modes: modesAnswer(chosen) } })}
        >
          Confirm modes
        </Button>
      )}
      {single || !repeat ? null : (
        <Button
          variant="outline"
          className="h-9 px-4"
          disabled={chosen.length === 0}
          onClick={() => setChosen((current) => current.slice(0, -1))}
        >
          Undo
        </Button>
      )}
      {canCancel ? (
        <Button
          ref={cancel}
          variant="outline"
          className="h-9 px-4"
          onClick={() => void answer({ ChooseMode: 'Cancel' })}
        >
          {cancelWords}
        </Button>
      ) : null}
      <div
        role="group"
        aria-label="Modes"
        data-slot="mode-list"
        className="flex max-h-56 basis-full flex-col gap-1 overflow-y-auto p-0.5"
      >
        {modes.map((mode, place) => {
          const key = numberKey(place)
          const label = modeLabel(mode)
          const count = times(mode.index)
          const picked = count > 0
          return (
            <Button
              key={mode.index}
              ref={place === 0 ? first : undefined}
              variant={picked ? 'default' : 'outline'}
              className="h-auto min-h-9 justify-start whitespace-normal px-3 py-0.5 text-left text-sm leading-snug"
              aria-label={label}
              aria-keyshortcuts={key}
              aria-pressed={single ? undefined : picked}
              disabled={!single && full && (repeat || !picked)}
              focusableWhenDisabled
              onClick={() => press(mode.index)}
            >
              {key === undefined ? null : <KeyHint>{key}</KeyHint>}
              <span className="ml-2 min-w-0">{label}</span>
              {count > 1 ? <span className="ml-2 text-xs">x{count}</span> : null}
            </Button>
          )
        })}
      </div>
    </PromptLayout>
  )
}
