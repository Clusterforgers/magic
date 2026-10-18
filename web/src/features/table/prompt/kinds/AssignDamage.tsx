import { useId, useMemo, useRef, useState } from 'react'
import type { ChangeEvent, FocusEvent, FormEvent, KeyboardEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId } from '@/protocol/generated'
import { joinWords } from '../../eventText'
import {
  clampedAmount,
  defaultSplit,
  factsOf,
  judge,
  labelled,
  lethalNote,
  setAmount,
  toAnswer,
} from '../damageSplit'
import type { DamageRecipient } from '../damageSplit'
import { markCards, useCardMarks } from '../marks'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'

/** Props of one row of the split. */
interface RowProps {
  recipient: DamageRecipient
  label: string
  amount: number
  /** The most this row may take given what the others hold. */
  max: number
  active: boolean
  inputRef(element: HTMLInputElement | null): void
  onFocus(): void
  onChange(wanted: number): void
}

/** One blocker: its name and numbers, and a stepper (less, a number, more) for the damage it receives. */
function Row({ recipient, label, amount, max, active, inputRef, onFocus, onChange }: RowProps) {
  const note = lethalNote(recipient, amount)
  const step = (by: number) => () => onChange(amount + by)
  const key = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
    event.preventDefault()
    onChange(amount + (event.key === 'ArrowUp' ? 1 : -1))
  }
  return (
    <li
      data-slot="damage-row"
      data-object-id={recipient.id}
      data-active={active}
      className="grid gap-1 rounded-md border bg-background px-2 py-1.5 data-[active=true]:border-ring data-[active=true]:ring-2 data-[active=true]:ring-ring/50"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-x-3">
        <span className="min-w-0 text-sm font-medium break-words">{label}</span>
        <span data-slot="damage-facts" className="text-xs text-muted-foreground">
          {factsOf(recipient)}
        </span>
      </div>
      <div role="group" aria-label={`Damage to ${label}`} className="flex flex-wrap items-center gap-1.5">
        <Button
          type="button"
          variant="outline"
          className="h-10 w-10 px-0 aria-disabled:opacity-50"
          aria-label={`Less damage to ${label}`}
          aria-disabled={amount <= 0}
          onClick={step(-1)}
        >
          <span aria-hidden="true">−</span>
        </Button>
        <Input
          ref={inputRef}
          type="number"
          inputMode="numeric"
          min={0}
          max={max}
          step={1}
          value={amount}
          aria-label={`Damage to ${label}`}
          className="h-10 w-16 text-center [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
          onFocus={(event: FocusEvent<HTMLInputElement>) => {
            event.currentTarget.select()
            onFocus()
          }}
          onChange={(event: ChangeEvent<HTMLInputElement>) => onChange(Number(event.target.value))}
          onKeyDown={key}
        />
        <Button
          type="button"
          variant="outline"
          className="h-10 w-10 px-0 aria-disabled:opacity-50"
          aria-label={`More damage to ${label}`}
          aria-disabled={amount >= max}
          onClick={step(1)}
        >
          <span aria-hidden="true">+</span>
        </Button>
        {note === null ? null : (
          <span data-slot="damage-lethal" className="text-xs font-medium">
            {note}
          </span>
        )}
      </div>
    </li>
  )
}

/**
 * AssignDamage: the attacker divides its combat damage among its blockers. One row per blocker with a stepper, a
 * default split (lethal damage in the offered order, the rest to the last) so the common case is one click, and
 * Confirm once everything is assigned. Blockers are marked legal on the table, the row in use selected; a click on a
 * blocker's card moves focus to its row.
 */
export const AssignDamagePrompt: PromptComponent<'AssignDamage'> = ({ options }) => {
  const { attacker, power, blockers } = options
  const { answer, selection } = usePrompt()
  const statusId = useId()
  const inputs = useRef(new Map<ObjectId, HTMLInputElement>())
  const attackerName = usePlay((state) => state.objectsById[attacker]?.name ?? 'the attacker')
  const objects = usePlay(useShallow((state) => blockers.map((id) => state.objectsById[id])))

  const recipients = useMemo<DamageRecipient[]>(
    () =>
      blockers.map((id, index) => ({
        id,
        name: objects[index]?.name ?? 'Blocker',
        toughness: objects[index]?.toughness ?? null,
        damage: objects[index]?.permanent?.damage ?? 0,
      })),
    [blockers, objects],
  )
  const labels = useMemo(() => labelled(recipients.map((recipient) => recipient.name)), [recipients])
  const defaults = useMemo(() => defaultSplit(power, recipients), [power, recipients])
  // Until the player changes a number, the split follows the default for the creatures as they are now.
  const [edited, setEdited] = useState<readonly number[] | null>(null)
  const amounts = edited !== null && edited.length === recipients.length ? edited : defaults
  const verdict = judge(power, amounts)

  const change = (index: number) => (wanted: number) => setEdited(setAmount(amounts, index, wanted, power))

  const marks = useMemo(
    () =>
      markCards(
        [],
        blockers,
        blockers.filter((id) => selection.has(id)),
      ),
    [blockers, selection],
  )
  const focusRow = (id: ObjectId): void => {
    selection.set([id])
    inputs.current.get(id)?.focus()
  }
  useCardMarks(marks, focusRow, { toggles: true })

  const confirm = (event?: FormEvent): void => {
    event?.preventDefault()
    if (verdict.legal) answer(toAnswer(recipients, amounts))
  }

  const target = blockers.length === 1 ? 'its blocker' : 'its blockers'
  const verb = blockers.length === 1 ? 'Assign' : 'Divide'
  const where = blockers.length === 1 ? 'to' : 'among'
  const text = `${verb} the ${power} combat damage of ${attackerName} ${where} ${target}: ${joinWords(labels)}.`

  return (
    <PromptLayout text={text}>
      <form data-slot="damage-split" className="grid w-full gap-2" onSubmit={confirm}>
        <ul aria-label="Damage for each blocker" className="grid gap-1.5">
          {recipients.map((recipient, index) => {
            const amount = amounts[index] ?? 0
            return (
              <Row
                key={recipient.id}
                recipient={recipient}
                label={labels[index] ?? recipient.name}
                amount={amount}
                max={clampedAmount(amounts, index, power, power)}
                active={selection.has(recipient.id)}
                inputRef={(element) => {
                  if (element === null) inputs.current.delete(recipient.id)
                  else inputs.current.set(recipient.id, element)
                }}
                onFocus={() => selection.set([recipient.id])}
                onChange={change(index)}
              />
            )
          })}
        </ul>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <Button type="submit" className="h-10 px-4" disabled={!verdict.legal} aria-describedby={statusId}>
            Confirm
          </Button>
          <p
            id={statusId}
            role="status"
            data-slot="damage-status"
            data-legal={verdict.legal}
            className="min-w-0 text-sm"
          >
            {verdict.status}
          </p>
        </div>
      </form>
    </PromptLayout>
  )
}
