/** Pure: the commands of the picked card, as its card menu lists them, what each says it does, the preview of its change and the action it sends. */
import type { DamageTarget, ManualAction, ObjectView, PlayerId } from '@/protocol/generated'
import {
  attachTargets,
  buildAction,
  counterName,
  damageTargets,
  draftKind,
  EMPTY_DRAFT,
  heldOf,
  KNOWN_COUNTERS,
  LIMITS,
  manualMenu,
  targetKey,
  zoneKind,
} from '../offers'
import type { Draft, ManualGroup, ManualItem, ManualMenu, MenuInput } from '../offers'
import type { Preview } from './commands'

/** The commands of a card, in the order of its menu. */
export type CardCommandId =
  'move' | 'discard' | 'tap' | 'counters' | 'attach' | 'unattach' | 'damage' | 'pump'

/** One command of the picked card. */
export interface CardCommand {
  id: CardCommandId
  label: string
  /** One line of what the command does, in plain words. */
  effect: string
  /** Why it cannot be used now; the row is listed, inert. */
  unavailable?: string
  /** The places a card can be moved to (`move`), as the menu lists them. */
  moves?: ManualItem[]
  /** The action sent as it is (`discard`, `tap`, `unattach`). */
  action?: ManualAction
}

/** What a card's rows have chosen so far: the menu's form fields, a signed counter change and the move picked. */
export interface CardDraft extends Draft {
  /** Counters added (positive) or removed (negative). */
  delta: number
  /** The key of the move picked among `CardCommand.moves`. */
  dest: string
  /** `targetKey` of what damage is dealt to, or empty. */
  hitKey: string
}

/** The start of every card row. */
export function startCardDraft(command: CardCommand, input: MenuInput): CardDraft {
  const held = input.object.counters?.at(0)
  return {
    ...EMPTY_DRAFT,
    kind: held === undefined ? '+1/+1' : counterName(held[0]),
    delta: 0,
    power: 0,
    toughness: 0,
    dest: command.moves?.find((move) => move.unavailable === undefined)?.key ?? command.moves?.[0]?.key ?? '',
    hitKey: '',
  }
}

/** What the preview of a card row needs besides the draft. */
export interface CardContext {
  input: MenuInput
  /** Every object of the view, to name hosts and targets. */
  objects: Readonly<Record<number, ObjectView>>
  nameOf: (seat: PlayerId) => string
}

const find = (group: ManualGroup | undefined, key: string): ManualItem | undefined =>
  group?.items.find((item) => item.key === key)

const MOVE_EFFECT = 'Put the card into another zone.'

function movesOf(group: ManualGroup | undefined): CardCommand[] {
  if (group === undefined) return []
  const moves = group.items.filter((item) => item.action !== undefined && 'Move' in item.action)
  const out: CardCommand[] = []
  if (moves.length > 0) {
    // The menu lists an entry inert for a reason; the row is inert only when every place is.
    const reasons = moves.map((move) => move.unavailable)
    const all = reasons.every((reason) => reason !== undefined) ? reasons[0] : undefined
    out.push({ id: 'move', label: 'Move to…', effect: MOVE_EFFECT, moves, unavailable: all })
  } else if (group.note !== undefined) {
    out.push({ id: 'move', label: 'Move to…', effect: MOVE_EFFECT, unavailable: group.note })
  }
  const discard = find(group, 'discard')
  if (discard?.action !== undefined) {
    out.push({
      id: 'discard',
      label: 'Discard',
      effect: 'Put the card from your hand into your graveyard as a discard.',
      action: discard.action,
      unavailable: discard.unavailable,
    })
  }
  return out
}

/** The commands of the card `menu` was made for, mirroring its groups: Move to, Counters, Tap, Attach and the card's effects. */
export function cardCommands(menu: ManualMenu): CardCommand[] {
  const group = (id: ManualGroup['id']): ManualGroup | undefined => menu.groups.find((g) => g.id === id)
  const commands = movesOf(group('move'))
  const counters = group('counters')
  if (counters !== undefined) {
    const held = counters.items.some((item) => item.key.startsWith('remove-'))
    commands.push({
      id: 'counters',
      label: 'Counters',
      effect: held ? 'Add counters, or take them off.' : 'Add counters to the card.',
      unavailable: counters.items.every((item) => item.unavailable !== undefined)
        ? counters.items[0]?.unavailable
        : undefined,
    })
  }
  const tap = find(group('tap'), 'tap')
  if (tap?.action !== undefined) {
    const untap = 'Untap' in tap.action
    commands.push({
      id: 'tap',
      label: tap.label,
      effect: untap ? 'Turn the permanent upright.' : 'Turn the permanent sideways.',
      action: tap.action,
      unavailable: tap.unavailable,
    })
  }
  const attach = group('attach')
  const to = find(attach, 'attach')
  if (to !== undefined) {
    commands.push({
      id: 'attach',
      label: 'Attach to…',
      effect: 'Attach it to another permanent.',
      unavailable: to.unavailable,
    })
  } else if (attach?.note !== undefined) {
    commands.push({
      id: 'attach',
      label: 'Attach to…',
      effect: 'Attach it to another permanent.',
      unavailable: attach.note,
    })
  }
  const off = find(attach, 'unattach')
  if (off?.action !== undefined) {
    commands.push({
      id: 'unattach',
      label: 'Unattach',
      effect: 'Take it off what it is attached to.',
      action: off.action,
      unavailable: off.unavailable,
    })
  }
  const effects = group('effect')
  const damage = find(effects, 'damage')
  if (damage !== undefined) {
    commands.push({
      id: 'damage',
      label: 'Deal damage',
      effect: 'The card deals damage to a player or a permanent.',
      unavailable: damage.unavailable,
    })
  }
  const pump = find(effects, 'pump')
  if (pump !== undefined) {
    commands.push({
      id: 'pump',
      label: 'Pump',
      effect: 'Give the creature power and toughness until end of turn.',
      unavailable: pump.unavailable,
    })
  }
  return commands
}

/** The commands of the object in `input`, in one step. */
export function commandsFor(input: MenuInput): CardCommand[] {
  return cardCommands(manualMenu(input))
}

/** The kinds a counters row offers: the ones on the card first, then the protocol's, then any other. */
export function counterChoices(object: ObjectView): string[] {
  const held = (object.counters ?? []).map(([kind]) => counterName(kind))
  return [...new Set([...held, ...KNOWN_COUNTERS])]
}

/** The permanents an attachment row offers. */
export function attachChoices(context: CardContext): ObjectView[] {
  return attachTargets(Object.values(context.objects), context.input.object, context.input.seat)
}

/** What a damage row can hit. */
export function hitChoices(context: CardContext): DamageTarget[] {
  return damageTargets(Object.values(context.objects), context.input.players, context.input.seat)
}

const lowerFirst = (text: string): string =>
  text.includes("'s ") ? text : text.charAt(0).toLowerCase() + text.slice(1)
const plural = (n: number, one: string): string => `${n} ${one}${n === 1 ? '' : 's'}`
const signedNumber = (n: number): string => (n > 0 ? `+${n}` : `${n}`)

const FROM_WORDS: Record<string, string> = {
  battlefield: 'the battlefield',
  hand: 'hand',
  graveyard: 'graveyard',
  exile: 'exile',
  command: 'the command zone',
  library: 'library',
  stack: 'the stack',
  phased: 'phased out',
}

function nameOfTarget(target: DamageTarget, context: CardContext): string {
  if ('Player' in target) return context.nameOf(target.Player)
  return context.objects[target.Object]?.name ?? 'that permanent'
}

/** What applying `command` with `draft` changes, in the seat's words, and the action it sends. */
export function previewOfCard(command: CardCommand, draft: CardDraft, context: CardContext): Preview {
  const { input } = context
  if (command.unavailable !== undefined) return { problem: command.unavailable }
  const { object } = input
  const name = object.name
  switch (command.id) {
    case 'move': {
      const move = command.moves?.find((item) => item.key === draft.dest)
      if (move?.action === undefined) return { problem: 'Choose where it goes.' }
      if (move.unavailable !== undefined) return { problem: move.unavailable }
      const place = lowerFirst(move.label)
      const token = object.kind === 'Token'
      return {
        preview: token
          ? `${name}: the token ceases to exist`
          : `${name}: ${FROM_WORDS[zoneKind(object.zone)] ?? 'its zone'} → ${place}`,
        apply: token ? 'Remove the token' : `Move to ${place}`,
        action: move.action,
      }
    }
    case 'discard':
      return sends(command, `${name}: hand → graveyard, discarded`, `Discard ${name}`)
    case 'tap': {
      const tapped = object.permanent?.tapped === true
      return sends(
        command,
        `${name}: ${tapped ? 'tapped → untapped' : 'untapped → tapped'}`,
        `${tapped ? 'Untap' : 'Tap'} ${name}`,
      )
    }
    case 'unattach': {
      const host = object.attached_to === undefined ? undefined : context.objects[object.attached_to]
      return sends(
        command,
        `${name}: no longer attached${host === undefined ? '' : ` to ${host.name}`}`,
        'Unattach',
      )
    }
    case 'counters':
      return countersPreview(draft, context)
    case 'attach': {
      const built = buildAction('attach', { ...input, draft })
      if ('problem' in built) return { problem: built.problem }
      const host = draft.target === null ? undefined : context.objects[draft.target]
      return {
        preview: `${name}: attach to ${host?.name ?? 'that permanent'}`,
        apply: `Attach to ${host?.name ?? 'it'}`,
        action: built.action,
      }
    }
    case 'damage': {
      const hit = hitChoices(context).find((target) => targetKey(target) === draft.hitKey) ?? null
      const built = buildAction('damage', { ...input, draft: { ...draft, hit } })
      if ('problem' in built) return { problem: built.problem }
      const to = hit === null ? 'the target' : nameOfTarget(hit, context)
      return {
        preview: `${name} deals ${draft.count} damage to ${to}`,
        apply: `Deal ${draft.count} damage`,
        action: built.action,
      }
    }
    case 'pump': {
      const built = buildAction('pump', { ...input, draft })
      if ('problem' in built) return { problem: built.problem }
      const change = `${signedNumber(draft.power)}/${signedNumber(draft.toughness)}`
      return {
        preview: `${name}: ${change} until end of turn`,
        apply: `Pump ${change}`,
        action: built.action,
      }
    }
  }
}

function sends(command: CardCommand, preview: string, apply: string): Preview {
  return command.action === undefined
    ? { problem: 'Nothing to send.' }
    : { preview, apply, action: command.action }
}

function countersPreview(draft: CardDraft, context: CardContext): Preview {
  const { input } = context
  const { delta } = draft
  const kind = draftKind(draft)
  if (kind === null) return { problem: 'Name the counter with letters, digits, spaces and - + /.' }
  if (!Number.isInteger(delta) || Math.abs(delta) > LIMITS.counters) {
    return { problem: `Choose a change from -${LIMITS.counters} to ${LIMITS.counters}.` }
  }
  if (delta === 0) return { problem: 'Choose an amount with the buttons or the field.' }
  const word = counterName(kind)
  const now = heldOf(input.object.counters ?? [], kind)
  const built = buildAction(delta > 0 ? 'addCounters' : 'removeCounters', {
    ...input,
    draft: { ...draft, count: Math.abs(delta) },
  })
  if ('problem' in built) return { problem: built.problem }
  const counters = plural(Math.abs(delta), `${word} counter`)
  return {
    preview: `${input.object.name}: ${word} ${now} → ${now + delta}`,
    apply: delta > 0 ? `Add ${counters}` : `Remove ${counters}`,
    action: built.action,
  }
}
