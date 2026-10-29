import { useId, useMemo, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { costPips } from '../../cardModel'
import { SIZE } from '../../cardSize'
import { ManaPips } from '../../ManaPips'
import { askerWords } from '../choiceText'
import {
  firstAllowed,
  isAllowed,
  numberProblem,
  numberSentence,
  rangeWords,
  step,
  topOf,
  totalFor,
} from '../numberChoice'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus } from '../usePromptKeys'

/**
 * ChooseNumber: a number from `min` to `max` (X, or how many times to pay a cost). A field takes a typed number (the
 * arrow keys step it, Enter confirms), the minus and plus buttons and the slider step over numbers `except` names,
 * and focus starts in the field on the smallest allowed number, the answer the response clock gives. For an X of a
 * mana cost the total cost for the chosen X is shown. Confirm is off, with the reason, for a number not allowed;
 * Cancel backs out of the cast when the options allow it.
 */
export const ChooseNumberPrompt: PromptComponent<'ChooseNumber'> = ({ options }) => {
  const { answer } = usePrompt()
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const [typed, setTyped] = useState(() => String(firstAllowed(options)))
  const field = useRef<HTMLInputElement>(null)
  useDefaultFocus(field)
  const fieldId = useId()
  const problemId = useId()

  const value = typed.trim() === '' ? Number.NaN : Number(typed)
  const problem = numberProblem(options, value)
  const top = topOf(options)
  const current = Number.isFinite(value) ? value : options.min
  const set = (n: number): void => setTyped(String(n))
  const text = options.text ?? ''
  const label = text === '' ? 'Number' : text
  const total =
    options.cost === undefined || problem !== undefined
      ? undefined
      : totalFor(options.cost, options.per_x ?? 0, value)

  const confirm = (event?: FormEvent): void => {
    event?.preventDefault()
    if (problem === undefined) answer({ ChooseNumber: { Number: value } })
  }
  const sentence = numberSentence(askerWords(source?.name), text)

  return (
    <PromptLayout
      text={
        <>
          {sentence}
          <span className="sr-only"> {rangeWords(options)}</span>
        </>
      }
    >
      <form className="flex flex-wrap items-center gap-2" data-slot="number-form" onSubmit={confirm}>
        <label htmlFor={fieldId} className="sr-only">
          {label}
        </label>
        <Button
          type="button"
          variant="outline"
          className="h-10 w-10"
          aria-label="Less"
          disabled={step(options, current, -1) === current}
          onClick={() => set(step(options, current, -1))}
        >
          −
        </Button>
        <Input
          ref={field}
          id={fieldId}
          type="number"
          inputMode="numeric"
          className="h-10 w-20 text-center tabular-nums"
          min={options.min}
          max={top}
          value={typed}
          aria-invalid={problem !== undefined}
          aria-describedby={problem === undefined ? undefined : problemId}
          onChange={(event) => setTyped(event.target.value)}
          onKeyDown={(event) => {
            // The arrows skip the numbers that are not allowed, as the buttons do.
            if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
            event.preventDefault()
            set(step(options, current, event.key === 'ArrowUp' ? 1 : -1))
          }}
        />
        <Button
          type="button"
          variant="outline"
          className="h-10 w-10"
          aria-label="More"
          disabled={step(options, current, 1) === current}
          onClick={() => set(step(options, current, 1))}
        >
          +
        </Button>
        {options.max === null ? null : (
          <input
            type="range"
            aria-label={`${label} (slider)`}
            className="w-40 accent-current"
            min={options.min}
            max={top}
            step={1}
            value={Math.min(Math.max(current, options.min), top)}
            onChange={(event) => {
              const wanted = Number(event.target.value)
              set(isAllowed(options, wanted) ? wanted : step(options, wanted, 1))
            }}
          />
        )}
        {total === undefined ? null : (
          <span data-slot="number-total" className="flex items-center gap-1.5 text-sm">
            Total cost
            <ManaPips pips={costPips(total)} sizeClass={SIZE.compact.pip} label="Total cost" />
          </span>
        )}
        <Button
          type="submit"
          className="h-10 px-4"
          data-slot="number-confirm"
          disabled={problem !== undefined}
        >
          {problem === undefined ? `Choose ${value}` : 'Choose'}
        </Button>
        {options.can_cancel === true ? (
          <Button
            type="button"
            variant="outline"
            className="h-10 px-4"
            onClick={() => answer({ ChooseNumber: 'Cancel' })}
          >
            Cancel
          </Button>
        ) : null}
        {problem === undefined ? null : (
          <p id={problemId} data-slot="number-problem" className="text-muted-foreground basis-full text-sm">
            {problem}
          </p>
        )}
      </form>
    </PromptLayout>
  )
}
