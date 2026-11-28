/**
 * The custom cards of a Delves fight as the card preview draws them (pure): a boss ability or an emblem of a command
 * zone matched by name to its card in the campaign content, else made from the words the table holds.
 */
import type {
  CardKind,
  CustomCardView,
  DelveContent,
  ObjectId,
  ObjectView,
  PlayerView,
} from '@/protocol/generated'
import type { CustomShown } from '../preview'
import { bossAbilities } from './bossAbilities'
import type { BossAbility, BossAbilityKind } from './bossAbilities'

/** The card kind of each boss ability kind, for a card the content does not hold. */
const KIND_CARD: Record<BossAbilityKind, CardKind> = {
  innate: 'Innate',
  modifier: 'Encounter',
  relic: 'Relic',
  phase: 'Phase',
  'next-phase': 'Phase',
  personality: 'Innate',
}

/** The content's custom card named `name`; undefined when the content is not known or holds none. */
export function customCardNamed(content: DelveContent | null, name: string): CustomCardView | undefined {
  return content?.cards.find((card) => card.name === name)
}

/** A card made from a boss ability's own words, for one the content does not hold (yet). */
function madeCard(ability: BossAbility): CustomCardView {
  const kind = KIND_CARD[ability.kind]
  return {
    id: ability.key,
    name: ability.card,
    kind,
    type_line: ability.kind === 'personality' ? 'Boss personality' : kind,
    text: ability.text,
    art: '',
    role: '',
    price: 0,
    rarity: 'Common',
  }
}

/** The card of `ability` with the table's words: its rules as the boss plays them, at its level. */
function cardOf(ability: BossAbility, content: DelveContent | null): CustomCardView {
  const known = customCardNamed(content, ability.card)
  return known === undefined ? madeCard(ability) : { ...known, text: ability.text || known.text }
}

/** The kicker of a boss ability: "Innate · Level III", "Phase 1 · in force", "Phase 2 at round 14", "Boss relic". */
export function abilityKicker(ability: BossAbility, kindWords: Record<BossAbilityKind, string>): string {
  if (ability.kind === 'phase') return `${ability.when ?? 'Phase'} · in force`
  if (ability.kind === 'next-phase') return ability.when ?? 'Next phase'
  return ability.when === undefined ? kindWords[ability.kind] : `${kindWords[ability.kind]} · ${ability.when}`
}

/**
 * The custom card the preview shows for `ability` of a boss whose command zone is `place` ("In Cy's command zone"); a
 * phase to come is in no zone yet. A phase carries the boss's phases in order, so the preview can step between them.
 */
export function abilityShown(
  ability: BossAbility,
  all: readonly BossAbility[],
  content: DelveContent | null,
  place: string,
  kindWords: Record<BossAbilityKind, string>,
): CustomShown {
  const one = (each: BossAbility): CustomShown => ({
    card: cardOf(each, content),
    kicker: abilityKicker(each, kindWords),
    ...(each.kind === 'next-phase' ? {} : { place }),
  })
  const shown = one(ability)
  if (ability.kind !== 'phase' && ability.kind !== 'next-phase') return shown
  const phases = all.filter((each) => each.kind === 'phase' || each.kind === 'next-phase').map(one)
  return { ...shown, phases }
}

/** The custom card an emblem of a command zone is: the content's card of its name; undefined for any other object. */
export function emblemCard(
  object: ObjectView | undefined,
  content: DelveContent | null,
): CustomCardView | undefined {
  if (object?.kind !== 'Emblem') return undefined
  return customCardNamed(content, object.name)
}

/**
 * A stable id below zero for a custom card no object stands for (a phase to come), which the preview store keys by;
 * the table's object ids are never negative.
 */
export function customPreviewId(key: string): ObjectId {
  let hash = 0
  for (const char of key) hash = (Math.imul(hash, 31) + (char.codePointAt(0) ?? 0)) | 0
  return -1 - (Math.abs(hash) % 1_000_000_000)
}

/**
 * What the preview shows for the emblem `card` of the seat `owner` at `place`: a boss's emblem as its ability (its
 * kicker, a phase with the boss's phases), any other (a hero's relic) as its card alone.
 */
export function emblemShown(
  card: CustomCardView,
  owner: PlayerView | undefined,
  content: DelveContent | null,
  place: string,
  kindWords: Record<BossAbilityKind, string>,
): CustomShown {
  const all = owner === undefined ? [] : bossAbilities(owner)
  const ability = all.find((each) => each.card === card.name)
  if (ability === undefined) return { card, place }
  return abilityShown(ability, all, content, place, kindWords)
}
