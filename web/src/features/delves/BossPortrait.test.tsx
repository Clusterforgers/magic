// Goal: a boss's portrait is the art crop of the real card its content names, and its colours with a crown when it has no art card or the picture fails, at every size the descent and the fight panel draw it.
import { fireEvent, render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { BossArtView } from '@/protocol/generated'
import { BossPortrait } from './CommanderArt'

const KRENKO: BossArtView = { card: 'Krenko, Tin Street Kingpin', oracle_id: 'o-krenko' }

const portrait = (container: HTMLElement) => container.querySelector('[data-slot="commander-art"]')

describe('a boss portrait', () => {
  it.each(['size-14 rounded-full', 'size-20 rounded-full'])('shows the art card at %s', (className) => {
    const { container } = render(<BossPortrait colours="BR" art={KRENKO} className={className} />)
    const img = portrait(container)?.querySelector('img')
    expect(img).toHaveAttribute('src', '/api/images/oracle/o-krenko/0/art_crop')
    expect(portrait(container)?.querySelector('.lucide-crown')).toBeNull()
  })

  it.each<[string, BossArtView | undefined]>([
    ['no art card', undefined],
    ['a picture that fails', KRENKO],
  ])('falls back to its colours with a crown for %s', (_, art) => {
    const { container } = render(<BossPortrait colours="B" art={art} />)
    const img = portrait(container)?.querySelector('img')
    if (img) fireEvent.error(img)
    expect(portrait(container)?.querySelector('img')).toBeNull()
    expect(portrait(container)?.querySelector('.lucide-crown')).not.toBeNull()
  })
})
