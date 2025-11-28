/**
 * Kasane Wasm TypeScript Wrapper
 *
 * This module provides a TypeScript interface for interacting with the
 * Kasane spatio-temporal database engine built with WebAssembly.
 */

import type {
  Command,
  Configuration,
  Output,
  UserError,
  Storage,
  CreateKey,
  DropKey,
  ShowKeys,
  InsertValue,
  DeleteValue,
  SelectValue,
  ShowValues,
  KeyType,
  Range,
  ValueEntry,
} from "./types";

/**
 * Result type for Kasane operations
 * Either contains the successful output or an error
 */
export type KasaneResult<T = Output> =
  | { ok: true; value: T }
  | { ok: false; error: UserError };

/**
 * Interface for the Wasm module exports
 * These functions are expected to be exported from the compiled Wasm
 */
export interface KasaneWasmExports {
  /**
   * Initialize the Kasane storage
   * @param conf - Configuration object
   * @param importData - Optional array of Storage data to import
   */
  init(conf: Configuration, importData?: Storage[]): void;

  /**
   * Execute a command against the Kasane storage
   * @param command - The command to execute
   * @returns Result containing Output or UserError
   */
  kasane(command: Command): Output | UserError;

  /**
   * Export the current storage state
   * @returns The current storage data
   */
  export(): Storage;
}

/**
 * Kasane client class for interacting with the Wasm module
 */
export class Kasane {
  private wasm: KasaneWasmExports;
  private initialized: boolean = false;

  /**
   * Create a new Kasane client
   * @param wasmModule - The loaded Wasm module exports
   */
  constructor(wasmModule: KasaneWasmExports) {
    this.wasm = wasmModule;
  }

  /**
   * Initialize the Kasane storage
   * @param config - Configuration options (empty object for Wasm mode)
   * @param importData - Optional storage data to import from a previous session
   */
  init(config: Configuration = {}, importData?: Storage[]): void {
    this.wasm.init(config, importData);
    this.initialized = true;
  }

  /**
   * Check if the storage has been initialized
   */
  isInitialized(): boolean {
    return this.initialized;
  }

  /**
   * Execute a raw command
   * @param command - The command to execute
   * @returns Result containing the output or error
   */
  execute(command: Command): KasaneResult {
    if (!this.initialized) {
      return {
        ok: false,
        error: {
          parseError: {
            message: "Kasane not initialized. Call init() first.",
            location: "TypeScript wrapper",
          },
        },
      };
    }

    try {
      const result = this.wasm.kasane(command);
      // Check if result is an error (has specific error properties)
      if (this.isError(result)) {
        return { ok: false, error: result as UserError };
      }
      return { ok: true, value: result as Output };
    } catch (e) {
      return {
        ok: false,
        error: {
          parseError: {
            message: e instanceof Error ? e.message : String(e),
            location: "TypeScript wrapper",
          },
        },
      };
    }
  }

  /**
   * Export the current storage state for persistence
   * @returns The current storage data
   */
  export(): Storage {
    if (!this.initialized) {
      throw new Error("Kasane not initialized. Call init() first.");
    }
    return this.wasm.export();
  }

  // ---------------------- Key Operations ----------------------

  /**
   * Create a new key in the storage
   * @param keyName - Name of the key to create
   * @param keyType - Type of values the key will store
   */
  createKey(keyName: string, keyType: KeyType): KasaneResult {
    const params: CreateKey = { keyName, keyType };
    return this.execute({ createKey: params });
  }

  /**
   * Drop a key from the storage
   * @param keyName - Name of the key to drop
   */
  dropKey(keyName: string): KasaneResult {
    const params: DropKey = { keyName };
    return this.execute({ dropKey: params });
  }

  /**
   * Show all keys in the storage
   */
  showKeys(): KasaneResult {
    const params: ShowKeys = {};
    return this.execute({ showKeys: params });
  }

  // ---------------------- Value Operations ----------------------

  /**
   * Insert a value into the storage
   * @param keyName - Name of the key to insert into
   * @param range - Range specification for the value
   * @param value - The value to insert
   */
  insertValue(keyName: string, range: Range, value: ValueEntry): KasaneResult {
    const params: InsertValue = { keyName, range, value };
    return this.execute({ insertValue: params });
  }

  /**
   * Delete values from the storage
   * @param keyName - Name of the key to delete from
   * @param range - Range specification for deletion
   */
  deleteValue(keyName: string, range: Range): KasaneResult {
    const params: DeleteValue = { keyName, range };
    return this.execute({ deleteValue: params });
  }

  /**
   * Select values from the storage
   * @param keyNames - Names of keys to select from
   * @param range - Range specification for selection
   */
  selectValue(keyNames: string[], range: Range): KasaneResult {
    const params: SelectValue = { keyNames, range };
    return this.execute({ selectValue: params });
  }

  /**
   * Show all values for a key
   * @param keyName - Name of the key to show values for
   */
  showValues(keyName: string): KasaneResult {
    const params: ShowValues = { keyName };
    return this.execute({ showValues: params });
  }

  // ---------------------- Helper Methods ----------------------

  /**
   * Check if a result is an error
   */
  private isError(result: unknown): boolean {
    if (typeof result !== "object" || result === null) {
      return false;
    }
    const r = result as Record<string, unknown>;
    return (
      "keyNameValidationError" in r ||
      "parseError" in r ||
      "keyNotFound" in r ||
      "keyAlreadyExists" in r
    );
  }
}

/**
 * Create and initialize a new Kasane instance
 * @param wasmModule - The loaded Wasm module exports
 * @param config - Optional configuration
 * @param importData - Optional storage data to import
 * @returns Initialized Kasane instance
 */
export function createKasane(
  wasmModule: KasaneWasmExports,
  config: Configuration = {},
  importData?: Storage[]
): Kasane {
  const kasane = new Kasane(wasmModule);
  kasane.init(config, importData);
  return kasane;
}
