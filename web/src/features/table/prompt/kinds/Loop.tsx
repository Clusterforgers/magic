import { useId, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { usePlay } from '@/features/game/gameContext'
import { iterationsProblem, LOOP_PRESETS, loopCards, stepWords } from '../../loopText'
import { useSeatNamer } from '../../names'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/**
 * Loop (CR 732): the engine found a loop and asks the player whose choice drives it how many times to repeat it, in
 * one batch. The presets 10, 100 and 1,000 and a number field (up to `max`) repeat it; Stop (S) plays on by hand. A
 * loop that runs by itself and never ends (`can_draw`) also offers to let it go on, which is a draw. Focus starts on
 * Stop, or on the draw where it is offered: the answers the response clock gives. The other players respond with their
 * normal priority after the batch.
 */
export const LoopPrompt: PromptComponent<'Loop'> = ({ options }) => {
  const { answer } = usePrompt()
  const me = usePlay((state) => state.seat)
  const namer = useSeatNamer()
  const seat = (who: number): string => (who === me ? 'you' : namer(who))
  const [typed, setTyped] = useState('')
  const stop = useRef<HTMLButtonElement>(null)
  const draw = useRef<HTMLButtonElement>(null)
  useDefaultFocus(options.can_draw ? draw : stop)
  const fieldId = useId()
  const problemId = useId()
  const repeat = (times: number) => () => void answer({ Loop: { Repeat: times } })
  const stopIt = (): void => void answer({ Loop: 'Stop' })
  usePromptKeys({ s: stopIt })

  const problem = typed.trim() === '' ? null : iterationsProblem(typed, options.max)
  const confirm = (event: FormEvent): void => {
    event.preventDefault()
    if (typed.trim() !== '' && problem === null) repeat(Number(typed.trim()))()
  }
  const each = stepWords(options.per_iteration, seat)
  const cards = loopCards(options.cards)
  const head = options.mandatory
    ? `${cards} repeat by themselves. You can respond to break the loop.`
    : `${cards} make a loop.`
  const detail = each.length === 0 ? 'Each time nothing changes.' : `Each time: ${each.join(', ')}.`
  const ends = options.bounded ? ' It ends by itself; a large number runs it to its end.' : ''
  return (
    <PromptLayout
      text={
        <span data-slot="loop-text">
          {head} <span className="text-muted-foreground">{detail + ends}</span>
          {options.asked > 0 ? (
            <span className="text-muted-foreground"> Offered {options.asked + 1} times this turn.</span>
          ) : null}
        </span>
      }
      choices={
        <form className="flex flex-wrap items-center gap-2" data-slot="loop-form" onSubmit={confirm}>
          <span className="text-sm">Repeat</span>
          {LOOP_PRESETS.map((times) => (
            <Button
              key={times}
              type="button"
              variant="outline"
              className="h-10 min-w-16 px-3 tabular-nums"
              aria-label={`Repeat ${times.toLocaleString('en-US')} times`}
              disabled={times > options.max}
              onClick={repeat(times)}
            >
              {times.toLocaleString('en-US')}
            </Button>
          ))}
          <label htmlFor={fieldId} className="sr-only">
            Times to repeat
          </label>
          <Input
            id={fieldId}
            type="number"
            inputMode="numeric"
            className="h-10 w-24 text-center tabular-nums"
            min={1}
            max={options.max}
            placeholder="N"
            value={typed}
            disabled={options.max === 0}
            aria-invalid={problem !== null}
            aria-describedby={problem === null ? undefined : problemId}
            onChange={(event) => setTyped(event.target.value)}
          />
          <Button type="submit" className="h-10 px-4" disabled={typed.trim() === '' || problem !== null}>
            Repeat
          </Button>
          {problem === null ? null : (
            <span id={problemId} role="alert" className="text-sm text-destructive">
              {problem}
            </span>
          )}
        </form>
      }
    >
      <Button
        ref={stop}
        variant="outline"
        className="h-10 min-w-24 px-4"
        aria-label="Stop the loop"
        aria-keyshortcuts="S"
        onClick={stopIt}
      >
        Stop
        <KeyHint>S</KeyHint>
      </Button>
      {options.can_draw ? (
        <Button
          ref={draw}
          variant="outline"
          className="h-10 min-w-24 px-4"
          aria-label="Let it loop: the game is a draw"
          onClick={() => void answer({ Loop: 'Draw' })}
        >
          Let it loop (draw)
        </Button>
      ) : null}
    </PromptLayout>
  )
}
