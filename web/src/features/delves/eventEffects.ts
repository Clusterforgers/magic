/** What an event option does and what a hero's answer did, read from the content's outcome data (`docs/delves.md`, Events). */
import type { DelveOptionView, Hero, Needs, Outcome, Resolved } from '@/protocol/generated'

/** What a chip pictures. */
export type ChipKind = 'gold' | 'life' | 'relic' | 'card'

/** Whether a chip is something gained, paid or neither. */
export type ChipTone = 'gain' | 'cost' | 'flat'

/** One effect as a chip: an amount of gold or life, a relic or a card. */
export interface Chip {
  kind: ChipKind
  tone: ChipTone
  text: string
}

/** A chance: what a win gives and what a loss gives, each with its odds in percent. */
export interface Chance {
  odds: number
  win: Part
  lose: Part
}

/** Sure effects and chances side by side. */
export interface Part {
  chips: Chip[]
  chances: Chance[]
}

/** A number with its sign, a minus as "−": "+40", "−8", "0". */
export function signed(n: number): string {
  if (n === 0) return '0'
  return n > 0 ? `+${n}` : `−${Math.abs(n)}`
}

/** The chip of a sure outcome; `null` for a gamble, which is a chance instead. */
export function chipOf(outcome: Outcome): Chip | null {
  if (outcome === 'Relic') return { kind: 'relic', tone: 'gain', text: 'a relic' }
  if (outcome === 'CardPick') return { kind: 'card', tone: 'gain', text: 'a card for your Satchel' }
  if (outcome === 'Duplicate')
    return { kind: 'card', tone: 'gain', text: 'a copy of a card from your deck or Satchel' }
  if ('Gold' in outcome) return amount('gold', outcome.Gold)
  if ('Life' in outcome) return amount('life', outcome.Life)
  return null
}

function amount(kind: 'gold' | 'life', n: number): Chip {
  const tone: ChipTone = n > 0 ? 'gain' : n < 0 ? 'cost' : 'flat'
  return { kind, tone, text: n === 0 ? `no ${kind} change` : `${signed(n)} ${kind}` }
}

/** Outcomes as chips and chances. */
export function partOf(outcomes: readonly Outcome[]): Part {
  const part: Part = { chips: [], chances: [] }
  for (const outcome of outcomes) {
    const chip = chipOf(outcome)
    if (chip !== null) part.chips.push(chip)
    else if (typeof outcome === 'object' && 'Gamble' in outcome) {
      const { odds, win, lose } = outcome.Gamble
      part.chances.push({ odds, win: partOf(win), lose: partOf(lose) })
    }
  }
  return part
}

/** Whether a part holds nothing. */
export const isEmpty = (part: Part): boolean => part.chips.length === 0 && part.chances.length === 0

/** Why a hero with this gold and life cannot take an option, or `null` when they can. */
export function blockedReason(needs: Needs, gold: number, life: number): string | null {
  if (gold < needs.gold) return `Needs ${needs.gold} gold: you have ${gold}`
  if (needs.life > 0 && life <= needs.life) return `Needs more than ${needs.life} life: you have ${life}`
  return null
}

/** An option as the page draws it: its chips and chances, and what keeps this hero from it. */
export interface OptionView {
  label: string
  part: Part
  blocked: string | null
}

/** Each option of an event for a hero with this gold and life. */
export function optionViews(options: readonly DelveOptionView[], gold: number, life: number): OptionView[] {
  return options.map((option) => ({
    label: option.label,
    part: partOf(option.outcomes),
    blocked: blockedReason(option.needs, gold, life),
  }))
}

/** A result for the panel: who did what, what it gave or cost, and how its gambles fell. */
export interface ResultView {
  key: string
  mine: boolean
  who: string
  what: string
  chips: Chip[]
  /** One line per gamble: "Won the 60% gamble". */
  rolls: string[]
}

/** The panel's rows from the run's results: the viewer's own first, then the others' in the order they answered. */
export function resultViews(
  results: readonly Resolved[],
  heroes: readonly Hero[],
  me: number | undefined,
): ResultView[] {
  const rows = results.map((result, i): ResultView => {
    const mine = result.hero === me
    return {
      key: `${i}-${result.hero}`,
      mine,
      who: mine ? 'You' : (heroes[result.hero]?.name ?? 'A hero'),
      what: result.what,
      chips: result.outcomes.flatMap((o) => chipOf(o) ?? []),
      rolls: result.rolls.map((r) => `${r.won ? 'Won' : 'Lost'} the ${r.odds}% gamble`),
    }
  })
  return [...rows.filter((r) => r.mine), ...rows.filter((r) => !r.mine)]
}

/** A result in one sentence, for a screen reader: "You chose “Push past”: −8 life, +40 gold." */
export function resultSentence(row: ResultView): string {
  const facts = [...row.rolls, ...row.chips.map((c) => c.text)]
  return `${row.who} ${row.what}${facts.length > 0 ? `: ${facts.join(', ')}` : ''}.`
}
