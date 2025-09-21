# Kasane

Kasane was developed to safely and efficiently manage drone flight paths.

Drone route planning requires comprehensive interpretation and utilization of diverse, time-varying information such as:

- Weather conditions
- Radio wave status
- Population density
- Buildings and terrain
- Other aircraft position information
- Government and local authority regulations and notifications

To centrally manage this dynamic information, we developed the database engine **"Kasane"** based on the [4-dimensional spatio-temporal information utilization spatial ID guidelines](https://www.ipa.go.jp/digital/architecture/guidelines/4dspatio-temporal-guideline.html) proposed by IPA, which enables **multilayered integration and management of spatio-temporal data**.

## Space-Time ID Preview

You can preview space-time IDs using [https://voxel.airbee.xyz/](https://voxel.airbee.xyz/). This tool allows you to visualize and interact with the 4-dimensional spatial ID structure that Kasane utilizes.

[🇯🇵 日本語版](./README_JA.md)

[Documentation](https://kasane.dev)

# Kasane Features

- **Spatio-temporal Support**: Supports 4-dimensional data structures including the time axis. Enables management and analysis of continuously changing information.

- **Information Layering**: Different types and dimensions of information can be layered and spatiotemporal regions that meet specified conditions can be flexibly extracted.

- **Transaction Features**: Transaction-based data operations are implemented to ensure data consistency and safety.

- **Type-safe Design**: By incorporating type information into data, input errors and lack of consistency are prevented, achieving more reliable data management.

- **Wasm Support**: Using WebAssembly (Wasm), operation is possible in diverse execution environments including browsers and edge environments.

# Open Source

Kasane is designed for use in any field requiring spatio-temporal information integration, including not only drone route management but also urban planning, disaster response, and logistics management.
This software is published as open source under the MIT license and can be freely used for both commercial and non-commercial purposes.

# Development Environment

This project uses Rust with a workspace configuration. To set up the development environment:

```bash
# Clone the repository
git clone https://github.com/Tomoro0726/Kasane.git
cd Kasane

# Build the project
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench
```

## API Documentation

- [📖 API Reference](./API_REFERENCE.md) - Comprehensive command documentation
- [📋 Command Summary](./COMMAND_SUMMARY.md) - Quick reference table

## [logic](/logic/README.md)

Library defining logical operations for space-time IDs

## [core](/core/README.md)

Implementation of the Kasane database
