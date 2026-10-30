import { useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect } from '@/components/ui/native-select'
import type { DamageTarget, ManualAction, ObjectView } from '@/protocol/generated'
import {
  attachTargets,
  buildAction,
  counterName,
  damageTargets,
  KNOWN_COUNTERS,
  LIMITS,
  startDraft,
  targetKey,
} from './offers'
import type { Ask, BuildInput, Draft } from './offers'

/** How many permanents the attach list shows before the filter narrows it. */
const TARGET_LIMIT = 40

interface FormWords {
  title: (name: string) => string
  submit: string
  /** The label of the number field. */
  count: string
}

const FORMS: Record<Ask, FormWords> = {
  addCounters: { title: (name) => `Add counters to ${name}`, submit: 'Add', count: 'How many' },
  removeCounters: { title: (name) => `Remove counters from ${name}`, submit: 'Remove', count: 'How many' },
  attach: { title: (name) => `Attach ${name} to`, submit: 'Attach', count: '' },
  damage: { title: (name) => `Deal damage with ${name}`, submit: 'Deal', count: 'Damage' },
  pump: { title: (name) => `Pump ${name} until end of turn`, submit: 'Pump', count: '' },
  note: { title: (name) => `Note on ${name}`, submit: 'Save note', count: '' },
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="grid gap-1 text-sm">
      <span className="text-muted-foreground">{label}</span>
      {children}
    </label>
  )
}

interface NumberProps {
  label: string
  value: number
  onChange: (value: number) => void
  autoFocus?: boolean
  min?: number
}

function NumberField({ label, value, onChange, autoFocus = false, min = 0 }: NumberProps) {
  return (
    <Field label={label}>
      <Input
        type="number"
        // A numeric keypad has no minus sign, so a field that takes one gets the full keyboard.
        inputMode={min < 0 ? 'text' : 'numeric'}
        min={min}
        value={Number.isNaN(value) ? '' : value}
        data-autofocus={autoFocus || undefined}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
      />
    </Field>
  )
}

interface FormProps {
  ask: Ask
  input: Omit<BuildInput, 'draft'>
  /** Every object of the table, for the list of things to attach to. */
  objects: Readonly<Record<number, ObjectView>>
  nameOf: (seat: number) => string
  onBack: () => void
  onSend: (action: ManualAction) => void
}

/**
 * The form that completes an action the menu cannot send as it is: counters on the card, an attachment, damage, a
 * pump or a note.
 */
export function ManualForm({ ask, input, objects, nameOf, onBack, onSend }: FormProps) {
  const [draft, setDraft] = useState(() => startDraft(ask, input))
  const set = (change: Partial<Draft>): void => setDraft((now) => ({ ...now, ...change }))
  const words = FORMS[ask]
  const built = buildAction(ask, { ...input, draft })
  const problem = 'problem' in built ? built.problem : undefined
  const kindsAsked = ask === 'addCounters' || ask === 'removeCounters'

  const head = (
    <p className="px-1 text-sm font-medium" id="manual-form-title">
      {words.title(input.object.name)}
    </p>
  )
  const back = (
    <Button type="button" variant="ghost" size="sm" onClick={onBack}>
      Back
    </Button>
  )
  if (ask === 'attach') {
    return (
      <AttachList
        head={head}
        back={back}
        input={input}
        objects={objects}
        nameOf={nameOf}
        onPick={(target) => onSend({ Attach: { object: input.object.id, to: target } })}
      />
    )
  }
  return (
    <form
      role="dialog"
      aria-labelledby="manual-form-title"
      className="grid gap-2 p-2"
      onSubmit={(event) => {
        event.preventDefault()
        if ('action' in built) onSend(built.action)
      }}
    >
      {head}
      {kindsAsked ? (
        <>
          <Field label="Kind of counter">
            <NativeSelect
              value={draft.kind}
              data-autofocus
              onChange={(event) => set({ kind: event.currentTarget.value })}
            >
              {ask.startsWith('add')
                ? KNOWN_COUNTERS.map((kind) => <option key={kind}>{kind}</option>)
                : (input.object.counters ?? []).map(([kind]) => (
                    <option key={counterName(kind)}>{counterName(kind)}</option>
                  ))}
              {ask.startsWith('add') ? <option value="other">Other…</option> : null}
            </NativeSelect>
          </Field>
          {draft.kind === 'other' ? (
            <Field label="Name of the counter">
              <Input
                value={draft.other}
                maxLength={LIMITS.name}
                onChange={(event) => set({ other: event.currentTarget.value })}
              />
            </Field>
          ) : null}
        </>
      ) : null}
      {ask === 'damage' ? (
        <DamageTargetField input={input} objects={objects} nameOf={nameOf} draft={draft} set={set} />
      ) : null}
      {ask === 'note' ? (
        <Field label="Note">
          <Input
            value={draft.text}
            maxLength={LIMITS.note}
            data-autofocus
            onChange={(event) => set({ text: event.currentTarget.value })}
          />
        </Field>
      ) : null}
      {ask === 'note' ? null : ask === 'pump' ? (
        <div className="grid grid-cols-2 gap-2">
          <NumberField
            label="Power change"
            value={draft.power}
            autoFocus
            min={-LIMITS.pump}
            onChange={(power) => set({ power })}
          />
          <NumberField
            label="Toughness change"
            value={draft.toughness}
            min={-LIMITS.pump}
            onChange={(toughness) => set({ toughness })}
          />
        </div>
      ) : (
        <NumberField label={words.count} value={draft.count} min={1} onChange={(count) => set({ count })} />
      )}
      <p role="status" className="min-h-5 px-1 text-xs text-muted-foreground">
        {problem}
      </p>
      <div className="flex justify-end gap-2">
        {back}
        <Button type="submit" size="sm" disabled={problem !== undefined}>
          {words.submit}
        </Button>
      </div>
    </form>
  )
}

interface DamageTargetProps {
  input: Omit<BuildInput, 'draft'>
  objects: Readonly<Record<number, ObjectView>>
  nameOf: (seat: number) => string
  draft: Draft
  set: (change: Partial<Draft>) => void
}

/** The players and permanents a card can deal damage to, by name; nothing is chosen at first. */
function DamageTargetField({ input, objects, nameOf, draft, set }: DamageTargetProps) {
  const targets = useMemo(
    () => damageTargets(Object.values(objects), input.players, input.seat),
    [objects, input.players, input.seat],
  )
  const label = (target: DamageTarget): string => {
    if ('Player' in target) {
      return target.Player === input.seat ? `${nameOf(target.Player)} (you)` : nameOf(target.Player)
    }
    const object = objects[target.Object]
    return object === undefined ? 'A permanent' : `${object.name} (${nameOf(object.controller)})`
  }
  return (
    <Field label="Deal it to">
      <NativeSelect
        value={draft.hit === null ? '' : targetKey(draft.hit)}
        data-autofocus
        onChange={(event) => {
          const key = event.currentTarget.value
          set({ hit: targets.find((target) => targetKey(target) === key) ?? null })
        }}
      >
        <option value="">Choose…</option>
        {targets.map((target) => (
          <option key={targetKey(target)} value={targetKey(target)}>
            {label(target)}
          </option>
        ))}
      </NativeSelect>
    </Field>
  )
}

interface AttachProps {
  head: ReactNode
  back: ReactNode
  input: Omit<BuildInput, 'draft'>
  objects: Readonly<Record<number, ObjectView>>
  nameOf: (seat: number) => string
  onPick: (target: number) => void
}

/** The permanents an attachment can go to, narrowed by what is typed; a choice sends the action. */
function AttachList({ head, back, input, objects, nameOf, onPick }: AttachProps) {
  const [filter, setFilter] = useState('')
  const all = useMemo(
    () => attachTargets(Object.values(objects), input.object, input.seat),
    [objects, input.object, input.seat],
  )
  const shown = all.filter((object) => object.name.toLowerCase().includes(filter.trim().toLowerCase()))
  return (
    <div role="dialog" aria-labelledby="manual-form-title" className="grid min-h-0 gap-2 p-2">
      {head}
      <Field label="Filter by name">
        <Input value={filter} data-autofocus onChange={(event) => setFilter(event.currentTarget.value)} />
      </Field>
      <ul aria-label="Permanents" className="grid max-h-56 gap-0.5 overflow-y-auto">
        {shown.slice(0, TARGET_LIMIT).map((object) => (
          <li key={object.id}>
            <button
              type="button"
              className="flex w-full min-w-0 items-baseline justify-between gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent focus-visible:bg-accent focus-visible:outline-none pointer-coarse:min-h-11"
              onClick={() => onPick(object.id)}
            >
              <span className="truncate">{object.name}</span>
              <span className="shrink-0 text-xs text-muted-foreground">{nameOf(object.controller)}</span>
            </button>
          </li>
        ))}
      </ul>
      <p role="status" className="px-1 text-xs text-muted-foreground">
        {shown.length === 0
          ? 'No permanent matches.'
          : shown.length > TARGET_LIMIT
            ? `${shown.length - TARGET_LIMIT} more; type to narrow the list.`
            : ''}
      </p>
      <div className="flex justify-end">{back}</div>
    </div>
  )
}
