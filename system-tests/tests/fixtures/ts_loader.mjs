// system-tests/tests/fixtures/ts_loader.mjs
// ============================================================================
// Module: TypeScript Loader Shim
// Description: Resolve .js specifiers to .ts when the .js file is missing.
// Purpose: Allow Node --experimental-strip-types to run TS sources with .js imports.
// ============================================================================

import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

function shouldRewrite(specifier) {
  if (!specifier.endsWith(".js")) {
    return false;
  }
  return specifier.startsWith(".") || specifier.startsWith("/") || specifier.startsWith("file:");
}

function toTsUrl(specifier, parentURL) {
  if (specifier.startsWith("file:")) {
    return specifier.replace(/\.js$/, ".ts");
  }
  const base = parentURL ?? pathToFileURL(`${process.cwd()}/`).href;
  const resolved = new URL(specifier, base);
  return resolved.href.replace(/\.js$/, ".ts");
}

export async function resolve(specifier, context, nextResolve) {
  try {
    return await nextResolve(specifier, context);
  } catch (error) {
    if (shouldRewrite(specifier)) {
      const tsUrl = toTsUrl(specifier, context.parentURL);
      try {
        if (existsSync(fileURLToPath(tsUrl))) {
          return await nextResolve(tsUrl, context);
        }
      } catch (_) {
        // Fall through to original error.
      }
    }
    throw error;
  }
}
