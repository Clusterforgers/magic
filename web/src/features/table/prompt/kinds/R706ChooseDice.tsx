import { Dices } from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '../../names'
import { askerWords, numberKey, numberKeysWords } from '../choiceText'
import { dieLabel, isReroll, rerollCost, rerollLabel, rollWords } from '../diceText'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/**
 * R706ChooseDice (CR 706.2b, 706.6): a question about the dice of a roll just made. A reroll (Monitor Monitor) shows each
 * die as a toggle, numbered for the keyboard; Reroll pays the cost and rerolls the dice chosen, Keep (K) declines, and
 * focus starts on Keep, the answer the response clock gives. The roll to ignore (Bamboozling Beeble) is chosen by
 * pressing a die, which answers at once; focus starts on the first die.
 */
export const R706ChooseDicePrompt: PromptComponent<'R706ChooseDice'> = ({ options }) => {
  const { answer } = usePrompt()
  const me = usePlay((state) => state.seat)
  const namer = useSeatNamer()
  const [source] = useObjects(useMemo(() => (options.source === undefined ? [] : [options.source]), [options.source]))
  const reroll = isReroll(options)
  const [chosen, setChosen] = useState<number[]>([])
  const keep = useRef<HTMLButtonElement>(null)
  const firstDie = useRef<HTMLButtonElement>(null)
  useDefaultFocus(reroll ? keep : firstDie)

  const roller = options.roller === me ? 'You' : namer(options.roller)
  const toggle = (at: number) => (): void =>
    setChosen((was) => (was.includes(at) ? was.filter((die) => die !== at) : [...was, at].sort((a, b) => a - b)))
  const press = (at: number) => (reroll ? toggle(at) : () => void answer({ R706ChooseDice: [at] }))
  const keepAll = (): void => void answer({ R706ChooseDice: [] })
  usePromptKeys({
    ...Object.fromEntries(options.results.slice(0, 10).map((_, at) => [numberKey(at) ?? '', press(at)])),
    ...(reroll ? { k: keepAll } : {}),
  })

  const cost = rerollCost(options)
  return (
    <PromptLayout
      text={
        <span data-slot="dice-text">
          {askerWords(source?.name)}: {options.text}{' '}
          {reroll ? null : (
            <span className="text-muted-foreground">{rollWords(roller, options.sides, options.results)}</span>
          )}
          <span className="sr-only"> {numberKeysWords(options.results.length)}</span>
        </span>
      }
      inline={options.results.length <= 4}
      choices={
        <div role="group" aria-label="Dice" data-slot="dice-list" className="flex flex-wrap items-center gap-2 p-0.5">
          {options.results.map((result, at) => {
            const on = chosen.includes(at)
            const key = numberKey(at)
            return (
              <Button
                key={at}
                ref={at === 0 ? firstDie : undefined}
                variant="outline"
                className={cn('h-10 min-w-16 px-3 tabular-nums', on && 'border-primary bg-primary/10')}
                aria-label={dieLabel(at, result)}
                aria-pressed={reroll ? on : undefined}
                aria-keyshortcuts={key}
                onClick={press(at)}
              >
                <Dices aria-hidden="true" />
                <span className="text-base font-semibold">{result}</span>
                {key === undefined ? null : <KeyHint>{key}</KeyHint>}
              </Button>
            )
          })}
        </div>
      }
    >
      {reroll ? (
        <>
          <Button
            className="h-10 px-4"
            disabled={chosen.length === 0}
            onClick={() => void answer({ R706ChooseDice: chosen })}
          >
            {rerollLabel(chosen.length, cost)}
          </Button>
          <Button
            ref={keep}
            variant="outline"
            className="h-10 min-w-24 px-4"
            aria-label="Keep the results"
            aria-keyshortcuts="K"
            onClick={keepAll}
          >
            Keep
            <KeyHint>K</KeyHint>
          </Button>
        </>
      ) : null}
    </PromptLayout>
  )
}
