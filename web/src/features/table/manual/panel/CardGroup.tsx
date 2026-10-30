import { useEffect, useId, useMemo, useRef, useState } from 'react'
import type { KeyboardEvent } from 'react'
import { CrosshairIcon, XIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { useSeatNamer } from '../../names'
import { LIMITS, targetKey, zoneKind } from '../offers'
import type { MenuInput } from '../offers'
import { useMenuInput } from '../useMenuInput'
import { AmountField } from './AmountField'
import {
  attachChoices,
  commandsFor,
  counterChoices,
  hitChoices,
  previewOfCard,
  startCardDraft,
} from './cardCommands'
import type { CardCommand, CardContext, CardDraft } from './cardCommands'
import type { AmountSpec } from './commands'
import { FeedbackLine } from './FeedbackLine'
import { useManualSender } from './feedback'
import { PICK_FOR, pickedCard, usePick, usePickStore } from './pick'

const COUNTER_SPEC: AmountSpec = {
  signed: true,
  min: -LIMITS.counters,
  max: LIMITS.counters,
  presets: [-1, 1],
  start: 0,
}
const DAMAGE_SPEC: AmountSpec = { signed: false, min: 1, max: LIMITS.damage, presets: [1, 2, 3, 5], start: 1 }
const PUMP_SPEC: AmountSpec = {
  signed: true,
  min: -LIMITS.pump,
  max: LIMITS.pump,
  presets: [-1, 1],
  start: 0,
}

/** Where a card is, in the words of a picked-card line. */
function placeOf(object: ObjectView, input: MenuInput): string {
  const zone = zoneKind(object.zone)
  const whose = object.controller === input.seat ? '' : `${input.ownerName}'s `
  return zone === 'battlefield'
    ? `${whose}battlefield`
    : `${whose}${zone === 'command' ? 'command zone' : zone}`
}

/**
 * The Card group: "Pick a card" arms the table's next press, then the card's commands appear as the card menu lists
 * them, each a row that opens in place with its choices, the change it will make and one Apply button.
 */
export function CardGroup({ open, setOpen }: { open: string | null; setOpen: (id: string | null) => void }) {
  const heading = useId()
  const picks = usePickStore()
  const pick = usePick()
  const card = pickedCard(pick)
  const armed = pick.phase === 'armed'
  const input = useMenuInput(card ?? -1)
  const shown = card !== null && input !== null ? input : null
  const header = useRef<HTMLParagraphElement>(null)
  const root = useRef<HTMLDivElement>(null)
  // The group sits under the others, so a card chosen scrolls it to the top of the panel, where its commands are.
  useEffect(() => {
    if (card !== null) root.current?.scrollIntoView?.({ block: 'start' })
  }, [card])
  // A card chosen while this is on show takes the focus, so the keyboard continues with its commands.
  const was = useRef(card)
  useEffect(() => {
    if (card !== null && card !== was.current && pick.phase === 'picked')
      header.current?.focus({ preventScroll: true })
    was.current = card
  }, [card, pick.phase])
  const commands = useMemo(() => (shown === null ? [] : commandsFor(shown)), [shown])
  return (
    <div ref={root} role="group" aria-labelledby={heading} data-group="card" className="grid gap-0.5 py-1">
      <h3 id={heading} className="px-2 pt-1 text-xs font-medium text-muted-foreground">
        Card
      </h3>
      <button
        type="button"
        data-command="pick"
        aria-pressed={armed}
        disabled={picks === null}
        onClick={() => picks?.dispatch(armed ? { type: 'cancel' } : { type: 'arm', for: PICK_FOR })}
        className={cn(
          'flex min-h-8 w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left outline-none select-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring pointer-coarse:min-h-11',
          armed && 'bg-muted',
        )}
      >
        <CrosshairIcon aria-hidden="true" className="size-4 shrink-0" />
        <span className="flex min-w-0 flex-col">
          <span className="text-sm font-medium">
            {armed ? 'Press a card on the table…' : shown === null ? 'Pick a card' : 'Pick another card'}
          </span>
          <span className="text-xs text-muted-foreground">
            {armed ? 'Press this again or Escape to cancel.' : 'Press a card on the table to act on it.'}
          </span>
        </span>
      </button>
      {shown === null ? (
        <p data-slot="card-empty" className="px-2 text-xs text-muted-foreground">
          {card !== null && !armed
            ? 'That card is no longer on the table.'
            : 'Pick a card on the table to act on it.'}
        </p>
      ) : (
        <>
          <p
            ref={header}
            tabIndex={-1}
            data-slot="card-picked"
            className="mx-2 flex items-center justify-between gap-2 rounded-md bg-muted px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <span className="min-w-0 truncate">
              <span className="font-medium">{shown.object.name}</span>{' '}
              <span className="text-xs text-muted-foreground">{placeOf(shown.object, shown)}</span>
            </span>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Clear the picked card"
              onClick={() => picks?.dispatch({ type: 'clear' })}
            >
              <XIcon aria-hidden="true" />
            </Button>
          </p>
          {commands.map((command) => (
            <CardRow
              key={`${shown.object.id}-${command.id}`}
              command={command}
              input={shown}
              open={open === `card:${command.id}`}
              onToggle={() => setOpen(open === `card:${command.id}` ? null : `card:${command.id}`)}
              onClose={() => setOpen(null)}
            />
          ))}
        </>
      )}
    </div>
  )
}

interface RowProps {
  command: CardCommand
  input: MenuInput
  open: boolean
  onToggle: () => void
  onClose: () => void
}

/** One command of the picked card: a button with its name and what it does, and its form in place while it is open. */
function CardRow({ command, input, open, onToggle, onClose }: RowProps) {
  const form = useId()
  const toggle = useRef<HTMLButtonElement>(null)
  const close = (): void => {
    onClose()
    toggle.current?.focus({ preventScroll: true })
  }
  return (
    <div
      data-command={`card:${command.id}`}
      data-open={open || undefined}
      data-card-command=""
      className={cn('rounded-md', open && 'bg-muted')}
    >
      <button
        ref={toggle}
        type="button"
        aria-expanded={open}
        aria-controls={open ? form : undefined}
        onClick={onToggle}
        className="flex min-h-8 w-full min-w-0 flex-col rounded-md px-2 py-1.5 text-left outline-none select-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring pointer-coarse:min-h-11"
      >
        <span className="text-sm font-medium">{command.label}</span>
        <span className="text-xs text-muted-foreground">{command.unavailable ?? command.effect}</span>
      </button>
      {open ? <CardForm id={form} command={command} input={input} onClose={close} /> : null}
    </div>
  )
}

interface FormProps {
  id: string
  command: CardCommand
  input: MenuInput
  onClose: () => void
}

/** The choices of the command, the preview of its change and Apply; Enter applies and Escape closes. */
function CardForm({ id, command, input, onClose }: FormProps) {
  const nameOf = useSeatNamer()
  const objects = usePlay((state) => state.objectsById)
  const { feedback, send, clear } = useManualSender()
  const [draft, setDraft] = useState<CardDraft>(() => startCardDraft(command, input))
  const context = useMemo<CardContext>(() => ({ input, objects, nameOf }), [input, objects, nameOf])
  const preview = previewOfCard(command, draft, context)
  const ready = 'action' in preview
  const set = (next: Partial<CardDraft>): void => {
    clear()
    setDraft((was) => ({ ...was, ...next }))
  }
  const apply = (): void => {
    if (ready && feedback?.state !== 'sending') {
      send(preview.action, preview.apply, () => setDraft(startCardDraft(command, input)))
    }
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    event.stopPropagation()
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    } else if (event.key === 'Enter' && !(event.target instanceof HTMLButtonElement)) {
      event.preventDefault()
      apply()
    }
  }
  return (
    <div
      id={id}
      role="group"
      aria-label={command.label}
      onKeyDown={onKeyDown}
      className="grid gap-2 px-2 pt-1 pb-2"
    >
      <CardFields command={command} draft={draft} set={set} context={context} />
      <p
        data-slot="command-preview"
        aria-live="polite"
        className={cn('text-sm tabular-nums', !ready && 'text-muted-foreground')}
      >
        {ready ? preview.preview : preview.problem}
      </p>
      <Button
        size="sm"
        data-slot="command-apply"
        aria-disabled={!ready || undefined}
        onClick={ready ? apply : undefined}
      >
        {ready ? preview.apply : 'Apply'}
      </Button>
      <FeedbackLine feedback={feedback} />
    </div>
  )
}

interface FieldsProps {
  command: CardCommand
  draft: CardDraft
  set: (next: Partial<CardDraft>) => void
  context: CardContext
}

/** What the command still asks for: a place, a kind and an amount of counters, a permanent, a target and damage, or a pump. */
function CardFields({ command, draft, set, context }: FieldsProps) {
  const { input } = context
  switch (command.id) {
    case 'move':
      return (
        <NativeSelect
          aria-label="Move to"
          value={draft.dest}
          onChange={(event) => set({ dest: event.currentTarget.value })}
        >
          {(command.moves ?? []).map((move) => (
            <NativeSelectOption key={move.key} value={move.key} disabled={move.unavailable !== undefined}>
              {move.label}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      )
    case 'counters':
      return (
        <>
          <NativeSelect
            aria-label="Kind of counter"
            value={draft.kind}
            onChange={(event) => set({ kind: event.currentTarget.value })}
          >
            {counterChoices(input.object).map((kind) => (
              <NativeSelectOption key={kind} value={kind}>
                {kind}
              </NativeSelectOption>
            ))}
            <NativeSelectOption value="other">Other…</NativeSelectOption>
          </NativeSelect>
          {draft.kind === 'other' ? (
            <Input
              aria-label="Name of the counter"
              maxLength={LIMITS.name}
              value={draft.other}
              onChange={(event) => set({ other: event.currentTarget.value })}
              className="h-8 pointer-coarse:h-11"
            />
          ) : null}
          <AmountField
            label="Change"
            spec={COUNTER_SPEC}
            value={draft.delta}
            onChange={(delta) => set({ delta })}
          />
        </>
      )
    case 'attach':
      return (
        <NativeSelect
          aria-label="Attach to"
          value={draft.target ?? ''}
          onChange={(event) =>
            set({ target: event.currentTarget.value === '' ? null : Number(event.currentTarget.value) })
          }
        >
          <NativeSelectOption value="">Choose a permanent…</NativeSelectOption>
          {attachChoices(context).map((host) => (
            <NativeSelectOption key={host.id} value={host.id}>
              {host.controller === input.seat
                ? host.name
                : `${host.name} (${context.nameOf(host.controller)})`}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      )
    case 'damage':
      return (
        <>
          <NativeSelect
            aria-label="Deal damage to"
            value={draft.hitKey}
            onChange={(event) => set({ hitKey: event.currentTarget.value })}
          >
            <NativeSelectOption value="">Choose a target…</NativeSelectOption>
            {hitChoices(context).map((target) => (
              <NativeSelectOption key={targetKey(target)} value={targetKey(target)}>
                {'Player' in target
                  ? context.nameOf(target.Player)
                  : (context.objects[target.Object]?.name ?? 'Permanent')}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <AmountField
            label="Damage"
            spec={DAMAGE_SPEC}
            value={draft.count}
            onChange={(count) => set({ count })}
          />
        </>
      )
    case 'pump':
      return (
        <>
          <AmountField
            label="Power"
            spec={PUMP_SPEC}
            value={draft.power}
            onChange={(power) => set({ power })}
          />
          <AmountField
            label="Toughness"
            spec={PUMP_SPEC}
            value={draft.toughness}
            onChange={(toughness) => set({ toughness })}
          />
        </>
      )
    default:
      return null
  }
}
