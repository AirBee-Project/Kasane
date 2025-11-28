/**
 * TypeScript type definitions for Kasane Wasm interface
 * These types mirror the Rust types from the kasane crate
 */

// ---------------------- Key Types ----------------------

/**
 * Represents the type of a key in Kasane
 */
export type KeyType = "text" | "float" | "int" | "boolean";

/**
 * Parameters for creating a new key
 */
export interface CreateKey {
  keyName: string;
  keyType: KeyType;
}

/**
 * Parameters for dropping a key
 */
export interface DropKey {
  keyName: string;
}

/**
 * Parameters for showing keys
 */
export interface ShowKeys {}

// ---------------------- Value Types ----------------------

/**
 * Represents a value entry that can be stored
 */
export type ValueEntry =
  | { TEXT: string }
  | { FLOAT: number }
  | { INT: number }
  | { BOOLEAN: boolean };

/**
 * Parameters for inserting a value
 */
export interface InsertValue {
  keyName: string;
  range: Range;
  value: ValueEntry;
}

/**
 * Parameters for deleting a value
 */
export interface DeleteValue {
  keyName: string;
  range: Range;
}

/**
 * Parameters for selecting values
 */
export interface SelectValue {
  keyNames: string[];
  range: Range;
}

/**
 * Parameters for showing values
 */
export interface ShowValues {
  keyName: string;
}

// ---------------------- Range & Function Types ----------------------

/**
 * Represents a coordinate in 3D space
 */
export interface Coordinate {
  lat: number;
  lon: number;
  alt: number;
  time: number;
}

/**
 * Represents a space-time ID input
 */
export interface SpaceTimeIDInput {
  z: number;
  f: [number | null, number | null];
  x: [number | null, number | null];
  y: [number | null, number | null];
}

/**
 * Point function definition
 */
export interface Point {
  z: number;
  point1: Coordinate;
}

/**
 * Line function definition
 */
export interface Line {
  z: number;
  point1: Coordinate;
  point2: Coordinate;
}

/**
 * Triangle function definition
 */
export interface Triangle {
  z: number;
  point1: Coordinate;
  point2: Coordinate;
  point3: Coordinate;
}

/**
 * Filter for value with a specific key
 */
export interface FilterValue {
  keyName: string;
  filter: Filter;
}

/**
 * Boolean filter operations
 */
export type FilterBoolean =
  | "hasValue"
  | "isTrue"
  | "isFalse"
  | { equals: boolean }
  | { notEquals: boolean };

/**
 * Float filter operations
 */
export type FilterFloat =
  | "hasValue"
  | { equal: number }
  | { notEqual: number }
  | { greaterThan: number }
  | { greaterEqual: number }
  | { lessThan: number }
  | { lessEqual: number }
  | { between: [number, number] }
  | { in: number[] }
  | { notIn: number[] };

/**
 * Integer filter operations
 */
export type FilterInt =
  | "hasValue"
  | { equal: number }
  | { notEqual: number }
  | { greaterThan: number }
  | { greaterEqual: number }
  | { lessThan: number }
  | { lessEqual: number }
  | { between: [number, number] }
  | { in: number[] }
  | { notIn: number[] };

/**
 * Text filter operations
 */
export type FilterText =
  | "hasValue"
  | { equal: string }
  | { notEqual: string }
  | { contains: string }
  | { notContains: string }
  | { startsWith: string }
  | { endsWith: string }
  | { caseInsensitiveEqual: string };

/**
 * Filter type union
 */
export type Filter =
  | { filterBoolean: FilterBoolean }
  | { filterInt: FilterInt }
  | { filterFloat: FilterFloat }
  | { filterText: FilterText };

/**
 * Function type for range specification
 */
export type Function = { point: Point } | { line: Line } | { triangle: Triangle };

/**
 * Calculation type for range specification
 */
export type Calculation =
  | { AND: Range[] }
  | { OR: Range[] }
  | { DIFF: { base: Range; remove: Range } };

/**
 * Range specification for queries
 */
export type Range =
  | { function: Function }
  | { calculation: Calculation }
  | { ids: SpaceTimeIDInput[] }
  | { filterValue: FilterValue };

// ---------------------- Command Types ----------------------

/**
 * Command type union - all available commands
 */
export type Command =
  | { createKey: CreateKey }
  | { dropKey: DropKey }
  | { showKeys: ShowKeys }
  | { insertValue: InsertValue }
  | { deleteValue: DeleteValue }
  | { selectValue: SelectValue }
  | { showValues: ShowValues };

// ---------------------- Output Types ----------------------

/**
 * Key information returned by showKeys
 */
export interface Key {
  keyName: string;
  keyType: KeyType;
}

/**
 * Output for showKeys command
 */
export interface ShowkeysOutput {
  keyNames: Key[];
}

/**
 * A serializable representation of SpaceTimeID for output
 */
export interface SpaceTimeIDOutput {
  z: number;
  f: [number, number];
  x: [number, number];
  y: [number, number];
}

/**
 * A value with its associated SpaceTimeID
 */
export interface Value {
  id: SpaceTimeIDOutput;
  value: ValueEntry;
}

/**
 * Values for a single key in SelectValue response
 */
export interface KeyValues {
  keyName: string;
  values: Value[];
}

/**
 * Output for selectValue command - returns values for multiple keys within a range
 */
export interface SelectValueOutput {
  keyValues: KeyValues[];
}

/**
 * Output for showValues command - returns all values for a single key
 */
export interface ShowValuesOutput {
  values: Value[];
}

/**
 * Output type union - all possible command outputs
 */
export type Output =
  | "success"
  | { showkeys: ShowkeysOutput }
  | { selectValue: SelectValueOutput }
  | { showValues: ShowValuesOutput };

// ---------------------- Configuration Types ----------------------

/**
 * Configuration for Wasm mode (minimal configuration)
 */
export interface Configuration {}

// ---------------------- Error Types ----------------------

/**
 * User error types that can be returned from operations
 */
export type UserError =
  | {
      keyNameValidationError: {
        name: string;
        reason: string;
        location: string;
      };
    }
  | {
      parseError: {
        message: string;
        location: string;
      };
    }
  | {
      keyNotFound: {
        keyName: string;
        location: string;
      };
    }
  | {
      keyAlreadyExists: {
        keyName: string;
        location: string;
      };
    };

// ---------------------- Storage Types ----------------------

/**
 * Storage data structure for import/export
 * This is used for persisting and restoring Kasane state
 */
export interface Storage {
  /**
   * Internal storage data - opaque to TypeScript users
   * The actual structure is managed by the Wasm module
   */
  inner: unknown;
}
