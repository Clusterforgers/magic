/** The permanents and cards that pay for a spell without making mana, as the lines of buttons the payment shows. */
import type { ObjectId } from '@/protocol/generated'
import { manaName } from './payText'
import type { Offer, OfferRow } from './PaymentSummary'
import { usedBy } from './payment'
import type { Spend } from './payment'

/** The lines for convoke and improvise: only what is not in the payment yet, and each way marked usable by `usable`. */
export function offersOf(
  pieces: readonly Spend[],
  chosen: readonly Spend[],
  nameOf: (id: ObjectId) => string,
  usable: (spend: Spend) => boolean,
): OfferRow[] {
  const taken = new Set(chosen.flatMap((spend) => usedBy(spend) ?? []))
  const rows = new Map<string, Map<ObjectId, Offer & { choices: Offer['choices'][number][] }>>()
  const add = (label: string, id: ObjectId, spend: Spend, label2: string, aria: string): void => {
    if (taken.has(usedBy(spend) ?? '')) return
    const row = rows.get(label) ?? new Map()
    rows.set(label, row)
    const offer = row.get(id) ?? { key: `${label}-${id}`, name: nameOf(id), choices: [] }
    row.set(id, offer)
    offer.choices.push({
      label: label2,
      aria,
      ...(spend.kind === 'convoke' && spend.pays !== null ? { mana: spend.pays } : {}),
      spend,
      usable: usable(spend),
    })
  }
  for (const spend of pieces) {
    if (spend.kind === 'convoke') {
      const name = nameOf(spend.creature)
      const pays = spend.pays === null ? '{1}' : manaName(spend.pays)
      add(
        'Convoke',
        spend.creature,
        spend,
        spend.pays === null ? '1' : manaName(spend.pays),
        `Tap ${name} to convoke for ${pays}`,
      )
    }
    if (spend.kind === 'improvise') {
      add('Improvise', spend.artifact, spend, '1', `Tap ${nameOf(spend.artifact)} to improvise for {1}`)
    }
    if (spend.kind === 'delve') {
      add('Delve', spend.card, spend, '1', `Exile ${nameOf(spend.card)} to delve for {1}`)
    }
  }
  return [...rows].map(([label, offers]) => ({ label, offers: [...offers.values()] }))
}
