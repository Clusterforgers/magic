// Goal: the disc of a chip or a row shows the card's own art crop where the card is identified, the first letter of its name where it is not or the picture fails, and never a name to a screen reader, which the chip says in words.
import { fireEvent, render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { buildFixture } from '../../dev/fixture'
import { ArtDisc } from './AskerChip'

const card = must(buildFixture({ seats: 2 }).view.objects.find((object) => object.zone === 'Battlefield'))
const identified: ObjectView = { ...card, identity: { oracle_id: '0000579f-7b35-4ed3-b44c-db2a538066fe' } }

describe('the art disc', () => {
  it.each<[string, ObjectView | undefined, string]>([
    ['a card without a picture', card, 'Mentor of the Meek'],
    ['a card the view does not hold', undefined, 'Brago, King Eternal'],
    ['no name', undefined, ''],
  ])('draws the first letter for %s', (_what, object, name) => {
    const { container } = render(<ArtDisc object={object} name={name} />)
    const disc = must(container.querySelector('[data-slot="art-disc"]'))
    expect(disc).toHaveAttribute('aria-hidden', 'true')
    expect(disc.querySelector('img')).toBeNull()
    expect(disc).toHaveTextContent(name === '' ? '?' : (name[0] ?? ''))
  })

  it("draws an identified card's art crop, and its letter once the picture fails", () => {
    const { container } = render(<ArtDisc object={identified} name="Mentor of the Meek" />)
    const picture = must(container.querySelector('img'))
    expect(picture.getAttribute('src')).toContain('art_crop')
    expect(picture.getAttribute('src')).toContain('0000579f-7b35-4ed3-b44c-db2a538066fe')
    expect(picture).toHaveAttribute('alt', '')
    fireEvent.error(picture)
    expect(container.querySelector('img')).toBeNull()
    expect(must(container.querySelector('[data-slot="art-disc"]'))).toHaveTextContent('M')
  })
})
