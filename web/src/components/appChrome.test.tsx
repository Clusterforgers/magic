// Goal: a page that takes the whole window, or only fits it under the header, tells the shell while it is mounted (so the shell drops its header and the page cannot scroll, or keeps the header and is the window high), however many pages ask, each ask apart from the other, and tells it again when the page goes.
import { render, renderHook, act } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useFitWindow, useFullBleed, useIsFitWindow, useIsFullBleed } from './appChrome'

function Page() {
  useFullBleed()
  return <p>a page that is the whole window</p>
}

describe('useFullBleed', () => {
  it('is off until a page asks, on while it is mounted, and off again when it goes', () => {
    const shell = renderHook(() => useIsFullBleed())
    expect(shell.result.current).toBe(false)
    const page = render(<Page />)
    expect(shell.result.current).toBe(true)
    page.unmount()
    expect(shell.result.current).toBe(false)
  })

  it('counts the pages, so one leaving does not give the header back to a window another still holds', () => {
    const shell = renderHook(() => useIsFullBleed())
    const first = render(<Page />)
    const second = render(<Page />)
    first.unmount()
    expect(shell.result.current).toBe(true)
    second.unmount()
    expect(shell.result.current).toBe(false)
  })

  it('is seen by every shell that listens, and by one that starts listening late', () => {
    const early = renderHook(() => useIsFullBleed())
    const page = render(<Page />)
    const late = renderHook(() => useIsFullBleed())
    expect([early.result.current, late.result.current]).toEqual([true, true])
    act(() => page.unmount())
    expect([early.result.current, late.result.current]).toEqual([false, false])
  })
})

function FitPage() {
  useFitWindow()
  return <p>a page that fits the window</p>
}

describe('useFitWindow', () => {
  it('is off until a page asks, on while it is mounted, and off again when it goes', () => {
    const shell = renderHook(() => useIsFitWindow())
    expect(shell.result.current).toBe(false)
    const page = render(<FitPage />)
    expect(shell.result.current).toBe(true)
    page.unmount()
    expect(shell.result.current).toBe(false)
  })

  it('counts the pages, as the whole-window pages are counted', () => {
    const shell = renderHook(() => useIsFitWindow())
    const first = render(<FitPage />)
    const second = render(<FitPage />)
    first.unmount()
    expect(shell.result.current).toBe(true)
    second.unmount()
    expect(shell.result.current).toBe(false)
  })

  it('is a separate ask from the whole window: neither shows as the other', () => {
    const fit = renderHook(() => useIsFitWindow())
    const bleed = renderHook(() => useIsFullBleed())
    const page = render(<FitPage />)
    expect([fit.result.current, bleed.result.current]).toEqual([true, false])
    page.unmount()
    const whole = render(<Page />)
    expect([fit.result.current, bleed.result.current]).toEqual([false, true])
    whole.unmount()
  })
})
