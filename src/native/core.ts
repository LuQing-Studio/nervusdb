import { createRequire } from 'node:module';

/**
 * Minimal loader for the upcoming Rust native bindings.
 *
 * The actual implementation will be provided by a N-API addon.
 * Until then we expose a graceful fallback that allows the rest of the
 * TypeScript runtime to detect whether the native layer is available.
 */

export interface NativeOpenOptions {
  dataPath: string;
}

export interface NativeDatabaseHandle {
  addFact(subject: string, predicate: string, object: string): number;
  close(): void;
}

export interface NativeCoreBinding {
  open(options: NativeOpenOptions): NativeDatabaseHandle;
}

let cachedBinding: NativeCoreBinding | null | undefined;

/**
 * Loads the native binding in a resilient way. If the addon is missing
 * (e.g. during local development or on unsupported platforms) we simply
 * return `null` and let the TypeScript implementation take over.
 */
export function loadNativeCore(): NativeCoreBinding | null {
  if (cachedBinding !== undefined) {
    return cachedBinding;
  }

  if (process.env.NERVUSDB_DISABLE_NATIVE === '1') {
    cachedBinding = null;
    return cachedBinding;
  }

  try {
    const requireNative = createRequire(import.meta.url);
    const binding = requireNative('../build/Release/nervusdb_core.node') as NativeCoreBinding;
    cachedBinding = binding;
  } catch {
    cachedBinding = null;
  }
  return cachedBinding;
}

/**
 * Allows tests to override the cached binding.
 */
export function __setNativeCoreForTesting(binding: NativeCoreBinding | null | undefined): void {
  cachedBinding = binding;
}
