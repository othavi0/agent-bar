# qbar - Fix & Improve Task

You are working on **qbar**, an LLM quota monitor for Waybar (TypeScript/Bun).

Read the full codebase first (`src/` directory) before making changes.

## Task Overview

There are 5 tasks. Do them in order. After each task, verify it compiles with `bun x tsc --noEmit`.

---

## Task 1: Fix `waitEnter()` in `src/tui/login-single.ts`

### Problem
The `waitEnter()` function uses `process.stdin.setRawMode(true)` which doesn't work when launched via `qbar-open-terminal` (bash -lc wrapper). The Enter key just adds newlines instead of closing the terminal.

### Fix
Replace the `waitEnter()` function with a simpler approach that works in both raw and non-raw mode:

```typescript
async function waitEnter(): Promise<void> {
  p.log.info(colorize('Press Enter to continue...', semantic.subtitle));
  return new Promise<void>((resolve) => {
    const rl = (await import('node:readline')).createInterface({ input: process.stdin });
    rl.once('line', () => { rl.close(); resolve(); });
  });
}
```

Wait — that uses top-level await inside a non-async arrow. Instead, do it like this:

**File: `src/tui/login-single.ts`**

Replace the current `waitEnter` function with:

```typescript
async function waitEnter(): Promise<void> {
  const { createInterface } = await import('node:readline');
  p.log.info(colorize('Press Enter to continue...', semantic.subtitle));
  return new Promise<void>((resolve) => {
    const rl = createInterface({ input: process.stdin });
    rl.once('line', () => { rl.close(); resolve(); });
  });
}
```

This uses `readline` which handles Enter properly in all terminal contexts, including when launched via bash wrappers.

### Verify
- `bun x tsc --noEmit` passes
- No other files need changes for this task

---

## Task 2: Fix `refresh.ts` auto-close

### Problem
`src/refresh.ts` currently does `console.log('\n\x1b[2m(closing in 5s...)\x1b[0m')` and `await Bun.sleep(5000)`. This is okay but should also use the same approach as login for consistency.

### Fix
**File: `src/refresh.ts`**

Replace the last section (after the `pkill` line) with:

```typescript
// Auto-close after showing results
console.log('\n\x1b[2m(closing in 3s or press Enter...)\x1b[0m');
await Promise.race([
  Bun.sleep(3000),
  new Promise<void>((resolve) => {
    try {
      const { createInterface } = require('node:readline');
      const rl = createInterface({ input: process.stdin });
      rl.once('line', () => { rl.close(); resolve(); });
    } catch { /* ignore if stdin unavailable */ }
  }),
]);
```

This lets the user press Enter to close immediately OR auto-closes after 3 seconds.

### Verify
- `bun x tsc --noEmit` passes

---

## Task 3: Add Antigravity token auto-refresh

### Problem
Antigravity OAuth tokens expire after ~1 hour. The current code in `src/providers/antigravity.ts` checks if expired and just falls through, returning "Not logged in". It should try to refresh the token using the refresh_token.

### Fix
**File: `src/providers/antigravity.ts`**

Add a `refreshToken` method to `AntigravityProvider`:

```typescript
private async refreshAccessToken(tokens: TokenData, source: 'antigravity-usage' | 'qbar'): Promise<TokenData | null> {
  if (!tokens.refreshToken) return null;
  
  try {
    const response = await fetch('https://oauth2.googleapis.com/token', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({
        client_id: '590579400786-a7rn11flab8l5b7maoq7hg0akn06aoc7.apps.googleusercontent.com',
        grant_type: 'refresh_token',
        refresh_token: tokens.refreshToken,
      }),
    });

    if (!response.ok) {
      logger.debug('Token refresh failed', { status: response.status });
      return null;
    }

    const data: any = await response.json();
    const refreshed: TokenData = {
      ...tokens,
      accessToken: data.access_token,
      expiresAt: Date.now() + (data.expires_in * 1000),
    };
    if (data.refresh_token) {
      refreshed.refreshToken = data.refresh_token;
    }

    // Write back to source
    await this.saveTokens(refreshed, source);
    return refreshed;
  } catch (error) {
    logger.debug('Token refresh error', { error });
    return null;
  }
}

private async saveTokens(tokens: TokenData, source: 'antigravity-usage' | 'qbar'): Promise<void> {
  try {
    if (source === 'antigravity-usage') {
      const configFile = Bun.file(`${this.antigravityUsagePath}/config.json`);
      const config: AntigravityConfig = await configFile.json();
      const tokenPath = `${this.antigravityUsagePath}/accounts/${config.activeAccount}/tokens.json`;
      await Bun.write(tokenPath, JSON.stringify(tokens, null, 2));
    } else {
      const file = Bun.file(this.qbarAuthPath);
      let auth: any = {};
      if (await file.exists()) auth = await file.json();
      auth.antigravity = tokens;
      await Bun.write(this.qbarAuthPath, JSON.stringify(auth, null, 2));
    }
  } catch (error) {
    logger.debug('Failed to save refreshed tokens', { error });
  }
}
```

Then modify the `getTokens()` method. Where it says `logger.debug('antigravity-usage tokens expired');` and falls through, instead try to refresh:

```typescript
// In the antigravity-usage section, after checking expiresAt:
if (tokens.expiresAt > Date.now()) {
  return tokens;
}
// Token expired, try refresh
logger.debug('antigravity-usage tokens expired, attempting refresh');
const refreshed = await this.refreshAccessToken(tokens, 'antigravity-usage');
if (refreshed) return refreshed;
```

Do the same for the qbar auth section.

**IMPORTANT:** The `client_id` above is Google's public OAuth client ID used by the antigravity-usage CLI. You can verify it by checking the antigravity-usage source, but this is the standard Google AI Studio OAuth client ID.

Actually, let me reconsider. The client_id might be specific to the antigravity-usage app. Instead of hardcoding it, read it from the token file if available, or use a well-known approach:

Check if there's a `client_id` field in the token data. If not, we need to find it. For now, use this approach:

```typescript
// Read the client_id from the antigravity-usage config or use the known one
private readonly GOOGLE_CLIENT_ID = '590579400786-a7rn11flab8l5b7maoq7hg0akn06aoc7.apps.googleusercontent.com';
```

If refresh fails, the code already falls through to return null (triggering "Not logged in"), which is fine — the user can re-login.

### Verify
- `bun x tsc --noEmit` passes
- The token data structure in `~/.config/antigravity-usage/accounts/<email>/tokens.json` must have `refreshToken` field

---

## Task 4: Consume new API fields for Claude and Codex

### 4a. Claude: new per-model weekly quotas

**File: `src/providers/claude.ts`**

The Claude API now returns these additional fields (currently null but may be populated in the future):
- `seven_day_opus` — same shape as `seven_day`
- `seven_day_sonnet` — same shape as `seven_day`
- `seven_day_cowork` — same shape as `seven_day`

Update `ClaudeUsageResponse`:

```typescript
interface ClaudeUsageResponse {
  five_hour?: {
    utilization: number;
    resets_at?: string;
  };
  seven_day?: {
    utilization: number;
    resets_at?: string;
  };
  seven_day_opus?: {
    utilization: number;
    resets_at?: string;
  } | null;
  seven_day_sonnet?: {
    utilization: number;
    resets_at?: string;
  } | null;
  seven_day_cowork?: {
    utilization: number;
    resets_at?: string;
  } | null;
  extra_usage?: {
    is_enabled: boolean;
    monthly_limit: number;
    used_credits: number;
    utilization: number;
  };
  error?: {
    error_code: string;
    message: string;
  };
}
```

Update `ProviderQuota` in `src/providers/types.ts` — add a `weeklyModels` field:

```typescript
export interface ProviderQuota {
  // ... existing fields ...
  /** Per-model weekly quotas (Claude Pro feature) */
  weeklyModels?: Record<string, QuotaWindow>;
}
```

In `ClaudeProvider.getQuota()`, after parsing `secondary`, add:

```typescript
// Parse per-model weekly quotas
const weeklyModels: Record<string, QuotaWindow> = {};

if (usage.seven_day_opus) {
  const used = Math.round(usage.seven_day_opus.utilization);
  weeklyModels['Opus'] = {
    remaining: 100 - used,
    resetsAt: usage.seven_day_opus.resets_at || null,
  };
}

if (usage.seven_day_sonnet) {
  const used = Math.round(usage.seven_day_sonnet.utilization);
  weeklyModels['Sonnet'] = {
    remaining: 100 - used,
    resetsAt: usage.seven_day_sonnet.resets_at || null,
  };
}

if (usage.seven_day_cowork) {
  const used = Math.round(usage.seven_day_cowork.utilization);
  weeklyModels['Cowork'] = {
    remaining: 100 - used,
    resetsAt: usage.seven_day_cowork.resets_at || null,
  };
}
```

And include `weeklyModels: Object.keys(weeklyModels).length > 0 ? weeklyModels : undefined` in the return value.

Then update both formatters (`src/formatters/waybar.ts` and `src/formatters/terminal.ts`) to show per-model weekly quotas when `weeklyModels` is present. In the Claude tooltip builders, after the secondary (weekly) section, add:

```typescript
// Per-model weekly (if available)
if (p.weeklyModels && Object.keys(p.weeklyModels).length > 0) {
  // Replace the single "All Models" weekly line with per-model lines
  // Show each model separately instead of the generic weekly
}
```

**Logic:** If `weeklyModels` has entries, show them INSTEAD of the generic "All Models" weekly line. If `weeklyModels` is empty/undefined, show the existing generic weekly line.

### 4b. Codex: credits field

**File: `src/providers/codex.ts`**

The Codex API returns a `credits` field:
```json
{
  "credits": {
    "has_credits": false,
    "unlimited": false,
    "balance": "0"
  },
  "plan_type": null
}
```

Update `CodexRateLimits` interface:

```typescript
interface CodexRateLimits {
  primary: {
    used_percent: number;
    window_minutes: number;
    resets_at: number;
  };
  secondary: {
    used_percent: number;
    window_minutes: number;
    resets_at: number;
  };
  credits?: {
    has_credits: boolean;
    unlimited: boolean;
    balance: string;
  };
  plan_type?: string | null;
}
```

Update `ProviderQuota` type (or use existing `extraUsage`) to expose Codex credits. Add to the return value in `getQuota()`:

```typescript
// Include credits info if available
let codexCredits: ProviderQuota['extraUsage'] | undefined;
if (limits.credits?.has_credits || parseFloat(limits.credits?.balance || '0') > 0) {
  const balance = parseFloat(limits.credits!.balance);
  codexCredits = {
    enabled: true,
    remaining: limits.credits!.unlimited ? 100 : Math.min(100, Math.round(balance)),
    limit: limits.credits!.unlimited ? -1 : 0, // unknown total
    used: 0,
  };
}
```

And include `extraUsage: codexCredits` in the return.

Update the Codex tooltip in both formatters to show credits when available.

### Verify
- `bun x tsc --noEmit` passes
- Run `bun src/index.ts -t` to see terminal output

---

## Task 5: Add Amp provider

### Overview
Create a new provider for **Amp** (ampcode.com) that monitors free tier and individual credits.

### 5a. Create `src/providers/amp.ts`

The Amp CLI (`~/.cache/.bun/bin/amp`) has a `usage` command that outputs text like:
```
Signed in as othavioquiliao@gmail.com (othavio)
Amp Free: $20/$20 remaining (replenishes +$0.83/hour) [+100% bonus for 59 more days] - https://ampcode.com/settings#amp-free
Individual credits: $0 remaining - https://ampcode.com/settings
```

Create a provider that:
1. Checks if Amp is installed (look for `~/.cache/.bun/bin/amp` or `amp` in PATH via `Bun.which`)
2. Checks if logged in (run `amp usage` and check for "Signed in")
3. Parses the output to extract:
   - **Free tier:** remaining/total dollars, replenish rate, bonus info
   - **Individual credits:** remaining dollars
4. Converts dollar amounts to percentage for the bar display (remaining/total * 100)

```typescript
import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

function findAmpBin(): string | null {
  if (typeof Bun.which === 'function') {
    const found = Bun.which('amp');
    if (found) return found;
  }
  const home = process.env.HOME ?? '';
  const paths = [
    `${home}/.cache/.bun/bin/amp`,
    `${home}/.bun/bin/amp`,
  ];
  // Use sync check
  const { existsSync } = require('node:fs');
  for (const p of paths) {
    if (existsSync(p)) return p;
  }
  return null;
}

export class AmpProvider implements Provider {
  readonly id = 'amp';
  readonly name = 'Amp';

  async isAvailable(): Promise<boolean> {
    return findAmpBin() !== null;
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    const bin = findAmpBin();
    if (!bin) {
      return { ...base, error: 'Amp CLI not installed' };
    }

    try {
      return await cache.getOrFetch<ProviderQuota>(
        'amp-quota',
        async () => await this.fetchUsage(base, bin),
        CONFIG.cache.ttlMs
      );
    } catch (error) {
      logger.error('Amp quota fetch error', { error });
      return { ...base, error: 'Failed to fetch usage' };
    }
  }

  private async fetchUsage(base: ProviderQuota, bin: string): Promise<ProviderQuota> {
    try {
      const proc = Bun.spawn([bin, 'usage'], {
        stdout: 'pipe',
        stderr: 'pipe',
        env: { ...process.env, NO_COLOR: '1', TERM: 'dumb' },
      });

      const stdout = await new Response(proc.stdout).text();
      const exitCode = await proc.exited;

      if (exitCode !== 0) {
        return { ...base, error: 'Not logged in' };
      }

      // Parse account
      const accountMatch = stdout.match(/Signed in as (\S+)/);
      const account = accountMatch?.[1] || undefined;

      if (!account) {
        return { ...base, error: 'Not logged in' };
      }

      // Parse free tier: "Amp Free: $X/$Y remaining (replenishes +$Z/hour)"
      const freeMatch = stdout.match(/Amp Free:\s*\$([0-9.]+)\/\$([0-9.]+)\s*remaining/);
      let primary: QuotaWindow | undefined;

      if (freeMatch) {
        const remaining = parseFloat(freeMatch[1]);
        const total = parseFloat(freeMatch[2]);
        const pct = total > 0 ? Math.round((remaining / total) * 100) : 0;
        primary = {
          remaining: pct,
          resetsAt: null, // Free tier replenishes continuously
        };
      }

      // Parse replenish rate
      const replenishMatch = stdout.match(/replenishes \+\$([0-9.]+)\/hour/);
      const replenishRate = replenishMatch ? `+$${replenishMatch[1]}/hr` : null;

      // Parse bonus
      const bonusMatch = stdout.match(/\+(\d+)%\s*bonus\s*for\s*(\d+)\s*more\s*days/);
      const bonus = bonusMatch ? `+${bonusMatch[1]}% (${bonusMatch[2]}d)` : null;

      // Parse individual credits: "Individual credits: $X remaining"
      const creditsMatch = stdout.match(/Individual credits:\s*\$([0-9.]+)\s*remaining/);
      let extraUsage: ProviderQuota['extraUsage'] | undefined;

      if (creditsMatch) {
        const balance = parseFloat(creditsMatch[1]);
        if (balance > 0) {
          extraUsage = {
            enabled: true,
            remaining: 100, // We don't know the total, just show balance
            limit: 0,
            used: 0,
          };
        }
      }

      // Store raw data for tooltip display
      const models: Record<string, QuotaWindow> = {};
      
      if (freeMatch) {
        const remaining = parseFloat(freeMatch[1]);
        const total = parseFloat(freeMatch[2]);
        const pct = total > 0 ? Math.round((remaining / total) * 100) : 0;
        let label = `Free $${remaining}/$${total}`;
        if (replenishRate) label += ` (${replenishRate})`;
        if (bonus) label += ` ${bonus}`;
        models[label] = { remaining: pct, resetsAt: null };
      }

      if (creditsMatch) {
        const balance = parseFloat(creditsMatch[1]);
        models[`Credits $${balance}`] = { remaining: balance > 0 ? 100 : 0, resetsAt: null };
      }

      return {
        ...base,
        available: true,
        account,
        primary,
        extraUsage,
        models,
      };
    } catch (error) {
      logger.error('Amp usage parse error', { error });
      return { ...base, error: 'Failed to parse usage' };
    }
  }
}
```

### 5b. Register the provider

**File: `src/providers/index.ts`**

Add import and register:

```typescript
import { AmpProvider } from './amp';
// Add to exports
export { AmpProvider } from './amp';

// Add to providers array
export const providers: Provider[] = [
  new ClaudeProvider(),
  new CodexProvider(),
  new AntigravityProvider(),
  new AmpProvider(),
];
```

### 5c. Add Amp config paths

**File: `src/config.ts`**

Add to `CONFIG.paths`:

```typescript
amp: {
  bin: join(homedir(), '.cache', '.bun', 'bin', 'amp'),
  settings: join(XDG_CONFIG_HOME, 'amp', 'settings.json'),
  threads: join(homedir(), '.local', 'share', 'amp', 'threads'),
},
```

### 5d. Add Amp tooltip to formatters

**File: `src/formatters/waybar.ts`**

Add a `buildAmpTooltip` function following the same pattern as the others. Use `C.mauve` (purple) as the Amp accent color:

```typescript
function buildAmpTooltip(p: ProviderQuota): string {
  const lines: string[] = [];
  const v = s(C.mauve, B.v);
  
  lines.push(s(C.mauve, B.tl + B.h) + ' ' + s(C.mauve, 'Amp', true) + ' ' + s(C.mauve, B.h.repeat(53)));
  lines.push(v);
  
  if (p.error) {
    lines.push(v + '  ' + s(C.red, `⚠️ ${p.error}`));
  } else if (p.models && Object.keys(p.models).length > 0) {
    const maxLen = Math.max(...Object.keys(p.models).map(n => n.length), 20);
    
    lines.push(label('Usage', C.mauve));
    for (const [name, window] of Object.entries(p.models)) {
      const nameS = s(C.lavender, name.padEnd(maxLen));
      const b = bar(window.remaining);
      const pctS = s(getColorForPercent(window.remaining), pct(window.remaining).padStart(4));
      lines.push(v + '  ' + indicator(window.remaining) + ' ' + nameS + ' ' + b + ' ' + pctS);
    }
  }
  
  if (p.account) {
    lines.push(v);
    lines.push(v + '  ' + s(C.muted, `Account: ${p.account}`));
  }
  
  lines.push(v);
  lines.push(s(C.mauve, B.bl + B.h.repeat(55)));
  
  return lines.join('\n');
}
```

Add `case 'amp':` to `buildTooltip()`, `formatProviderForWaybar()`, and any switch statements that handle providers.

**File: `src/formatters/terminal.ts`**

Add similar `buildAmp()` function following the terminal ANSI pattern.

### 5e. Add Amp login support

**File: `src/tui/login-single.ts`**

Add a case for `'amp'`:

```typescript
case 'amp': {
  p.note(
    'Will open Amp login in browser.',
    colorize('Amp Login', semantic.title)
  );

  const ampBin = findAmpBin();
  if (!ampBin) {
    // Install amp globally
    const ok = await ensureBunGlobalPackage('@anthropic-ai/amp', 'amp');
    if (!ok) { await waitEnter(); return; }
  }

  await runInteractive(ampBin || 'amp', ['login']);
  await waitEnter();
  return;
}
```

You'll need to import or inline `findAmpBin()`. Simplest: copy the function or import from the provider.

### 5f. Add Amp waybar module snippet

**File: `snippets/waybar-modules.jsonc`**

Add an Amp module entry:

```json
"custom/qbar-amp": {
  "exec": "$HOME/.local/bin/qbar --provider amp",
  "return-type": "json",
  "interval": 120,
  "tooltip": true,
  "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
  "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right amp"
}
```

### 5g. Update setup.ts

**File: `src/setup.ts`**

Add `custom/qbar-amp` to the waybar module list that gets injected.

### Verify
- `bun x tsc --noEmit` passes
- `bun src/index.ts -t` shows all 4 providers (Claude, Codex, Antigravity, Amp)
- `bun src/index.ts --provider amp` returns JSON with Amp data
- `bun src/index.ts -t -p amp` shows Amp in terminal

---

## Final Checks

After all 5 tasks:

1. Run `bun x tsc --noEmit` — must pass with 0 errors
2. Run `bun src/index.ts -t` — should show all providers with data
3. Run `bun src/index.ts --provider amp -t` — should show Amp data
4. Make sure no hardcoded secrets or tokens are in the code
5. Commit each task separately:
   - `git add -A && git commit -m "fix: waitEnter() readline approach for terminal compatibility"`
   - `git add -A && git commit -m "fix: refresh.ts auto-close with Enter or timeout"`
   - `git add -A && git commit -m "feat: antigravity token auto-refresh"`
   - `git add -A && git commit -m "feat: consume new Claude/Codex API fields"`
   - `git add -A && git commit -m "feat: add Amp provider with free/credits monitoring"`

## Important Notes

- This is a **Bun** project (NOT Node.js) — use Bun APIs where available
- Use `Bun.file()`, `Bun.write()`, `Bun.spawn()`, `Bun.which()`, `Bun.sleep()` etc.
- The project uses `@clack/prompts` for TUI, `ora` for spinners
- Catppuccin Mocha is the color theme throughout
- All providers follow the same `Provider` interface pattern
- Cache is file-based in `~/.config/waybar/qbar/cache/`
