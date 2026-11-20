import { Link } from '@tanstack/react-router'
import { ChevronRightIcon, GraduationCapIcon } from 'lucide-react'
import { useStore } from 'zustand'
import { buttonVariants } from '@/components/ui/button'
import { COURSE } from './course'
import { doneStore } from './tutorials'

/** The Play page's way to the tutorials: what they are, how many are done here, and a link to `/learn`. */
export function LearnCard() {
  const done = useStore(
    doneStore,
    (state) => COURSE.filter((lesson) => state.done.includes(lesson.id)).length,
  )
  const all = done === COURSE.length
  return (
    <section
      data-slot="learn-card"
      aria-labelledby="learn-card-title"
      className="surface flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3"
    >
      <GraduationCapIcon aria-hidden="true" className="size-5 shrink-0 text-muted-foreground" />
      <div className="grid min-w-[12rem] flex-1 gap-0.5">
        <h2 id="learn-card-title" className="text-sm font-semibold">
          {done === 0 ? 'New here? Learn to play' : 'Learn'}
        </h2>
        <p className="text-sm text-muted-foreground">
          Seven short lessons at a practice table, then decks, lobbies and more.{' '}
          <span className="tabular-nums">
            {all ? 'All done.' : `${done} of ${COURSE.length} lessons done.`}
          </span>
        </p>
      </div>
      <Link
        to="/learn"
        className={buttonVariants({ variant: done === 0 ? 'default' : 'outline', size: 'sm' })}
      >
        Tutorials
        <ChevronRightIcon aria-hidden="true" />
      </Link>
    </section>
  )
}
