/** What the attachments of a host give it, as the marks on its face, the words of its spoken name and the lines of its preview. Pure. */
import type { GivenRule, GivesView, ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import { tuckRank } from './rowStacks'

/** What a host needs to tell its own attachments from foreign ones. */
export type HostRef = Pick<ObjectView, 'id' | 'controller'>

/** How a mark reads: a raise, a lowering or a restriction, or a plain change. */
export type MarkTone = 'raise' | 'restrict' | 'plain'

/** One thing an attachment gives, short for a mark and in words for a screen reader and the preview. */
export interface GiveItem {
  /** "+2/+2", "Flying", "Pro red", "Umbra", "Can't attack". */
  text: string
  /** "+2/+2", "flying", "protection from red", "umbra armor: the host is not destroyed". */
  words: string
  tone: MarkTone
}

interface RuleWords {
  text: string
  words: string
  tone: MarkTone
}

const RULES: Record<GivenRule, RuleWords> = {
  CantAttack: { text: "Can't attack", words: "can't attack", tone: 'restrict' },
  CantAttackYou: { text: "Can't attack you", words: "can't attack its controller", tone: 'restrict' },
  CantBlock: { text: "Can't block", words: "can't block", tone: 'restrict' },
  CanBlockOnly: { text: 'Blocks few', words: 'can block only some attackers', tone: 'restrict' },
  CantBeBlocked: { text: 'Unblockable', words: "can't be blocked", tone: 'raise' },
  CantBeBlockedBy: { text: 'Evasive', words: "can't be blocked by some creatures", tone: 'raise' },
  BlockerLimit: { text: 'Block limit', words: 'needs or allows only some blockers', tone: 'plain' },
  BlocksAdditional: { text: 'Extra blocks', words: 'can block additional creatures', tone: 'raise' },
  DoesntUntap: { text: "Doesn't untap", words: "doesn't untap", tone: 'restrict' },
  CantBeTurnedFaceUp: { text: 'Stays face down', words: "can't be turned face up", tone: 'restrict' },
  LoseAbilities: { text: 'Loses abilities', words: 'loses all abilities', tone: 'restrict' },
  LoseKeyword: { text: 'Loses a keyword', words: 'loses a keyword', tone: 'restrict' },
  GainAbility: { text: 'New ability', words: 'gains an ability', tone: 'raise' },
  SetController: { text: 'Stolen', words: 'changes controller', tone: 'restrict' },
  SwitchPowerToughness: { text: 'Swaps P/T', words: 'power and toughness are switched', tone: 'plain' },
  ChangeTypes: { text: 'New types', words: 'changes its types', tone: 'plain' },
  SetColors: { text: 'New colour', words: 'changes its colour', tone: 'plain' },
  Goaded: { text: 'Goaded', words: 'is goaded', tone: 'restrict' },
  UmbraArmor: { text: 'Umbra', words: 'umbra armor, so it is not destroyed', tone: 'raise' },
}

const BOTH: RuleWords = { text: "Can't attack or block", words: "can't attack or block", tone: 'restrict' }

const signed = (n: number): string => (n < 0 ? `${n}` : `+${n}`)

/** "Pro red" for "Protection from red"; every other keyword as the server names it. */
export function shortKeyword(name: string): string {
  const protection = /^protection from (.+)$/i.exec(name)
  return protection?.[1] === undefined ? name : `Pro ${protection[1]}`
}

const lowerFirst = (text: string): string => text.charAt(0).toLowerCase() + text.slice(1)

/** The tone of a power and toughness change: a raise when neither falls, a lowering when neither rises. */
function statTone(power: number, toughness: number): MarkTone {
  if (power >= 0 && toughness >= 0) return 'raise'
  return power <= 0 && toughness <= 0 ? 'restrict' : 'plain'
}

/**
 * What `gives` gives, one item each in the order of a mark: the set power and toughness, the change to them, the
 * rules, then the keywords. Nothing for an attachment that gives nothing.
 */
export function itemsOf(gives: GivesView | undefined): GiveItem[] {
  if (gives === undefined) return []
  const items: GiveItem[] = []
  if (gives.set !== undefined) {
    const [power, toughness] = gives.set
    items.push({
      text: `Set ${power}/${toughness}`,
      words: `power and toughness set to ${power}/${toughness}`,
      tone: 'plain',
    })
  }
  const power = gives.power ?? 0
  const toughness = gives.toughness ?? 0
  if (power !== 0 || toughness !== 0) {
    const text = `${signed(power)}/${signed(toughness)}`
    items.push({ text, words: text, tone: statTone(power, toughness) })
  }
  const rules = gives.rules ?? []
  // Pacifism and its kin say both at once.
  const both = rules.includes('CantAttack') && rules.includes('CantBlock')
  for (const rule of rules) {
    if (both && rule === 'CantBlock') continue
    items.push(both && rule === 'CantAttack' ? BOTH : RULES[rule])
  }
  for (const keyword of gives.keywords ?? []) {
    items.push({ text: shortKeyword(keyword), words: lowerFirst(keyword), tone: 'plain' })
  }
  return items
}

/** The tone of a whole mark: a restriction anywhere shows, else a raise, else plain. */
function toneOf(items: readonly GiveItem[]): MarkTone {
  if (items.some((item) => item.tone === 'restrict')) return 'restrict'
  return items.some((item) => item.tone === 'raise') ? 'raise' : 'plain'
}

/** "a", "a and b", "a, b and c". */
export function listWords(words: readonly string[]): string {
  if (words.length < 2) return words.join('')
  return `${words.slice(0, -1).join(', ')} and ${words[words.length - 1] ?? ''}`
}

/** What one attachment gives its host, in the order it is tucked. */
export interface Attach {
  id: ObjectId
  name: string
  /** The attachment's controller when it is not the host's, else undefined. */
  foreign: PlayerId | undefined
  items: readonly GiveItem[]
}

/** The attachments of a host in the order they tuck (Auras, Equipment, Fortifications, each by id), with what each gives. */
export function attachesOf(host: HostRef, attachments: readonly ObjectView[]): Attach[] {
  return attachments
    .filter((attachment) => attachment.id !== host.id)
    .map((attachment) => ({ attachment, rank: tuckRank(attachment.subtypes) }))
    .sort((a, b) => a.rank - b.rank || a.attachment.id - b.attachment.id)
    .map(({ attachment }) => ({
      id: attachment.id,
      name: attachment.name,
      foreign: attachment.controller === host.controller ? undefined : attachment.controller,
      items: itemsOf(attachment.gives),
    }))
}

/** One mark on the host: what one attachment gives, or what several give alike. */
export interface Mark {
  /** Stable while the content is. */
  key: string
  /** What is drawn: the items that fit `chars`, or the attachment's name when it gives nothing. */
  text: string
  tone: MarkTone
  /** The attachments it stands for, in the order they tuck. */
  ids: readonly ObjectId[]
  /** The controllers of those that another seat controls. */
  foreign: readonly PlayerId[]
  /** Whether items were left out to fit. */
  cut: boolean
}

/** How many characters a mark may use before it leaves items out. */
export interface MarkOptions {
  chars?: number
}

/** The visible text of a list of items: as many as fit `chars` (the first always), with "…" when some are left out. */
function fitText(items: readonly GiveItem[], chars: number): { text: string; cut: boolean } {
  const all = items.map((item) => item.text).join(' ')
  if (all.length <= chars || items.length < 2) return { text: all, cut: false }
  // " …" takes two characters of what is written.
  let text = items[0]?.text ?? ''
  for (const item of items.slice(1)) {
    const next = `${text} ${item.text}`
    if (next.length + 2 > chars) break
    text = next
  }
  return { text: `${text} …`, cut: true }
}

/**
 * The marks of a host: one per attachment in the tuck order, "+2/+2", "Flying", "Pro red", "Umbra", "Can't attack"; an
 * attachment that gives nothing is marked by its name; attachments that give the same, and are controlled alike,
 * are one mark.
 */
export function marksOf(
  host: HostRef,
  attachments: readonly ObjectView[],
  options: MarkOptions = {},
): Mark[] {
  return marksFrom(attachesOf(host, attachments), options)
}

/** The marks of attachments already read with `attachesOf`. */
export function marksFrom(attaches: readonly Attach[], { chars = 99 }: MarkOptions = {}): Mark[] {
  const marks: Mark[] = []
  const seen = new Map<string, number>()
  for (const attach of attaches) {
    const { text, cut } =
      attach.items.length === 0 ? { text: attach.name, cut: false } : fitText(attach.items, chars)
    const identity = `${attach.items.length === 0 ? `name:${attach.name}` : attach.items.map((i) => i.text).join(' ')}|${attach.foreign ?? ''}`
    const at = seen.get(identity)
    const earlier = at === undefined ? undefined : marks[at]
    if (at !== undefined && earlier !== undefined) {
      marks[at] = {
        ...earlier,
        ids: [...earlier.ids, attach.id],
        foreign: attach.foreign === undefined ? earlier.foreign : [...earlier.foreign, attach.foreign],
      }
      continue
    }
    seen.set(identity, marks.length)
    marks.push({
      key: identity,
      text,
      tone: attach.items.length === 0 ? 'plain' : toneOf(attach.items),
      ids: [attach.id],
      foreign: attach.foreign === undefined ? [] : [attach.foreign],
      cut,
    })
  }
  return marks
}

/** The marks that fit `room` lines and how many attachments the last line, "+N", stands for. */
export interface CappedMarks {
  shown: Mark[]
  /** The attachments left out of `shown`; 0 when everything fits. */
  more: number
}

/** The marks that fit `room` lines: all of them when they fit, else the first `room - 1` and a "+N" line for the rest. */
export function capMarks(marks: readonly Mark[], room: number): CappedMarks {
  if (marks.length <= room) return { shown: [...marks], more: 0 }
  const shown = marks.slice(0, Math.max(0, room - 1))
  const left = marks.slice(shown.length)
  return { shown, more: left.reduce((sum, mark) => sum + mark.ids.length, 0) }
}

/** The icon a compact face shows with the count: a restriction wins, then a raise. */
export function compactTone(marks: readonly Mark[]): MarkTone {
  if (marks.some((mark) => mark.tone === 'restrict')) return 'restrict'
  return marks.some((mark) => mark.tone === 'raise') ? 'raise' : 'plain'
}

/** What an attachment gives in words: "+2/+0, trample", or null when it gives nothing. */
export function givesWords(gives: GivesView | undefined): string | null {
  const items = itemsOf(gives)
  return items.length === 0 ? null : items.map((item) => item.words).join(', ')
}

/** The attachment's name, with its controller's before it when another seat controls it: "Bo's Pacifism". */
export function attachName(attach: Pick<Attach, 'name' | 'foreign'>, seat: (id: PlayerId) => string): string {
  return attach.foreign === undefined ? attach.name : `${seat(attach.foreign)}'s ${attach.name}`
}

/** The attachments of a host in words for its spoken name: "Rancor attached giving +2/+0 and trample". */
export function attachedWords(attaches: readonly Attach[], seat: (id: PlayerId) => string): string[] {
  return attaches.map((attach) => {
    const name = attachName(attach, seat)
    return attach.items.length === 0
      ? `${name} attached`
      : `${name} attached giving ${listWords(attach.items.map((item) => item.words))}`
  })
}

/** The most of a face the foot stack may fill, and the height of a mark line with the gap under it, in px. */
const FOOT_SHARE = 0.4
const LINE_PX = 18

/** How many mark lines the foot stack of a face `height` px high has left once other marks take `usedPx` of it. */
export function roomFor(height: number, usedPx: number): number {
  return Math.floor((height * FOOT_SHARE - usedPx + LINE_PX - 16) / LINE_PX)
}
