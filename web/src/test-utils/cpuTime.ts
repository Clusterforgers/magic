// Times work by the CPU the test's own thread spends on it, which other processes on a busy machine do not stretch the way they stretch the wall clock.

/** The CPU microseconds the calling thread has used so far (the process's on a Node without per-thread timing). */
const threadMicros = (): number => {
  const usage = typeof process.threadCpuUsage === 'function' ? process.threadCpuUsage() : process.cpuUsage()
  return usage.user + usage.system
}

/** The CPU milliseconds this thread spends running `work`; the turns other processes take meanwhile do not count. */
export function cpuMs(work: () => void): number {
  const start = threadMicros()
  work()
  return (threadMicros() - start) / 1000
}

/** The CPU milliseconds this thread spends on `work` up to its end; the waits between its steps cost nothing. */
export async function cpuMsAsync(work: () => Promise<void>): Promise<number> {
  const start = threadMicros()
  await work()
  return (threadMicros() - start) / 1000
}
