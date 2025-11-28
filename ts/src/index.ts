/**
 * Kasane Wasm TypeScript Wrapper
 *
 * A TypeScript wrapper for the Kasane spatio-temporal database engine.
 *
 * @example
 * ```typescript
 * // Load from Vite /public folder
 * import { loadKasane } from '@kasane/wasm';
 *
 * const kasane = await loadKasane('/kasane.wasm');
 *
 * // Create a key
 * const result = kasane.createKey('temperature', 'float');
 * if (result.ok) {
 *   console.log('Key created successfully');
 * }
 *
 * // Export data for persistence
 * const data = kasane.export();
 * localStorage.setItem('kasane-data', JSON.stringify(data));
 * ```
 *
 * @packageDocumentation
 */

// Re-export all types
export * from "./types";

// Re-export main classes and functions
export {
  Kasane,
  createKasane,
  loadKasane,
  loadKasaneModule,
  type KasaneResult,
  type KasaneWasmExports,
  type LoadKasaneOptions,
} from "./kasane";
