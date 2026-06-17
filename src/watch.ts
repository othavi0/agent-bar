import { toJsonOutput } from './formatters/json';
import { logger } from './logger';
import { getAllQuotas, getQuotaFor } from './providers';
import type { AllQuotas } from './providers/types';

export interface StartWatchOptions {
  provider?: string;
  intervalMs: number;
}

async function fetchQuotas(provider?: string): Promise<AllQuotas> {
  if (provider) {
    const quota = await getQuotaFor(provider);
    return {
      providers: quota ? [quota] : [],
      fetchedAt: new Date().toISOString(),
    };
  }
  return getAllQuotas();
}

/** Serialize one quota snapshot as a single NDJSON line. */
export function buildWatchLine(quotas: AllQuotas): string {
  return `${JSON.stringify(toJsonOutput(quotas))}\n`;
}

/**
 * Long-running NDJSON emitter. Emits immediately, then every `intervalMs` after
 * the previous tick's write completes (backpressure-aware, no overlap/drift).
 * Exits 0 on EPIPE (consumer closed the pipe) or SIGTERM/SIGINT.
 * Never resolves on its own.
 */
export async function startWatch(opts: StartWatchOptions): Promise<void> {
  process.stdout.on('error', (err: Error & { code?: string }) => {
    if (err.code === 'EPIPE') process.exit(0);
  });

  if (process.stdout.isTTY) {
    process.stderr.write('[agent-bar] watch mode: output is NDJSON — pipe to a consumer\n');
  }

  let stopping = false;
  const stop = () => {
    stopping = true;
    process.exit(0);
  };
  process.on('SIGTERM', stop);
  process.on('SIGINT', stop);

  const tick = async (): Promise<void> => {
    if (stopping) return;
    try {
      const quotas = await fetchQuotas(opts.provider);
      process.stdout.write(buildWatchLine(quotas), () => {
        if (!stopping) setTimeout(tick, opts.intervalMs);
      });
    } catch (error) {
      logger.error('watch tick failed', { error });
      if (!stopping) setTimeout(tick, opts.intervalMs);
    }
  };

  await tick();
  // Keep the process alive; it exits only via signal or EPIPE.
  await new Promise<void>(() => {});
}
