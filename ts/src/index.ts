/**
 * Kasane Wasm TypeScript Wrapper
 *
 * A TypeScript wrapper for the Kasane spatio-temporal database engine.
 *
 * @example
 * ```typescript
 * import { Kasane, createKasane } from '@kasane/wasm';
 * import wasmModule from './kasane_bg.wasm';
 *
 * // Initialize Kasane
 * const kasane = createKasane(wasmModule);
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
export { Kasane, createKasane, type KasaneResult, type KasaneWasmExports } from "./kasane";
