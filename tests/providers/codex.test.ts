import { describe, it, expect, beforeEach, afterEach, mock, spyOn } from "bun:test";

// ---------------------------------------------------------------------------
// Types (mirroring the private interfaces from codex.ts for test clarity)
// ---------------------------------------------------------------------------

interface CodexWindowRaw {
  used_percent: number;
  window_minutes: number;
  resets_at: number;
}

interface CodexLimitBucket {
  limit_id: string;
  limit_name?: string | null;
  primary?: CodexWindowRaw;
  secondary?: CodexWindowRaw;
}

interface CodexRateLimits {
  primary?: CodexWindowRaw;
  secondary?: CodexWindowRaw;
  credits?: {
    has_credits: boolean;
    unlimited: boolean;
    balance: string;
  };
  plan_type?: string | null;
  buckets?: Record<string, CodexLimitBucket>;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Builds a minimal Bun.file()-like object for auth file mocking. */
function fakeFile(opts: { exists: boolean }) {
  return {
    exists: () => Promise.resolve(opts.exists),
  };
}

/** Unix timestamp for a reset time ~1 hour from now. */
function futureUnix(hoursFromNow = 1): number {
  return Math.floor(Date.now() / 1000) + hoursFromNow * 3600;
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

let bunFileSpy: ReturnType<typeof spyOn>;

const cacheGetMock = mock<(key: string) => Promise<unknown>>();
const cacheSetMock = mock<(key: string, data: unknown, ttl: number) => Promise<void>>();

mock.module("../../src/cache", () => ({
  cache: {
    get: cacheGetMock,
    set: cacheSetMock,
  },
}));

mock.module("../../src/logger", () => ({
  logger: {
    debug: () => {},
    info: () => {},
    warn: () => {},
    error: () => {},
  },
}));

// ---------------------------------------------------------------------------
// Setup / Teardown
// ---------------------------------------------------------------------------

beforeEach(() => {
  bunFileSpy = spyOn(Bun, "file").mockReturnValue(fakeFile({ exists: false }) as any);
  cacheGetMock.mockResolvedValue(null);
  cacheSetMock.mockResolvedValue(undefined);
});

afterEach(() => {
  bunFileSpy.mockRestore();
  cacheGetMock.mockReset();
  cacheSetMock.mockReset();
});

// ---------------------------------------------------------------------------
// Helper to get a fresh CodexProvider per test
// ---------------------------------------------------------------------------

async function createProvider() {
  const { CodexProvider } = await import("../../src/providers/codex");
  return new CodexProvider();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CodexProvider", () => {
  // -----------------------------------------------------------------------
  // Identity
  // -----------------------------------------------------------------------
  describe("identity", () => {
    it("has id 'codex', name 'Codex' and cacheKey 'codex-quota'", async () => {
      const p = await createProvider();
      expect(p.id).toBe("codex");
      expect(p.name).toBe("Codex");
      expect(p.cacheKey).toBe("codex-quota");
    });
  });

  // -----------------------------------------------------------------------
  // isAvailable()
  // -----------------------------------------------------------------------
  describe("isAvailable()", () => {
    it("returns true when auth.json exists", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);
      const p = await createProvider();
      expect(await p.isAvailable()).toBe(true);
    });

    it("returns false when auth.json does not exist", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: false }) as any);
      const p = await createProvider();
      expect(await p.isAvailable()).toBe(false);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — cached primary/secondary simples
  // -----------------------------------------------------------------------
  describe("getQuota() with cached data: primary/secondary", () => {
    it("parses primary used_percent 40 -> remaining 60", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);
      const resetAt = futureUnix(2);

      const limits: CodexRateLimits = {
        primary: { used_percent: 40, window_minutes: 300, resets_at: resetAt },
        secondary: { used_percent: 20, window_minutes: 10080, resets_at: resetAt },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.primary?.remaining).toBe(60);
      expect(q.secondary?.remaining).toBe(80);
    });

    it("includes resetsAt as ISO string from unix timestamp", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);
      const resetAt = 1711540800; // fixed known timestamp

      const limits: CodexRateLimits = {
        primary: { used_percent: 0, window_minutes: 300, resets_at: resetAt },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.primary?.resetsAt).toBe(new Date(resetAt * 1000).toISOString());
    });

    it("returns null resetsAt when resets_at is 0", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 50, window_minutes: 300, resets_at: 0 },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.primary?.resetsAt).toBeNull();
    });

    it("handles 100% usage -> remaining 0", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 100, window_minutes: 300, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.primary?.remaining).toBe(0);
    });

    it("handles 0% usage -> remaining 100", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 0, window_minutes: 300, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.primary?.remaining).toBe(100);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — window classification
  // -----------------------------------------------------------------------
  describe("getQuota() window classification", () => {
    it("classifies window_minutes 300 as fiveHour", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.modelsDetailed).toBeDefined();
      const model = Object.values(q.modelsDetailed!)[0];
      expect(model.fiveHour).toBeDefined();
      expect(model.fiveHour!.remaining).toBe(90);
    });

    it("classifies window_minutes 10080 as sevenDay", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        secondary: { used_percent: 25, window_minutes: 10080, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.modelsDetailed).toBeDefined();
      const model = Object.values(q.modelsDetailed!)[0];
      expect(model.sevenDay).toBeDefined();
      expect(model.sevenDay!.remaining).toBe(75);
    });

    it("tolerates fiveHour within +/- 90 min (e.g. 210 and 390)", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      // 210 = 300 - 90 (boundary)
      const limits: CodexRateLimits = {
        buckets: {
          b1: {
            limit_id: "b1",
            primary: { used_percent: 10, window_minutes: 210, resets_at: futureUnix() },
          },
          b2: {
            limit_id: "b2",
            primary: { used_percent: 20, window_minutes: 390, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      const models = q.modelsDetailed!;
      for (const windows of Object.values(models)) {
        expect(windows.fiveHour).toBeDefined();
      }
    });

    it("tolerates sevenDay within +/- 1440 min (e.g. 8640 and 11520)", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        buckets: {
          b1: {
            limit_id: "b1",
            secondary: { used_percent: 30, window_minutes: 8640, resets_at: futureUnix() },
          },
          b2: {
            limit_id: "b2",
            secondary: { used_percent: 40, window_minutes: 11520, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      const models = q.modelsDetailed!;
      for (const windows of Object.values(models)) {
        expect(windows.sevenDay).toBeDefined();
      }
    });

    it("classifies unrecognized window durations as other", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      // 60 min = 1 hour, well outside tolerance of both fiveHour and sevenDay
      const limits: CodexRateLimits = {
        buckets: {
          b1: {
            limit_id: "b1",
            primary: { used_percent: 10, window_minutes: 60, resets_at: futureUnix() },
            secondary: { used_percent: 20, window_minutes: 60, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      // Both primary and secondary have unusual windows.
      // The fallback mapping assigns primary -> fiveHour and secondary -> sevenDay
      // because classifyWindow returns "other" for both, but fallback kicks in.
      const model = Object.values(q.modelsDetailed!)[0];
      expect(model.fiveHour).toBeDefined();
      expect(model.sevenDay).toBeDefined();
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — buckets
  // -----------------------------------------------------------------------
  describe("getQuota() with multiple buckets", () => {
    it("creates modelsDetailed entries for each bucket", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        buckets: {
          "codex-mini": {
            limit_id: "codex-mini",
            limit_name: "Codex Mini",
            primary: { used_percent: 30, window_minutes: 300, resets_at: futureUnix() },
            secondary: { used_percent: 15, window_minutes: 10080, resets_at: futureUnix() },
          },
          "codex-standard": {
            limit_id: "codex-standard",
            limit_name: "Codex Standard",
            primary: { used_percent: 60, window_minutes: 300, resets_at: futureUnix() },
            secondary: { used_percent: 45, window_minutes: 10080, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(q.modelsDetailed).toBeDefined();
      const names = Object.keys(q.modelsDetailed!);
      expect(names.length).toBe(2);
      expect(names).toContain("Codex Mini");
      expect(names).toContain("Codex Standard");

      expect(q.modelsDetailed!["Codex Mini"].fiveHour?.remaining).toBe(70);
      expect(q.modelsDetailed!["Codex Mini"].sevenDay?.remaining).toBe(85);
      expect(q.modelsDetailed!["Codex Standard"].fiveHour?.remaining).toBe(40);
      expect(q.modelsDetailed!["Codex Standard"].sevenDay?.remaining).toBe(55);
    });

    it("uses limit_id when limit_name is null", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        buckets: {
          "my_custom_limit": {
            limit_id: "my_custom_limit",
            limit_name: null,
            primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      // limit_id "my_custom_limit" -> "My Custom Limit" (underscore to space, title case)
      const names = Object.keys(q.modelsDetailed!);
      expect(names.length).toBe(1);
      expect(names[0]).toBe("My Custom Limit");
    });

    it("flattens modelsDetailed into models (picks fiveHour first)", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        buckets: {
          codex: {
            limit_id: "codex",
            limit_name: "Codex",
            primary: { used_percent: 25, window_minutes: 300, resets_at: futureUnix() },
            secondary: { used_percent: 50, window_minutes: 10080, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.models).toBeDefined();
      expect(q.models!["Codex"].remaining).toBe(75); // fiveHour preferred
    });

    it("deduplicates bucket names with suffix when labels collide", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        buckets: {
          a: {
            limit_id: "a",
            limit_name: "Codex",
            primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
          },
          b: {
            limit_id: "b",
            limit_name: "Codex",
            primary: { used_percent: 20, window_minutes: 300, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      const names = Object.keys(q.modelsDetailed!);
      expect(names.length).toBe(2);
      expect(names).toContain("Codex");
      expect(names).toContain("Codex (2)");
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — legacy fallback (no buckets, only primary/secondary)
  // -----------------------------------------------------------------------
  describe("getQuota() legacy fallback (no buckets)", () => {
    it("creates a single 'Codex' entry when only primary/secondary exist", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 35, window_minutes: 300, resets_at: futureUnix() },
        secondary: { used_percent: 55, window_minutes: 10080, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.modelsDetailed).toBeDefined();
      expect(Object.keys(q.modelsDetailed!)).toEqual(["Codex"]);
      expect(q.modelsDetailed!["Codex"].fiveHour?.remaining).toBe(65);
      expect(q.modelsDetailed!["Codex"].sevenDay?.remaining).toBe(45);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — plan type mapping
  // -----------------------------------------------------------------------
  describe("getQuota() plan type mapping", () => {
    const planCases: [string, string][] = [
      ["free", "Free"],
      ["pro", "Pro"],
      ["team", "Business"],
      ["business", "Business"],
      ["enterprise", "Enterprise"],
      ["edu", "Edu"],
      ["education", "Edu"],
      ["go", "Go"],
      ["plus", "Plus"],
      ["apikey", "API Key"],
      ["api_key", "API Key"],
    ];

    for (const [input, expected] of planCases) {
      it(`maps plan_type '${input}' -> plan '${expected}'`, async () => {
        bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

        const limits: CodexRateLimits = {
          primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
          plan_type: input,
        };
        cacheGetMock.mockResolvedValue(limits);

        const p = await createProvider();
        const q = await p.getQuota();

        expect(q.plan).toBe(expected);
        expect(q.planType).toBe(input);
      });
    }

    it("passes through unknown plan_type as-is", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
        plan_type: "custom_plan_xyz",
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.plan).toBe("custom_plan_xyz");
    });

    it("omits plan/planType when plan_type is null", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
        plan_type: null,
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.plan).toBeUndefined();
      expect(q.planType).toBeUndefined();
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — credits / extraUsage
  // -----------------------------------------------------------------------
  describe("getQuota() credits handling", () => {
    it("sets extraUsage when has_credits is true with balance", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 20, window_minutes: 300, resets_at: futureUnix() },
        credits: { has_credits: true, unlimited: false, balance: "10.50" },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.extraUsage).toBeDefined();
      expect(q.extraUsage!.enabled).toBe(true);
      expect(q.extraUsage!.remaining).toBe(11); // Math.min(100, Math.round(10.50))
      expect(q.extraUsage!.limit).toBe(0);
      expect(q.extraUsage!.used).toBe(0);
    });

    it("caps remaining at 100 for large balances", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 5, window_minutes: 300, resets_at: futureUnix() },
        credits: { has_credits: true, unlimited: false, balance: "999.99" },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.extraUsage!.remaining).toBe(100);
    });

    it("sets remaining 100 and limit -1 for unlimited credits", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 20, window_minutes: 300, resets_at: futureUnix() },
        credits: { has_credits: true, unlimited: true, balance: "0" },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.extraUsage).toBeDefined();
      expect(q.extraUsage!.remaining).toBe(100);
      expect(q.extraUsage!.limit).toBe(-1);
    });

    it("sets extraUsage when balance > 0 even if has_credits is false", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 20, window_minutes: 300, resets_at: futureUnix() },
        credits: { has_credits: false, unlimited: false, balance: "5.00" },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.extraUsage).toBeDefined();
      expect(q.extraUsage!.enabled).toBe(true);
      expect(q.extraUsage!.remaining).toBe(5);
    });

    it("omits extraUsage when no credits data", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 20, window_minutes: 300, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.extraUsage).toBeUndefined();
    });

    it("omits extraUsage when has_credits false and balance is '0'", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 20, window_minutes: 300, resets_at: futureUnix() },
        credits: { has_credits: false, unlimited: false, balance: "0" },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.extraUsage).toBeUndefined();
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — pickPrimary / pickSecondary
  // -----------------------------------------------------------------------
  describe("getQuota() primary/secondary selection", () => {
    it("uses explicit primary/secondary from limits when available", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 30, window_minutes: 300, resets_at: futureUnix() },
        secondary: { used_percent: 50, window_minutes: 10080, resets_at: futureUnix() },
        buckets: {
          codex: {
            limit_id: "codex",
            primary: { used_percent: 99, window_minutes: 300, resets_at: futureUnix() },
            secondary: { used_percent: 99, window_minutes: 10080, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      // pickPrimary/pickSecondary prefer explicit limits.primary/secondary
      expect(q.primary?.remaining).toBe(70);
      expect(q.secondary?.remaining).toBe(50);
    });

    it("falls back to bucket fiveHour/sevenDay when no explicit primary/secondary", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        buckets: {
          codex: {
            limit_id: "codex",
            primary: { used_percent: 40, window_minutes: 300, resets_at: futureUnix() },
            secondary: { used_percent: 60, window_minutes: 10080, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      // pickPrimary falls back to first model's fiveHour
      expect(q.primary?.remaining).toBe(60);
      // pickSecondary falls back to first model's sevenDay
      expect(q.secondary?.remaining).toBe(40);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — not logged in
  // -----------------------------------------------------------------------
  describe("getQuota() when not logged in", () => {
    it("returns error 'Not logged in' when auth file does not exist", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: false }) as any);
      cacheGetMock.mockResolvedValue(null);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe("Not logged in. Open `agent-bar-omarchy menu` and choose Provider login.");
      expect(q.provider).toBe("codex");
      expect(q.displayName).toBe("Codex");
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — no data available
  // -----------------------------------------------------------------------
  describe("getQuota() when no data available", () => {
    it("returns error when cache empty and app-server/session unavailable", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);
      cacheGetMock.mockResolvedValue(null);

      // To avoid actually spawning codex app-server, we mock the provider's
      // fetchRateLimitsViaAppServer and findLatestSessionFile methods.
      const p = await createProvider();

      // Mock private methods to simulate total failure
      (p as any).fetchRateLimitsViaAppServer = async () => null;
      (p as any).findLatestSessionFile = async () => null;

      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe("No session data found");
    });

    it("returns error when app-server returns null and session extraction fails", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);
      cacheGetMock.mockResolvedValue(null);

      const p = await createProvider();

      (p as any).fetchRateLimitsViaAppServer = async () => null;
      (p as any).findLatestSessionFile = async () => "/fake/session.jsonl";
      (p as any).extractRateLimits = async () => null;

      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe("No rate limit data found (app-server + session log)");
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — cache contract
  // -----------------------------------------------------------------------
  describe("getQuota() cache contract", () => {
    it("calls cache.get with 'codex-quota'", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      await p.getQuota();

      expect(cacheGetMock).toHaveBeenCalledWith("codex-quota");
    });

    it("does not call cache.set when data comes from cache", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      await p.getQuota();

      expect(cacheSetMock).not.toHaveBeenCalled();
    });

    it("calls cache.set after fetching fresh data", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);
      cacheGetMock.mockResolvedValue(null);

      const freshLimits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
      };

      const p = await createProvider();
      (p as any).fetchRateLimitsViaAppServer = async () => freshLimits;

      const q = await p.getQuota();

      expect(q.available).toBe(true);
      expect(cacheSetMock).toHaveBeenCalledTimes(1);
      expect(cacheSetMock.mock.calls[0][0]).toBe("codex-quota");
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — windowMinutes in output
  // -----------------------------------------------------------------------
  describe("getQuota() windowMinutes propagation", () => {
    it("includes windowMinutes in QuotaWindow output", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
        secondary: { used_percent: 20, window_minutes: 10080, resets_at: futureUnix() },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.primary?.windowMinutes).toBe(300);
      expect(q.secondary?.windowMinutes).toBe(10080);
    });
  });

  // -----------------------------------------------------------------------
  // getQuota() — edge: empty buckets
  // -----------------------------------------------------------------------
  describe("getQuota() edge cases", () => {
    it("skips buckets with no primary or secondary", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      const limits: CodexRateLimits = {
        primary: { used_percent: 10, window_minutes: 300, resets_at: futureUnix() },
        buckets: {
          empty: {
            limit_id: "empty",
            // no primary, no secondary
          },
          valid: {
            limit_id: "valid",
            limit_name: "Valid Bucket",
            primary: { used_percent: 20, window_minutes: 300, resets_at: futureUnix() },
          },
        },
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      const names = Object.keys(q.modelsDetailed!);
      // "empty" bucket should be skipped as it has no windows
      expect(names).toContain("Valid Bucket");
    });

    it("returns 'No quota windows found' when limits have no usable data", async () => {
      bunFileSpy.mockReturnValue(fakeFile({ exists: true }) as any);

      // Limits with neither primary/secondary nor buckets with data
      const limits: CodexRateLimits = {
        plan_type: "pro",
      };
      cacheGetMock.mockResolvedValue(limits);

      const p = await createProvider();
      const q = await p.getQuota();

      expect(q.available).toBe(false);
      expect(q.error).toBe("No quota windows found");
    });
  });
});
