// Goal: an asking source that has left its zone keeps its picture: the chip's disc and the column's card draw the card the source last was (`AskingView.identity`) on the asking ability, while a source the view still holds is drawn as itself and an asker with no last-known card stays as it was.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { CardIdentity, ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { buildFixture } from '../../dev/fixture'
import { ArtDisc } from './AskerChip'
import { askerOf } from './asking'
import type { Asker } from './asking'
import { shownAsker } from './useAsker'

const card = must(buildFixture({ seats: 2 }).view.objects.find((object) => object.zone === 'Battlefield'))
const ability: ObjectView = {
  ...card,
  id: 900,
  kind: 'Ability',
  zone: 'Stack',
  source: 41,
  identity: undefined,
}
const last: CardIdentity = { oracle_id: '0000579f-7b35-4ed3-b44c-db2a538066fe', printing_id: 'p-1' }

const asker = (identity: CardIdentity | undefined, source?: number): Asker => ({
  source,
  stack: ability.id,
  resolving: true,
  name: 'Martyr',
  identity,
  text: undefined,
  clause: undefined,
  cause: undefined,
  run: undefined,
})

describe('the card of an asker', () => {
  it.each<[string, Asker, ObjectView | undefined, CardIdentity | undefined]>([
    ['a source that left, with its last-known card', asker(last), undefined, last],
    ['a source that left, with no last-known card', asker(undefined), undefined, undefined],
  ])('draws the ability for %s', (_what, given, source, identity) => {
    const shown = shownAsker(given, source, ability)
    expect(shown.object?.id).toBe(ability.id)
    expect(shown.object?.identity).toEqual(identity)
    expect(shown.board).toBeUndefined()
    expect(shown.shown).toBe('Martyr')
  })

  it('draws a source the view holds as itself', () => {
    const shown = shownAsker(asker(last, card.id), card, ability)
    expect(shown.object).toBe(card)
    expect(shown.board).toBe(card.id)
  })

  it('reads the last-known card off the decision', () => {
    const read = askerOf({
      decision: { player: 0, kind: { YesNo: { source: 41, text: 'Gain 1 life?' } } },
      asking: { stack: ability.id, name: 'Martyr', identity: last },
    } as never)
    expect(read?.identity).toEqual(last)
  })

  it("puts the last-known card's art in the chip's disc", () => {
    const shown = shownAsker(asker(last), undefined, ability)
    const { container } = render(<ArtDisc object={shown.object} name={shown.shown} />)
    expect(must(container.querySelector('img')).getAttribute('src')).toContain('p-1')
  })
})
