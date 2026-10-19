// Goal: a skeleton is a grey block hidden from assistive technology that pulses only where motion is welcome, and a tooltip shows its words on hover and on keyboard focus while the trigger keeps its own accessible name.
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { Skeleton } from '@/components/ui/skeleton'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'

describe('Skeleton', () => {
  it('is a muted block that a screen reader skips', () => {
    const { container } = render(<Skeleton className="h-8 w-40" />)
    const block = container.firstElementChild
    expect(block).toHaveAttribute('aria-hidden', 'true')
    expect(block).toHaveAttribute('data-slot', 'skeleton')
    expect(block).toHaveClass('bg-muted', 'h-8', 'w-40')
  })

  it('pulses only inside the no-preference media query', () => {
    const { container } = render(<Skeleton />)
    const classes = (container.firstElementChild?.className ?? '').split(' ')
    expect(classes).toContain('motion-safe:animate-pulse')
    expect(classes).not.toContain('animate-pulse')
  })
})

describe('Tooltip', () => {
  function Example() {
    return (
      <TooltipProvider delay={0}>
        <Tooltip>
          <TooltipTrigger render={<button type="button" aria-label="Theme: System" />}>icon</TooltipTrigger>
          <TooltipContent>Theme</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    )
  }

  it('shows its words when the pointer rests on the trigger, and hides them when it leaves', async () => {
    const user = userEvent.setup()
    render(<Example />)
    expect(screen.queryByText('Theme')).toBeNull()
    await user.hover(screen.getByRole('button'))
    expect(await screen.findByText('Theme')).toHaveAttribute('data-slot', 'tooltip-content')
    await user.unhover(screen.getByRole('button'))
    await waitFor(() => expect(screen.queryByText('Theme')).toBeNull())
  })

  it('shows its words when the trigger takes keyboard focus', async () => {
    const user = userEvent.setup()
    render(<Example />)
    await user.tab()
    expect(screen.getByRole('button')).toHaveFocus()
    expect(await screen.findByText('Theme')).toBeInTheDocument()
  })

  it('leaves the accessible name of the trigger to the trigger', async () => {
    render(<Example />)
    expect(screen.getByRole('button', { name: 'Theme: System' })).toBeInTheDocument()
  })
})
