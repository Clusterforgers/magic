import { useEffect } from 'react'

/** Sets the tab title to "<title> · Magic" while the calling page is mounted. */
export function usePageTitle(title: string): void {
  useEffect(() => {
    const previous = document.title
    document.title = `${title} · Magic`
    return () => {
      document.title = previous
    }
  }, [title])
}
