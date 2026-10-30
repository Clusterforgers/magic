import { useId, useRef, useState } from 'react'
import type { KeyboardEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { CommonToken, CounterKind, ManaType, ObjectId } from '@/protocol/generated'
import { useSeatNamer } from '../../names'
import { playerOf } from '../../selectors'
import { COMMON_TOKENS, GAME_OVER, MANA_KINDS, OUT_OF_GAME } from '../offers'
import { lookIsPending } from './arrange'
import { CardGroup } from './CardGroup'
import { FeedbackLine } from './FeedbackLine'
import { useManualSender } from './feedback'
import { panelGroups, pressPreset, previewOf, startDraft } from './commands'
import { LookTopBody } from './LookTopBody'
import { useRevealOnOpen } from './reveal'
import type { Command, CommandId, PanelDraft, PanelGroup, PanelInput } from './commands'

const NO_COUNTERS: readonly (readonly [CounterKind, number])[] = []
const NO_CARDS: readonly ObjectId[] = []

/** The commands no other command takes back (the library's order is lost), and what their row says about it. */
const LASTING: ReadonlySet<CommandId> = new Set<CommandId>(['mill', 'exileTop', 'bottomTop', 'shuffle'])
const LASTING_WORDS = 'This cannot be undone: the library’s order is lost.'

/** The step of Page Up and Page Down in an amount field. */
const PAGE_STEP = 5

/** The top cards the seat looked at and has not put back: its known top cards while its last look is still open. */
function lookedOf(
  known: readonly ObjectId[] | undefined,
  log: Parameters<typeof lookIsPending>[0],
  seat: number,
): readonly ObjectId[] {
  if (known === undefined || known.length === 0) return NO_CARDS
  return lookIsPending(log, seat) ? known : NO_CARDS
}

/** What the seat's commands are made from, or null for a viewer that is no seat of the game. */
function usePanelInput(): PanelInput | null {
  const nameOf = useSeatNamer()
  const mine = usePlay(
    useShallow((state) => {
      const player = state.seat === null ? undefined : playerOf(state, state.seat)
      if (state.seat === null || player === undefined) return null
      return {
        seat: state.seat,
        life: player.life,
        library: player.library.count,
        hand: player.hand.count,
        counters: player.counters ?? NO_COUNTERS,
        looked: lookedOf(player.known_top, state.log, state.seat),
        off: state.result !== null ? GAME_OVER : player.lost !== null ? OUT_OF_GAME : undefined,
      }
    }),
  )
  return mine === null ? null : { ...mine, name: nameOf(mine.seat) }
}

/**
 * The Commands panel: what the seat does by hand, grouped as You, Library and Make, each command a row that opens in
 * place with its amount, the change it will make and one Apply button. Nothing is sent before Apply. A viewer that is
 * no seat of the game has no panel.
 */
export function CommandsPanel({ initialOpen = null }: { initialOpen?: CommandId | null }) {
  const input = usePanelInput()
  // One row is open at a time, whichever group it is in; a command of the picked card is `card:` and its id.
  const [open, setOpen] = useState<string | null>(initialOpen)
  if (input === null) return null
  const groups = panelGroups(input)
  return (
    <section
      aria-label="Commands"
      data-slot="commands-panel"
      className="grid h-full min-h-0 content-start gap-1 overflow-y-auto overscroll-contain pr-1"
    >
      <LifeLine input={input} />
      {input.off === undefined ? null : (
        <p data-slot="commands-off" className="px-2 text-xs text-muted-foreground">
          {input.off}
        </p>
      )}
      {groups.map((group) => (
        <Group key={group.id} group={group} input={input} open={open} setOpen={setOpen} />
      ))}
      <CardGroup open={open} setOpen={setOpen} />
    </section>
  )
}

/** The seat's life in large figures, as a life counter shows it. */
function LifeLine({ input }: { input: PanelInput }) {
  return (
    <p data-slot="commands-life" className="flex items-baseline justify-between px-2 pt-1">
      <span className="truncate text-sm font-medium">{input.name}</span>
      <span className="text-2xl font-semibold tabular-nums">
        {input.life} <span className="text-xs font-normal text-muted-foreground">life</span>
      </span>
    </p>
  )
}

interface GroupProps {
  group: PanelGroup
  input: PanelInput
  open: string | null
  setOpen: (id: string | null) => void
}

function Group({ group, input, open, setOpen }: GroupProps) {
  const heading = useId()
  return (
    <div role="group" aria-labelledby={heading} data-group={group.id} className="grid gap-0.5 py-1">
      <h3 id={heading} className="px-2 pt-1 text-xs font-medium text-muted-foreground">
        {group.label}
      </h3>
      {group.commands.map((command) => (
        <Row
          key={command.id}
          command={command}
          input={input}
          open={open === command.id}
          onToggle={() => setOpen(open === command.id ? null : command.id)}
          onClose={() => setOpen(null)}
        />
      ))}
    </div>
  )
}

interface RowProps {
  command: Command
  input: PanelInput
  open: boolean
  onToggle: () => void
  onClose: () => void
}

/** One command: a button with its name and what it does, and its form in place while it is open. */
function Row({ command, input, open, onToggle, onClose }: RowProps) {
  const form = useId()
  const toggle = useRef<HTMLButtonElement>(null)
  const self = useRef<HTMLDivElement>(null)
  useRevealOnOpen(self, open)
  const close = (): void => {
    onClose()
    toggle.current?.focus({ preventScroll: true })
  }
  return (
    <div
      ref={self}
      data-command={command.id}
      data-open={open || undefined}
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
      {open ? (
        command.id === 'look' ? (
          <LookTopBody
            id={form}
            command={command}
            input={input}
            onClose={close}
            plain={<CommandForm id={form} command={command} input={input} onClose={close} />}
          />
        ) : (
          <CommandForm id={form} command={command} input={input} onClose={close} />
        )
      ) : null}
    </div>
  )
}

interface FormProps {
  id: string
  command: Command
  input: PanelInput
  onClose: () => void
}

/** The amount, the choice, the preview of the change and Apply; Enter applies and Escape closes. */
function CommandForm({ id, command, input, onClose }: FormProps) {
  const { feedback, send, clear } = useManualSender()
  const [draft, setDraft] = useState<PanelDraft>(() => startDraft(command))
  const preview = previewOf(command, draft, input)
  const ready = 'action' in preview
  const set = (next: Partial<PanelDraft>): void => {
    clear()
    setDraft((was) => ({ ...was, ...next }))
  }
  const apply = (): void => {
    if (!('action' in preview) || feedback?.state === 'sending') return
    // The row starts over once the server took the action; a refusal keeps the choices to change them.
    send(preview.action, preview.apply, () => setDraft(startDraft(command)))
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
      {command.choice === undefined ? null : <Choice kind={command.choice} draft={draft} set={set} />}
      {command.amount === undefined ? null : <Amount command={command} draft={draft} set={set} />}
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
      {LASTING.has(command.id) ? (
        <p data-slot="command-lasting" className="text-xs text-muted-foreground">
          {LASTING_WORDS}
        </p>
      ) : null}
      <FeedbackLine feedback={feedback} />
    </div>
  )
}

interface PartProps {
  draft: PanelDraft
  set: (next: Partial<PanelDraft>) => void
}

/** The quick amounts and the field; a signed command's buttons add to the pending change. */
function Amount({ command, draft, set }: PartProps & { command: Command }) {
  const spec = command.amount
  if (spec === undefined) return null
  const word = spec.signed ? 'Change' : 'How many'
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    const step = event.key === 'PageUp' ? PAGE_STEP : event.key === 'PageDown' ? -PAGE_STEP : 0
    if (step === 0) return
    event.preventDefault()
    set({
      amount: Math.min(spec.max, Math.max(spec.min, (Number.isNaN(draft.amount) ? 0 : draft.amount) + step)),
    })
  }
  return (
    <div className="flex flex-wrap items-center gap-1">
      {spec.presets.map((preset) => (
        <Button
          key={preset}
          variant="outline"
          size="sm"
          className="min-w-10 px-2 tabular-nums"
          aria-label={spec.signed ? `${preset > 0 ? 'Add' : 'Take'} ${Math.abs(preset)}` : `${preset}`}
          onClick={() => set({ amount: pressPreset(spec, draft.amount, preset) })}
        >
          {spec.signed && preset > 0 ? `+${preset}` : preset}
        </Button>
      ))}
      <Input
        type="number"
        aria-label={word}
        // A numeric keypad has no minus sign, so a field that takes one gets the full keyboard.
        inputMode={spec.signed ? 'text' : 'numeric'}
        min={spec.min}
        max={spec.max}
        value={Number.isNaN(draft.amount) ? '' : draft.amount}
        onKeyDown={onKeyDown}
        onChange={(event) => set({ amount: event.currentTarget.valueAsNumber })}
        className="h-8 w-20 tabular-nums pointer-coarse:h-11"
      />
    </div>
  )
}

/** The token or the kind of mana a command makes. */
function Choice({ kind, draft, set }: PartProps & { kind: 'token' | 'mana' }) {
  if (kind === 'token') {
    return (
      <NativeSelect
        aria-label="Token"
        value={draft.token}
        onChange={(event) => set({ token: event.currentTarget.value as CommonToken })}
      >
        {COMMON_TOKENS.map((token) => (
          <NativeSelectOption key={token} value={token}>
            {token}
          </NativeSelectOption>
        ))}
      </NativeSelect>
    )
  }
  return (
    <NativeSelect
      aria-label="Mana"
      value={draft.mana}
      onChange={(event) => set({ mana: event.currentTarget.value as ManaType })}
    >
      {MANA_KINDS.map(([mana, symbol]) => (
        <NativeSelectOption key={mana} value={mana}>
          {`${mana} {${symbol}}`}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  )
}
