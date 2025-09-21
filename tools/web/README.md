# Kasane API Web Tool

A React-based web interface for interacting with the Kasane API server.

## Features

- **Login Authentication**: Secure login with username/password
- **Command Builder**: GUI forms for building JSON API commands
- **Real-time Execution**: Send commands to the Kasane server and view responses
- **JSON Viewer**: Display both request and response JSON with syntax highlighting
- **Multiple Commands**: Queue and execute multiple commands in sequence

## Setup and Development

### Prerequisites

- Node.js 20+ and npm
- Kasane server running on localhost:8080

### Installation

```bash
cd tools/web
npm install
```

### Development

```bash
# Start the development server
npm run dev

# The web tool will be available at http://localhost:5173
```

### Production Build

```bash
npm run build
npm run preview
```

## Usage

1. **Login**: Use the default credentials `admin` / `nekocute` or any valid Kasane user account
2. **Build Commands**: Select command types from the dropdown and fill in required parameters
3. **Queue Commands**: Add multiple commands to be executed together
4. **Execute**: Send the commands to the server and view the results
5. **View JSON**: Both request and response JSON are displayed for inspection

## Supported Commands

### Database Operations
- **Create Space**: Create a new space in the database
- **Drop Space**: Delete an existing space
- **Info Space**: Get information about a space
- **Show Spaces**: List all available spaces
- **Version**: Get the Kasane server version

### Key Operations
- **Create Key**: Create a new key in a space
- **Drop Key**: Delete a key from a space
- **Show Keys**: List all keys in a space
- **Info Key**: Get information about a specific key

### User Operations
- **Create User**: Create a new user account
- **Drop User**: Delete a user account
- **Info User**: Get information about a user
- **Show Users**: List all users

## API Proxy

The development server includes a proxy configuration that forwards API requests from `/api/*` to `http://127.0.0.1:8080/*`, avoiding CORS issues during development.

## Technical Details

- **Framework**: React 19.1 with Vite 7.1
- **Styling**: CSS modules with custom styling
- **State Management**: React hooks (useState)
- **API Communication**: Fetch API with JSON
- **Development Server**: Vite with HMR support
