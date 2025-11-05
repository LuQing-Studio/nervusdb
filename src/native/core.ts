import { createRequire } from 'node:module';
import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

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

export interface NativeAddFactOutput {
  subject_id: number;
  predicate_id: number;
  object_id: number;
}

export interface NativeQueryCriteria {
  subject_id?: number;
  predicate_id?: number;
  object_id?: number;
}

export type NativeTriple = NativeAddFactOutput;

export interface NativeDatabaseHandle {
  addFact(subject: string, predicate: string, object: string): NativeTriple;
  query(criteria?: NativeQueryCriteria): NativeTriple[];
  openCursor(criteria?: NativeQueryCriteria): { id: number };
  readCursor(cursorId: number, batchSize: number): { triples: NativeTriple[]; done: boolean };
  closeCursor(cursorId: number): void;
  hydrate(dictionary: string[], triples: NativeTriple[]): void;
  close(): void;
}

export interface NativeCoreBinding {
  open(options: NativeOpenOptions): NativeDatabaseHandle;
}

let cachedBinding: NativeCoreBinding | null | undefined;

function resolveNativeAddonPath(): string | null {
  const baseDir = join(process.cwd(), 'native', 'nervusdb-node');
  const direct = join(baseDir, 'index.node');
  if (existsSync(direct)) return direct;

  const npmDir = join(baseDir, 'npm');
  const candidates: string[] = [];

  const platform = process.platform;
  const arch = process.arch;

  if (platform === 'win32') {
    if (arch === 'x64') candidates.push('win32-x64-msvc');
    if (arch === 'arm64') candidates.push('win32-arm64-msvc');
  } else if (platform === 'darwin') {
    if (arch === 'arm64') candidates.push('darwin-arm64');
    if (arch === 'x64') candidates.push('darwin-x64');
  } else if (platform === 'linux') {
    if (arch === 'x64') {
      candidates.push('linux-x64-gnu', 'linux-x64-musl');
    }
    if (arch === 'arm64') {
      candidates.push('linux-arm64-gnu', 'linux-arm64-musl');
    }
    if (arch === 'arm') {
      candidates.push('linux-arm-gnueabihf');
    }
  }

  if (existsSync(npmDir)) {
    for (const triplet of candidates) {
      const candidatePath = join(npmDir, triplet, 'index.node');
      if (existsSync(candidatePath)) {
        return candidatePath;
      }
    }

    // Fallback: scan directories (legacy behaviour / dev environments)
    for (const entry of readdirSync(npmDir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const candidate = join(npmDir, entry.name, 'index.node');
      if (existsSync(candidate)) {
        return candidate;
      }
    }
  }

  if (process.env.NERVUSDB_EXPECT_NATIVE === '1') {
    const available = existsSync(npmDir)
      ? readdirSync(npmDir, { withFileTypes: true })
          .filter((d) => d.isDirectory())
          .map((d) => d.name)
      : [];
    const tripletList = candidates.length ? candidates.join(', ') : '[]';
    const availableList = available.length ? available.join(', ') : 'none';
    console.error(
      `[Native Loader] Failed to resolve addon. Platform=${platform}, arch=${arch}, searched triplets=${tripletList}. Available directories: ${availableList}.`,
    );
  }

  return null;
}

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
    const addonPath = resolveNativeAddonPath();
    if (addonPath) {
      const binding = requireNative(addonPath) as NativeCoreBinding;
      cachedBinding = binding;
    } else {
      if (process.env.NERVUSDB_EXPECT_NATIVE === '1') {
        throw new Error(`Native addon expected but not found in ${addonPath ?? 'resolved paths'}`);
      }
      cachedBinding = null;
    }
  } catch (error) {
    if (process.env.NERVUSDB_EXPECT_NATIVE === '1') {
      throw error instanceof Error ? error : new Error(String(error));
    }
    cachedBinding = null;
  }
  return cachedBinding;
}

/**
 * Allows tests to override the cached binding.
 */
export function __setNativeCoreForTesting(binding: NativeCoreBinding | null | undefined): void {
  cachedBinding = binding ?? null;
}
