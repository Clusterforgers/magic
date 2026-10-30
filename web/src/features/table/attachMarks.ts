/** What the attachments of a host give it, as the marks on its face, the words of its spoken name and the lines of its preview. Pure. */
import type { GivenRule, GivesView, ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import { layoutWords } from './markLayout'
import type { LayoutOptions } from './markLayout'
import { tuckRank } from './rowStacks'

/** What a host needs to tell its own attachments from foreign ones. */
export type HostRef = Pick<ObjectView, 'id' | 'controller'>

/** How a mark reads: a raise, a lowering or a restriction, or a plain change. */
export type MarkTone = 'raise' | 'restrict' | 'plain'

/** One thing an attachment gives, short for a mark and in words for a screen reader and the preview. */
export interface GiveItem {
  /** "+2/+2", "Flying", "Pro red", "Umbra", "Can't attack". */
  text: string
  /** The same in fewer, whole words for a narrow mark: "No attack", "Pacified", "Indestr."; the text itself when it is short. */
  brief: string
  /** "+2/+2", "flying", "protection from red", "umbra armor: the host is not destroyed". */
  words: string
  tone: MarkTone
}

type RuleWords = GiveItem

const give = (text: string, brief: string, words: string, tone: MarkTone): RuleWords => ({
  text,
  brief,
  words,
  tone,
})

// Every rule the engine names has its words here: a rule left out is a type error.
const RULES: Record<GivenRule, RuleWords> = {
  CantAttack: give("Can't attack", 'No attack', "can't attack", 'restrict'),
  CantAttackYou: give("Can't attack you", 'Spares you', "can't attack its controller", 'restrict'),
  CantBlock: give("Can't block", 'No block', "can't block", 'restrict'),
  CanBlockOnly: give('Blocks few', 'Blocks few', 'can block only some attackers', 'restrict'),
  CantBeBlocked: give('Unblockable', 'Evades', "can't be blocked", 'raise'),
  CantBeBlockedBy: give('Evasive', 'Evasive', "can't be blocked by some creatures", 'raise'),
  BlockerLimit: give('Block limit', 'Blockers', 'needs or allows only some blockers', 'plain'),
  BlocksAdditional: give('Extra blocks', 'More blocks', 'can block additional creatures', 'raise'),
  DoesntUntap: give("Doesn't untap", 'No untap', "doesn't untap", 'restrict'),
  CantBeTurnedFaceUp: give('Stays face down', 'Face down', "can't be turned face up", 'restrict'),
  LoseAbilities: give('Loses abilities', 'Blanked', 'loses all abilities', 'restrict'),
  LoseKeyword: give('Loses a keyword', 'No keyword', 'loses a keyword', 'restrict'),
  GainAbility: give('New ability', 'Ability', 'gains an ability', 'raise'),
  SetController: give('Stolen', 'Stolen', 'changes controller', 'restrict'),
  SwitchPowerToughness: give('Swaps P/T', 'P/T swap', 'power and toughness are switched', 'plain'),
  ChangeTypes: give('New types', 'Types', 'changes its types', 'plain'),
  SetColors: give('New colour', 'Colour', 'changes its colour', 'plain'),
  Goaded: give('Goaded', 'Goaded', 'is goaded', 'restrict'),
  UmbraArmor: give('Umbra', 'Umbra', 'umbra armor, so it is not destroyed', 'raise'),
}

const BOTH = give("Can't attack or block", 'Pacified', "can't attack or block", 'restrict')

const BRIEF_KEYWORDS: Readonly<Record<string, string>> = {
  indestructible: 'Indestr.',
  'pro multicolored': 'Pro multi',
  'pro monocolored': 'Pro mono',
}

/** The keyword in a narrow mark: the long ones shortened, the rest as they are. */
function briefKeyword(short: string): string {
  return BRIEF_KEYWORDS[short.toLowerCase()] ?? short
}

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
    const text = `Set ${power}/${toughness}`
    items.push({
      text,
      brief: text,
      words: `power and toughness set to ${power}/${toughness}`,
      tone: 'plain',
    })
  }
  const power = gives.power ?? 0
  const toughness = gives.toughness ?? 0
  if (power !== 0 || toughness !== 0) {
    const text = `${signed(power)}/${signed(toughness)}`
    items.push({ text, brief: text, words: text, tone: statTone(power, toughness) })
  }
  const rules = gives.rules ?? []
  // Pacifism and its kin say both at once.
  const both = rules.includes('CantAttack') && rules.includes('CantBlock')
  for (const rule of rules) {
    if (both && rule === 'CantBlock') continue
    items.push(both && rule === 'CantAttack' ? BOTH : RULES[rule])
  }
  for (const keyword of gives.keywords ?? []) {
    const text = shortKeyword(keyword)
    items.push({ text, brief: briefKeyword(text), words: lowerFirst(keyword), tone: 'plain' })
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
  /** What it gives as the view says it, for summing the marks past the room. */
  gives?: GivesView | undefined
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
      gives: attachment.gives,
    }))
}

/** One mark on the host: what one attachment gives, or what several give alike. */
export interface Mark {
  /** Stable while the content is. */
  key: string
  /** What is drawn, `lines` joined: the items that fit, or the attachment's name when it gives nothing. */
  text: string
  /** The whole-word lines of `text`; one unless the mark was given room for two. */
  lines: readonly string[]
  tone: MarkTone
  /** The attachments it stands for, in the order they tuck. */
  ids: readonly ObjectId[]
  /** The controllers of those that another seat controls. */
  foreign: readonly PlayerId[]
  /** Whether items were left out to fit. */
  cut: boolean
}

/** How wide a mark may be and how many lines it may take before it leaves items out (`layoutWords`). */
export type MarkOptions = LayoutOptions

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
export function marksFrom(attaches: readonly Attach[], options: MarkOptions = {}): Mark[] {
  const marks: Mark[] = []
  const seen = new Map<string, number>()
  for (const attach of attaches) {
    const { lines, cut } = layoutWords(
      attach.items.length === 0 ? [{ text: attach.name, brief: attach.name }] : attach.items,
      options,
    )
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
      text: lines.join(' '),
      lines,
      tone: attach.items.length === 0 ? 'plain' : toneOf(attach.items),
      ids: [attach.id],
      foreign: attach.foreign === undefined ? [] : [attach.foreign],
      cut,
    })
  }
  return marks
}

/** What the attachments past the room give together: one mark with the sum. */
export interface RestMark extends Mark {
  /** The attachments it sums. */
  count: number
  /** What they give in words for a screen reader and a tooltip: "3 more attachments: +5/+3, haste and shroud". */
  words: string
}

/** The marks that fit `room` lines, and the sum of the attachments that do not (null when everything fits). */
export interface FoldedMarks {
  shown: Mark[]
  rest: RestMark | null
}

/** The sum of what attachments give: power and toughness added, the last set value, rules and keywords once each. */
export function sumGives(all: ReadonlyArray<GivesView | undefined>): GivesView {
  const given = all.filter((gives): gives is GivesView => gives !== undefined)
  const power = given.reduce((sum, gives) => sum + (gives.power ?? 0), 0)
  const toughness = given.reduce((sum, gives) => sum + (gives.toughness ?? 0), 0)
  const set = given.reduce<GivesView['set']>((last, gives) => gives.set ?? last, undefined)
  const rules = [...new Set(given.flatMap((gives) => gives.rules ?? []))]
  const keywords = given
    .flatMap((gives) => gives.keywords ?? [])
    .filter(
      (name, at, names) => names.findIndex((other) => other.toLowerCase() === name.toLowerCase()) === at,
    )
  return {
    ...(power === 0 ? {} : { power }),
    ...(toughness === 0 ? {} : { toughness }),
    ...(set === undefined ? {} : { set }),
    ...(rules.length === 0 ? {} : { rules }),
    ...(keywords.length === 0 ? {} : { keywords }),
  }
}

/** The one mark for attachments that do not get a line of their own: their summed gives, else just how many they are. */
function restMark(attaches: readonly Attach[], options: MarkOptions): RestMark {
  const items = itemsOf(sumGives(attaches.map((attach) => attach.gives)))
  const count = attaches.length
  const { lines, cut } =
    items.length === 0 ? { lines: [`+${count}`], cut: false } : layoutWords(items, options)
  return {
    key: 'rest',
    text: lines.join(' '),
    lines,
    tone: items.length === 0 ? 'plain' : toneOf(items),
    ids: attaches.map((attach) => attach.id),
    foreign: attaches.flatMap((attach) => (attach.foreign === undefined ? [] : [attach.foreign])),
    cut,
    count,
    words: `${count} more attachments${items.length === 0 ? '' : `: ${listWords(items.map((item) => item.words))}`}`,
  }
}

const linesOf = (marks: readonly Mark[]): number => marks.reduce((sum, mark) => sum + mark.lines.length, 0)

/**
 * The marks that fit `room` lines. All of them while they fit, each in up to two lines; else each in one line while
 * those fit; else the first ones in one line each and the rest as one mark that sums what the other attachments give
 * ("+5/+3 Haste Shroud"), in as many lines as are left (two at most).
 */
export function foldMarks(attaches: readonly Attach[], room: number, options: MarkOptions = {}): FoldedMarks {
  const whole = marksFrom(attaches, options)
  if (linesOf(whole) <= room) return { shown: whole, rest: null }
  const lone = marksFrom(attaches, { ...options, maxLines: 1 })
  if (lone.length <= room) return { shown: lone, rest: null }
  const keep = Math.max(0, room <= 2 ? room - 1 : room - 2)
  const shown = lone.slice(0, keep)
  const left = new Set(lone.slice(keep).flatMap((mark) => mark.ids))
  const rest = restMark(
    attaches.filter((attach) => left.has(attach.id)),
    { ...options, maxLines: Math.max(1, room - keep) },
  )
  return { shown, rest }
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
