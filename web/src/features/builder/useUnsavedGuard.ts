import { useBlocker } from '@tanstack/react-router'
import { useEffect } from 'react'
import type { Resolver } from './LeaveDialog'

/**
 * Guards unsaved changes. A navigation inside the app is held by the router until the returned resolver says go on or
 * back (`LeaveDialog` asks); leaving the page (reload, closing the tab) gets the browser's own prompt. Nothing is
 * guarded while `dirty` is false.
 */
export function useUnsavedGuard(dirty: boolean): Resolver {
  const blocker = useBlocker({
    shouldBlockFn: () => true,
    disabled: !dirty,
    // The router only listens for this on a browser history; the effect below does it for every history.
    enableBeforeUnload: false,
    withResolver: true,
  })
  useEffect(() => {
    if (!dirty) return
    const ask = (event: BeforeUnloadEvent): void => {
      event.preventDefault()
      event.returnValue = ''
    }
    window.addEventListener('beforeunload', ask)
    return () => window.removeEventListener('beforeunload', ask)
  }, [dirty])
  return blocker
}
