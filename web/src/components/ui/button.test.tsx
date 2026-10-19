// Goal: the classes `buttonVariants` gives a link are merged like a `Button`'s, so an outline link keeps its strong border and a large link its larger text instead of the base's transparent border and small text.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Button, buttonVariants } from './button'

const classesOf = (value: string): string[] => value.split(/\s+/).filter(Boolean)

describe('buttonVariants', () => {
  it('lets the outline variant replace the base border colour', () => {
    const classes = classesOf(buttonVariants({ variant: 'outline' }))
    expect(classes).toContain('border-border-strong')
    expect(classes).not.toContain('border-transparent')
  })

  it('lets the large size replace the base text size', () => {
    const classes = classesOf(buttonVariants({ size: 'lg' }))
    expect(classes).toContain('text-base')
    expect(classes).not.toContain('text-sm')
  })

  it('lets a class given by the caller win over the variant', () => {
    const classes = classesOf(buttonVariants({ size: 'lg', className: 'px-2' }))
    expect(classes).toContain('px-2')
    expect(classes).not.toContain('px-6')
  })

  it('gives a Button the same classes as a link of the same variant', () => {
    render(<Button variant="outline">Open</Button>)
    const button = classesOf(screen.getByRole('button', { name: 'Open' }).className)
    for (const name of classesOf(buttonVariants({ variant: 'outline' }))) expect(button).toContain(name)
  })
})
