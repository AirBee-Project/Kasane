/**
 * Kasane Wasm TypeScript Wrapper
 *
 * A TypeScript wrapper for the Kasane spatio-temporal database engine.
 *
 * @example
 * ```typescript
 * // Using wasm-pack generated module (--target web)
 * import init, * as kasaneWasm from './pkg/kasane.js';
 * import { initKasaneFromWasmPack } from '@kasane/wasm';
 *
 * const kasane = await initKasaneFromWasmPack(kasaneWasm, '/pkg/kasane_bg.wasm');
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
  initKasaneFromWasmPack,
  type KasaneResult,
  type KasaneWasmExports,
  type WasmPackModule,
  type LoadKasaneOptions,
} from "./kasane";
