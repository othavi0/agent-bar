#!/usr/bin/env bun

import { runUninstall } from "./uninstall";

export async function main() {
  await runUninstall({ force: true, title: "qbar remove" });
}

if (import.meta.main) {
  main().catch((error) => {
    console.error("Remove failed:", error);
    process.exit(1);
  });
}
