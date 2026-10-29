import { useId, useMemo, useRef, useState } from 'react'
import type { ChangeEvent, FocusEvent, FormEvent, KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, Target } from '@/protocol/generated'
import { useSeatNamer } from '../../names'
import { askerWords } from '../choiceText'
import { uniqueNames } from '../combatText'
import { divideSentence, divisionAnswer, evenShares, judgeShares, mostFor, setShare } from '../divideText'
import { markCards, useCardMarks } from '../marks'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { TargetFace } from '../TargetFace'
import { objectWords, targetKey } from '../targetText'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus } from '../usePromptKeys'

/** Props of one row of the division. */
interface RowProps {
  target: Target
  label: string
  share: number
  /** The most this row may hold given what the others hold. */
  max: number
  active: boolean
  inputRef(element: HTMLInputElement | null): void
  onEnter(): void
  onLeave(): void
  onChange(wanted: number): void
}

/** One target: its picture (a card) or its seat, its name, and a stepper (less, a number, more) for its share. */
function Row({ target, label, share, max, active, inputRef, onEnter, onLeave, onChange }: RowProps) {
  const step = (by: number) => () => onChange(share + by)
  const key = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
    event.preventDefault()
    onChange(share + (event.key === 'ArrowUp' ? 1 : -1))
  }
  return (
    <li
      data-slot="divide-row"
      data-target={targetKey(target)}
      data-active={active}
      onFocus={onEnter}
      onBlur={(event: FocusEvent<HTMLLIElement>) => {
        if (!event.currentTarget.contains(event.relatedTarget)) onLeave()
      }}
      className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md border bg-background px-2 py-1.5 data-[active=true]:border-ring data-[active=true]:ring-2 data-[active=true]:ring-ring/50"
    >
      {'Object' in target ? <TargetFace id={target.Object.id} /> : null}
      <span className="min-w-0 flex-1 text-sm font-medium break-words">{label}</span>
      <div role="group" aria-label={`Share for ${label}`} className="flex items-center gap-1.5">
        <Button
          type="button"
          variant="outline"
          className="h-10 w-10 px-0 aria-disabled:opacity-50"
          aria-label={`Less for ${label}`}
          aria-disabled={share <= 1}
          onClick={step(-1)}
        >
          <span aria-hidden="true">−</span>
        </Button>
        <Input
          ref={inputRef}
          type="number"
          inputMode="numeric"
          min={1}
          max={max}
          step={1}
          value={share}
          aria-label={`Amount for ${label}`}
          className="h-10 w-16 text-center [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
          onFocus={(event: FocusEvent<HTMLInputElement>) => {
            event.currentTarget.select()
          }}
          onChange={(event: ChangeEvent<HTMLInputElement>) => onChange(Number(event.target.value))}
          onKeyDown={key}
        />
        <Button
          type="button"
          variant="outline"
          className="h-10 w-10 px-0 aria-disabled:opacity-50"
          aria-label={`More for ${label}`}
          aria-disabled={share >= max}
          onClick={step(1)}
        >
          <span aria-hidden="true">+</span>
        </Button>
      </div>
    </li>
  )
}

/**
 * DivideAmount: the total of a spell or ability (damage, counters) is divided among its targets, each getting at
 * least one. One row per target, in the order the engine lists them: the card's picture or the player's name and a
 * stepper (the minus and plus buttons, the arrow keys or a typed number); a share never goes past what the other
 * targets leave. The split starts even, as the response clock would answer, so the common case is one press of
 * Confirm; the line beside Confirm, above the rows, counts what is left to give and Confirm waits until nothing is. A cast can be
 * taken back with Cancel; a triggered ability that divides as it goes on the stack cannot. Targets that are cards on
 * the table are marked, and a click on one moves focus to its row.
 */
export const DivideAmountPrompt: PromptComponent<'DivideAmount'> = ({ options }) => {
  const { total, among, text = '' } = options
  const { answer, selection } = usePrompt()
  const seatName = useSeatNamer()
  const me = usePlay((state) => state.seat)
  const statusId = useId()
  const inputs = useRef(new Map<ObjectId, HTMLInputElement>())
  const first = useRef<HTMLInputElement | null>(null)
  useDefaultFocus(first)

  const objectIds = useMemo(
    () => among.flatMap((target) => ('Object' in target ? [target.Object.id] : [])),
    [among],
  )
  const known = useObjects(useMemo(() => [options.source, ...objectIds], [options.source, objectIds]))
  const source = known[0]
  const labels = useMemo(() => {
    const byId = new Map(
      known.flatMap((object) => (object === undefined ? [] : [[object.id, object] as const])),
    )
    const names = uniqueNames(objectIds.map((id) => ({ id, name: byId.get(id)?.name ?? 'an object' })))
    return among.map((target) => {
      if ('Player' in target) {
        const name = seatName(target.Player)
        return target.Player === me ? `${name} (you)` : name
      }
      const { id } = target.Object
      const object = byId.get(id)
      const name = names.get(id) ?? 'an object'
      return object === undefined ? name : objectWords(object, name, seatName)
    })
  }, [known, among, objectIds, seatName, me])

  // Until the player changes a number, the split is the even one.
  const [edited, setEdited] = useState<readonly number[] | null>(null)
  const shares = edited !== null && edited.length === among.length ? edited : evenShares(total, among.length)
  const verdict = judgeShares(total, shares)

  const marks = useMemo(
    () =>
      markCards(
        [],
        objectIds,
        objectIds.filter((id) => selection.has(id)),
      ),
    [objectIds, selection],
  )
  const focusRow = (id: ObjectId): void => {
    selection.set([id])
    inputs.current.get(id)?.focus()
  }
  useCardMarks(marks, focusRow, { toggles: true })

  const confirm = (event?: FormEvent): void => {
    event?.preventDefault()
    if (verdict.legal) answer(divisionAnswer(among, shares))
  }
  const cancelWords = source?.zone === 'Battlefield' ? 'Cancel the ability' : 'Cancel the spell'

  return (
    <PromptLayout text={divideSentence(askerWords(source?.name), text, total)}>
      <form data-slot="divide-split" className="grid w-full gap-2" onSubmit={confirm}>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <Button type="submit" className="h-10 px-4" disabled={!verdict.legal} aria-describedby={statusId}>
            Confirm
          </Button>
          {options.can_cancel === true ? (
            <Button
              type="button"
              variant="outline"
              className="h-10 px-4"
              onClick={() => answer({ DivideAmount: 'Cancel' })}
            >
              {cancelWords}
            </Button>
          ) : null}
          <p
            id={statusId}
            role="status"
            data-slot="divide-status"
            data-legal={verdict.legal}
            data-left={verdict.left}
            className="min-w-0 text-sm"
          >
            {verdict.status}
          </p>
        </div>
        <ul aria-label="Amount for each target" className="grid max-h-64 gap-1.5 overflow-y-auto">
          {among.map((target, index) => {
            const share = shares[index] ?? 1
            const objectId = 'Object' in target ? target.Object.id : undefined
            return (
              <Row
                key={targetKey(target)}
                target={target}
                label={labels[index] ?? 'a target'}
                share={share}
                max={mostFor(shares, index, total)}
                active={objectId !== undefined && selection.has(objectId)}
                inputRef={(element) => {
                  if (index === 0) first.current = element
                  if (objectId === undefined) return
                  if (element === null) inputs.current.delete(objectId)
                  else inputs.current.set(objectId, element)
                }}
                onEnter={() => (objectId === undefined ? selection.clear() : selection.set([objectId]))}
                onLeave={() => selection.clear()}
                onChange={(wanted) => setEdited(setShare(shares, index, wanted, total))}
              />
            )
          })}
        </ul>
      </form>
    </PromptLayout>
  )
}
