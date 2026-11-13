// Goal: a run in the list shows its party and how far down the ladder it got, one skull per boss with the beaten ones marked.
import { screen } from '@testing-library/react'
import { expect, it } from 'vitest'
import { renderInRouter } from '@/test-utils/router'
import { DelveRow } from './DelvesPage'

it('shows the bosses beaten as skulls', async () => {
  await renderInRouter(
    <ul>
      <DelveRow
        delve={{
          id: 'd1',
          code: 'C',
          theme: 'The Warrens',
          status: 'map',
          heroes: ['Ana', 'Bo'],
          cleared: 2,
          bosses: 5,
        }}
      />
    </ul>,
  )
  const track = screen.getByRole('img', { name: '2 of 5 bosses beaten' })
  const skulls = [...track.querySelectorAll('[data-beaten]')].map((s) => s.getAttribute('data-beaten'))
  expect(skulls).toEqual(['true', 'true', 'false', 'false', 'false'])
  expect(screen.getByText('Ana, Bo')).toBeInTheDocument()
  expect(screen.getByText('Choosing a path')).toBeInTheDocument()
})
