import { useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { Button } from '@/components/ui/button'
import {
  askerWords,
  modeKeysWords,
  modeLabel,
  modeWeight,
  modesAnswer,
  modesSentence,
  numberKey,
  pressedMode,
} from '../choiceText'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PromptPicture } from '../PromptPicture'
import type { PromptComponent } from '../types'
import { UndoButton } from '../UndoButton'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/**
 * ChooseMode: the modes of a modal spell or ability. A choice of exactly one answers at once from a button or from
 * its number key; a choice of several toggles modes (again, a mode that may repeat is added again) and Confirm modes
 * sends them in the order they happen, which is the order on the card. A cast can be backed out of with its Undo button or
 * Escape, and a triggered ability that must choose has none. Focus starts on the answer the response clock gives: Undo
 * where there is one, else the first mode.
 */
/** The room the card's picture is sized by (`PROMPT_CARD_BOX`): a whole card up to 7 rem wide beside the modes. */
const MODE_PICTURE_ROOM = '12rem'

export const ChooseModePrompt: PromptComponent<'ChooseMode'> = ({ options }) => {
  const { answer } = usePrompt()
  const { modes, min, max, repeat, can_cancel: canCancel } = options
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const single = min === 1 && max === 1
  // The indices of the chosen modes in the order they were chosen.
  const [chosen, setChosen] = useState<readonly number[]>([])
  const weights = new Map(modes.map((mode) => [mode.index, modeWeight(mode)]))
  const weight = (index: number): number => weights.get(index) ?? 1
  const pawprints = modes.some((mode) => mode.weight !== undefined)
  // What the chosen modes take from the most: one each, or their pawprints.
  const spent = chosen.reduce((sum, index) => sum + weight(index), 0)
  const first = useRef<HTMLButtonElement>(null)
  const cancel = useRef<HTMLButtonElement>(null)
  useDefaultFocus(canCancel ? cancel : first)

  const press = (index: number): void => {
    if (single) {
      void answer({ ChooseMode: { Modes: [index] } })
      return
    }
    setChosen((current) => pressedMode(current, index, { max, repeat, weight }))
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
    chosen: spent,
    pawprints,
  })
  const times = (index: number): number => chosen.filter((mode) => mode === index).length

  return (
    <PromptLayout
      text={
        <>
          {text}
          <span className="sr-only"> {modeKeysWords(modes.length, single)}</span>
        </>
      }
      choices={
        // The modes stand beside the card's picture, so each reads against the card it is a mode of; they fill the
        // band's width in columns of at least 16 rem.
        <div
          data-slot="mode-choice"
          style={{ '--choice-room': MODE_PICTURE_ROOM } as CSSProperties}
          className="flex basis-full items-start gap-3"
        >
          <PromptPicture object={source} size="normal" />
          <div
            role="group"
            aria-label="Modes"
            data-slot="mode-list"
            className="grid max-h-56 min-w-0 flex-1 grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),1fr))] content-start gap-1 overflow-y-auto p-0.5"
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
                  disabled={!single && spent + weight(mode.index) > max && (repeat || !picked)}
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
        </div>
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
        <UndoButton
          ref={cancel}
          zone={source?.zone}
          className="h-9 px-4"
          onUndo={() => void answer('Undo')}
        />
      ) : null}
    </PromptLayout>
  )
}
