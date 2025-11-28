# @kasane/wasm

TypeScript wrapper for Kasane Wasm - a spatio-temporal database engine.

## Installation

```bash
npm install @kasane/wasm
```

## Usage

### Loading from Vite /public folder (Recommended)

```typescript
import { loadKasane } from '@kasane/wasm';

// Load Wasm from /public/kasane.wasm
const kasane = await loadKasane('/kasane.wasm');

// Create a key for storing temperature data
const result = kasane.createKey('temperature', 'float');
if (result.ok) {
  console.log('Key created successfully');
}

// Insert a value with space-time coordinates
const insertResult = kasane.insertValue(
  'temperature',
  {
    ids: [{
      z: 10,
      f: [0, null],
      x: [100, null],
      y: [200, null]
    }]
  },
  { FLOAT: 25.5 }
);

// Select values
const selectResult = kasane.selectValue(
  ['temperature'],
  {
    ids: [{
      z: 10,
      f: [0, 100],
      x: [50, 150],
      y: [150, 250]
    }]
  }
);

// Export data for persistence
const data = kasane.export();
localStorage.setItem('kasane-data', JSON.stringify(data));
```

### Loading with Import Data

```typescript
import { loadKasane, Storage } from '@kasane/wasm';

// Load previously exported data
const savedData = localStorage.getItem('kasane-data');
const importData: Storage[] = savedData ? [JSON.parse(savedData)] : undefined;

// Load and initialize with imported data
const kasane = await loadKasane('/kasane.wasm', {
  importData
});
```

### Manual Loading (Without Auto-Initialization)

```typescript
import { loadKasaneModule, Storage } from '@kasane/wasm';

// Load without auto-initialization
const kasane = await loadKasaneModule('/kasane.wasm');

// Load saved data
const savedData = localStorage.getItem('kasane-data');
const importData: Storage[] = savedData ? [JSON.parse(savedData)] : undefined;

// Initialize manually
kasane.init({}, importData);
```

### Using Pre-loaded Wasm Module

```typescript
import { Kasane, createKasane } from '@kasane/wasm';
import * as wasmModule from './kasane_bg.wasm';

// Initialize Kasane with a pre-loaded Wasm module
const kasane = createKasane(wasmModule);
```

## API

### `loadKasane(wasmUrl, options?)`

Load Kasane from a Wasm file URL and create an initialized instance.

- `wasmUrl`: URL to the Wasm file (e.g., '/kasane.wasm' for Vite public folder)
- `options.config`: Optional configuration object
- `options.importData`: Optional array of Storage data to import
- `options.importObject`: Optional WebAssembly import object

Returns: `Promise<Kasane>`

### `loadKasaneModule(wasmUrl, importObject?)`

Load Kasane from a Wasm file URL without auto-initialization.

- `wasmUrl`: URL to the Wasm file
- `importObject`: Optional WebAssembly import object

Returns: `Promise<Kasane>` (not initialized)

### `createKasane(wasmModule, config?, importData?)`

Create and initialize a new Kasane instance from a pre-loaded module.

- `wasmModule`: The loaded Wasm module exports
- `config`: Optional configuration object (empty for Wasm mode)
- `importData`: Optional array of Storage data to import

### `Kasane` Class

#### Methods

##### Key Operations

- `createKey(keyName: string, keyType: KeyType)` - Create a new key
- `dropKey(keyName: string)` - Drop a key
- `showKeys()` - List all keys

##### Value Operations

- `insertValue(keyName: string, range: Range, value: ValueEntry)` - Insert a value
- `deleteValue(keyName: string, range: Range)` - Delete values
- `selectValue(keyNames: string[], range: Range)` - Select values
- `showValues(keyName: string)` - Show all values for a key

##### Utility

- `init(config?, importData?)` - Initialize the storage
- `isInitialized()` - Check if storage is initialized
- `execute(command: Command)` - Execute a raw command
- `export()` - Export storage data

## Types

### KeyType

```typescript
type KeyType = "text" | "float" | "int" | "boolean";
```

### ValueEntry

```typescript
type ValueEntry =
  | { TEXT: string }
  | { FLOAT: number }
  | { INT: number }
  | { BOOLEAN: boolean };
```

### Range

Range can be specified in multiple ways:

```typescript
// By space-time IDs
{ ids: SpaceTimeIDInput[] }

// By geometric function
{ function: { point: Point } | { line: Line } | { triangle: Triangle } }

// By calculation
{ calculation: { AND: Range[] } | { OR: Range[] } | { DIFF: { base: Range, remove: Range } } }

// By filter
{ filterValue: { keyName: string, filter: Filter } }
```

### KasaneResult

All operations return a result type:

```typescript
type KasaneResult<T = Output> =
  | { ok: true; value: T }
  | { ok: false; error: UserError };
```

## Building from Source

```bash
cd ts
npm install
npm run build
```

## License

MIT

## Links

- [Kasane Documentation](https://kasane.dev)
- [GitHub Repository](https://github.com/AirBee-Project/Kasane)
- [Space-Time ID Preview](https://voxel.airbee.xyz/)
