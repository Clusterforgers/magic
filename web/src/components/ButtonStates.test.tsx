// Goal: the button keeps the target sizes of the design system (24, 32, 36 and 44 px, 44 under a coarse pointer), and its loading and aria-disabled states ignore presses yet stay focusable, while a disabled one is out of the tab order.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Button, buttonVariants } from '@/components/ui/button'

type Size = NonNullable<Parameters<typeof buttonVariants>[0]>['size']

describe('button sizes', () => {
  it.each<[NonNullable<Size>, string, string | null]>([
    ['xs', 'h-6', 'pointer-coarse:h-8'],
    ['sm', 'h-8', 'pointer-coarse:h-11'],
    ['default', 'h-9', 'pointer-coarse:h-11'],
    ['lg', 'h-11', null],
    ['icon-xs', 'size-6', 'pointer-coarse:size-8'],
    ['icon-sm', 'size-8', 'pointer-coarse:size-11'],
    ['icon', 'size-9', 'pointer-coarse:size-11'],
    ['icon-lg', 'size-11', null],
  ])('makes %s %s tall, and %s under a coarse pointer', (size, desktop, coarse) => {
    const classes = buttonVariants({ size }).split(' ')
    expect(classes).toContain(desktop)
    if (coarse !== null) expect(classes).toContain(coarse)
  })

  it('has no size under the 24 px floor of the design system', () => {
    for (const size of ['xs', 'sm', 'default', 'lg', 'icon-xs', 'icon-sm', 'icon', 'icon-lg'] as const) {
      const heights = buttonVariants({ size })
        .split(' ')
        .flatMap((name) => {
          const match = /^(?:h|size)-(\d+)$/.exec(name)
          return match === null ? [] : [Number(match[1]) * 4]
        })
      expect(heights, size).not.toHaveLength(0)
      expect(Math.min(...heights), size).toBeGreaterThanOrEqual(24)
    }
  })
})

describe('a loading button', () => {
  it('shows a spinner next to its words and says it is busy', () => {
    render(<Button loading>Saving…</Button>)
    const button = screen.getByRole('button', { name: 'Saving…' })
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(button).toHaveAttribute('data-loading')
    expect(button.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  })

  it('ignores presses but keeps the keyboard focus it had', async () => {
    const onClick = vi.fn()
    const user = userEvent.setup()
    render(
      <Button loading onClick={onClick}>
        Saving…
      </Button>,
    )
    const button = screen.getByRole('button')
    expect(button).toHaveAttribute('aria-disabled', 'true')
    expect(button).not.toBeDisabled()
    await user.click(button)
    await user.tab()
    button.focus()
    await user.keyboard('{Enter}')
    expect(onClick).not.toHaveBeenCalled()
    expect(button).toHaveFocus()
  })

  it('presses again once the wait is over', async () => {
    const onClick = vi.fn()
    const user = userEvent.setup()
    const { rerender } = render(
      <Button loading onClick={onClick}>
        Go
      </Button>,
    )
    rerender(<Button onClick={onClick}>Go</Button>)
    await user.click(screen.getByRole('button', { name: 'Go' }))
    expect(onClick).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button')).not.toHaveAttribute('aria-busy')
  })
})

describe('a button that cannot act', () => {
  it('stays focusable and silent when it is aria-disabled by focusableWhenDisabled', async () => {
    const onClick = vi.fn()
    const user = userEvent.setup()
    render(
      <Button disabled focusableWhenDisabled onClick={onClick}>
        Join
      </Button>,
    )
    const button = screen.getByRole('button', { name: 'Join' })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    await user.click(button)
    button.focus()
    expect(button).toHaveFocus()
    expect(onClick).not.toHaveBeenCalled()
  })

  it('leaves the tab order when it is disabled', async () => {
    const user = userEvent.setup()
    render(<Button disabled>Join</Button>)
    expect(screen.getByRole('button')).toBeDisabled()
    await user.tab()
    expect(screen.getByRole('button')).not.toHaveFocus()
  })
})
