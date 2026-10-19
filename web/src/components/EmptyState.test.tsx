// Goal: an empty state says why there is nothing and what to do next under a heading of the right level, draws its icon for the eye only, and leaves out what it was not given.
import { render, screen, within } from '@testing-library/react'
import { InboxIcon } from 'lucide-react'
import { describe, expect, it } from 'vitest'
import { EmptyState } from './EmptyState'

describe('EmptyState', () => {
  it('shows a title, the explanation and the way out', () => {
    render(
      <EmptyState icon={InboxIcon} title="No open games" action={<a href="/new">Create a game</a>}>
        Create one, or join with a code.
      </EmptyState>,
    )
    expect(screen.getByRole('heading', { level: 2, name: 'No open games' })).toBeInTheDocument()
    expect(screen.getByText('Create one, or join with a code.')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/new')
  })

  it('takes the heading level it is told', () => {
    render(<EmptyState title="Nothing here" as="h3" />)
    expect(screen.getByRole('heading', { level: 3, name: 'Nothing here' })).toBeInTheDocument()
  })

  it('hides its icon from a screen reader', () => {
    const { container } = render(<EmptyState icon={InboxIcon} title="Empty" />)
    expect(container.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  })

  it('draws only the title when it has nothing else', () => {
    const { container } = render(<EmptyState title="Empty" />)
    const state = container.querySelector('[data-slot="empty-state"]') as HTMLElement
    expect(state.querySelector('svg')).toBeNull()
    expect(state.querySelector('p')).toBeNull()
    expect(within(state).queryByRole('link')).toBeNull()
    expect(state.children).toHaveLength(1)
  })
})
