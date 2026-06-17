import { toJsonOutput } from './formatters/json';
import { logger } from './logger';
import { getAllQuotas, getProvider, getQuotaFor } from './providers';
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
  if (opts.provider && !getProvider(opts.provider)) {
    process.stderr.write(`[agent-bar] Unknown provider: ${opts.provider}\n`);
    process.exit(1);
  }

  process.stdout.on('error', (err: Error & { code?: string }) => {
    if (err.code === 'EPIPE') process.exit(0);
  });

  if (process.stdout.isTTY) {
    process.stderr.write('[agent-bar] watch mode: output is NDJSON — pipe to a consumer\n');
  }

  const tick = async (): Promise<void> => {
    try {
      const quotas = await fetchQuotas(opts.provider);
      process.stdout.write(buildWatchLine(quotas), (writeErr?: Error | null) => {
        if (!writeErr) {
          setTimeout(tick, opts.intervalMs);
          return;
        }
        // EPIPE is handled by the stdout 'error' listener (exits 0); any other
        // write error is fatal — log and exit rather than hang.
        if ((writeErr as Error & { code?: string }).code !== 'EPIPE') {
          logger.error('stdout write error', { writeErr });
          process.exit(1);
        }
      });
    } catch (error) {
      logger.error('watch tick failed', { error });
      setTimeout(tick, opts.intervalMs);
    }
  };

  await tick();
  // Keep the process alive; it exits only via signal or EPIPE.
  await new Promise<void>(() => {});
}
