/** What an event option does and what a hero's answer did, read from the content's outcome data (`docs/delves.md`, Events). */
import type {
  Bane,
  CommanderFacts,
  DelveOptionView,
  Hero,
  Needs,
  Outcome,
  Rarity,
  Resolved,
} from '@/protocol/generated'
import { maxLifeOf } from './model'
import { upgradeWords } from './upgrades'

/** What a chip pictures. */
export type ChipKind = 'gold' | 'life' | 'cap' | 'relic' | 'card' | 'bane'

/** Whether a chip is something gained, paid or neither. */
export type ChipTone = 'gain' | 'cost' | 'flat'

/** One effect as a chip: an amount of gold, life or life cap, a relic (of a rarity), a card or a commander bane. */
export interface Chip {
  kind: ChipKind
  tone: ChipTone
  text: string
  /** The rarity a relic gain names, drawn in its tier's colour. */
  rarity?: Rarity
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

/** A bane in words for this commander, as the upgrades list says it: "Ruric costs {1} more to cast". */
export const baneWords = (bane: Bane, commander = 'Your commander'): string =>
  upgradeWords({ commander, boon: bane })

/** The chip of a sure outcome; `null` for a gamble, which is a chance instead. `commander` names a bane's target. */
export function chipOf(outcome: Outcome, commander?: string): Chip | null {
  if (outcome === 'Relic') return { kind: 'relic', tone: 'gain', text: 'a relic' }
  if (outcome === 'CardPick') return { kind: 'card', tone: 'gain', text: 'a card for your Satchel' }
  if (outcome === 'Duplicate')
    return { kind: 'card', tone: 'gain', text: 'a copy of a card from your deck or Satchel' }
  if (outcome === 'LoseCard')
    return { kind: 'card', tone: 'cost', text: 'give up a card of your deck or Satchel' }
  if (outcome === 'LoseRelic') return { kind: 'relic', tone: 'cost', text: 'give up a relic' }
  if ('Gold' in outcome) return amount('gold', outcome.Gold)
  if ('Life' in outcome) return amount('life', outcome.Life)
  if ('MaxLife' in outcome) {
    const n = outcome.MaxLife
    const tone: ChipTone = n > 0 ? 'gain' : n < 0 ? 'cost' : 'flat'
    return { kind: 'cap', tone, text: `${signed(n)} max life for the run` }
  }
  if ('RelicOf' in outcome) {
    const rarity = outcome.RelicOf
    return { kind: 'relic', tone: 'gain', text: `a ${rarity} relic`, rarity }
  }
  if ('Bane' in outcome) return { kind: 'bane', tone: 'cost', text: baneWords(outcome.Bane, commander) }
  return null
}

function amount(kind: 'gold' | 'life', n: number): Chip {
  const tone: ChipTone = n > 0 ? 'gain' : n < 0 ? 'cost' : 'flat'
  return { kind, tone, text: n === 0 ? `no ${kind} change` : `${signed(n)} ${kind}` }
}

/** Outcomes as chips and chances; `commander` names a bane's target. */
export function partOf(outcomes: readonly Outcome[], commander?: string): Part {
  const part: Part = { chips: [], chances: [] }
  for (const outcome of outcomes) {
    const chip = chipOf(outcome, commander)
    if (chip !== null) part.chips.push(chip)
    else if (typeof outcome === 'object' && 'Gamble' in outcome) {
      const { odds, win, lose } = outcome.Gamble
      part.chances.push({ odds, win: partOf(win, commander), lose: partOf(lose, commander) })
    }
  }
  return part
}

/** Whether a part holds nothing. */
export const isEmpty = (part: Part): boolean => part.chips.length === 0 && part.chances.length === 0

/** The least life cap a cut may leave a hero (`mtg-campaign`, content.rs). */
export const MIN_LIFE_CAP = 20

/** What gates a hero's options: their gold and life, and, when known, their life cap, relics and first commander. */
export interface HeroFacts {
  gold: number
  life: number
  maxLife?: number
  relics?: number
  /** The deck's first commander; `null` for a deck without one, absent when not known. */
  commander?: CommanderFacts | null
}

/** The facts of a hero of the run. */
export function heroFacts(hero: Hero): HeroFacts {
  return {
    gold: hero.gold,
    life: hero.life,
    maxLife: maxLifeOf(hero),
    relics: hero.relics.length,
    commander: hero.deck === null ? undefined : (hero.deck.commanders[0] ?? null),
  }
}

/** Why a hero cannot take an option, or `null` when they can; an unknown fact never shuts one. */
export function blockedReason(needs: Needs, hero: HeroFacts): string | null {
  const { gold, life, maxLife, relics, commander } = hero
  if (gold < needs.gold) return `Needs ${needs.gold} gold: you have ${gold}`
  if (needs.life > 0 && life <= needs.life) return `Needs more than ${needs.life} life: you have ${life}`
  if (needs.max_life > 0 && maxLife !== undefined && maxLife - needs.max_life < MIN_LIFE_CAP)
    return `Needs a max life of ${MIN_LIFE_CAP + needs.max_life}: yours is ${maxLife}`
  if (needs.relic && relics === 0) return 'Needs a relic to give up: you have none'
  if ((needs.commander || needs.creature_commander) && commander === null)
    return 'Needs a commander: your deck has none'
  if (needs.creature_commander && commander && !commander.creature)
    return `Needs a creature commander: ${commander.name} is not a creature`
  return null
}

/** An option as the page draws it: what it costs and what it gives, side by side, and what keeps this hero from it. */
export interface OptionView {
  label: string
  /** Everything it does. */
  part: Part
  /** Its sure costs; none for a free option. */
  cost: Chip[]
  /** Its sure gains and its chances. */
  reward: Part
  blocked: string | null
}

/** Each option of an event for this hero. */
export function optionViews(options: readonly DelveOptionView[], hero: HeroFacts): OptionView[] {
  const commander = hero.commander?.name
  return options.map((option) => {
    const part = partOf(option.outcomes, commander)
    return {
      label: option.label,
      part,
      cost: part.chips.filter((c) => c.tone === 'cost'),
      reward: { chips: part.chips.filter((c) => c.tone !== 'cost'), chances: part.chances },
      blocked: blockedReason(option.needs, hero),
    }
  })
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
      chips: result.outcomes.flatMap((o) => chipOf(o, heroes[result.hero]?.deck?.commanders[0]?.name) ?? []),
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
