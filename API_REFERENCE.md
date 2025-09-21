# Kasane API Reference

This document provides a comprehensive reference for all available INPUT commands and their corresponding OUTPUT structures in Kasane.

## Overview

Kasane accepts commands through JSON packets with the following structure:

```json
{
  "session": "string",
  "command": [Command, ...]
}
```

## Database Operations

### CreateSpace

Creates a new space in the database.

**INPUT:**
```json
{
  "createSpace": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### DropSpace

Deletes an existing space from the database.

**INPUT:**
```json
{
  "dropSpace": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### InfoSpace

Retrieves information about a specific space.

**INPUT:**
```json
{
  "infoSpace": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "infoSpace": {
    "spacename": "string",
    "keynames": [
      {
        "keyname": "string",
        "keytype": "INT|BOOLEAN|TEXT|FLOAT",
        "keymode": "UniqueKey|MultiKey"
      }
    ]
  }
}
```

### ShowSpaces

Lists all available spaces.

**INPUT:**
```json
"showSpaces"
```

**OUTPUT:**
```json
{
  "showSpaces": {
    "spacenames": ["string", ...]
  }
}
```

### Version

Returns the current version of Kasane.

**INPUT:**
```json
"version"
```

**OUTPUT:**
```json
{
  "version": {
    "version": "string"
  }
}
```

## Key Operations

### CreateKey

Creates a new key within a space.

**INPUT:**
```json
{
  "createKey": {
    "spaceName": "string",
    "keyName": "string",
    "keyType": "INT|BOOLEAN|TEXT|FLOAT",
    "keyMode": "UniqueKey|MultiKey"
  }
}
```

**OUTPUT:**
```json
"success"
```

### DropKey

Deletes a key from a space.

**INPUT:**
```json
{
  "dropKey": {
    "spaceName": "string",
    "keyName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### ShowKeys

Lists all keys in a specific space.

**INPUT:**
```json
{
  "showKeys": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "showkeys": {
    "keynames": ["string", ...]
  }
}
```

### InfoKey

Retrieves information about a specific key.

**INPUT:**
```json
{
  "infoKey": {
    "spaceName": "string",
    "keyName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "infoKey": {
    "keyname": "string",
    "keytype": "INT|BOOLEAN|TEXT|FLOAT",
    "keymode": "UniqueKey|MultiKey"
  }
}
```

## Value Operations

### InsertValue

Inserts a new value into a key within a specified range.

**INPUT:**
```json
{
  "insertValue": {
    "spaceName": "string",
    "keyName": "string",
    "range": Range,
    "value": ValueEntry
  }
}
```

**OUTPUT:**
```json
"success"
```

### PatchValue

Updates an existing value in a key within a specified range.

**INPUT:**
```json
{
  "patchValue": {
    "spaceName": "string",
    "keyName": "string",
    "range": Range,
    "value": ValueEntry
  }
}
```

**OUTPUT:**
```json
"success"
```

### DeleteValue

Deletes a value from a key within a specified range.

**INPUT:**
```json
{
  "deleteValue": {
    "spaceName": "string",
    "keyName": "string",
    "range": Range
  }
}
```

**OUTPUT:**
```json
"success"
```

### SelectValue

Retrieves values from specified keys within a range.

**INPUT:**
```json
{
  "selectValue": {
    "spaceName": "string",
    "keyNames": ["string", ...],
    "range": Range,
    "vertex": boolean,
    "center": boolean,
    "idString": boolean,
    "idPure": boolean
  }
}
```

**OUTPUT:**
```json
{
  "selectValue": [
    {
      "id": SpaceTimeId,
      "center": Point,
      "vertex": [Point, Point, Point, Point, Point, Point, Point, Point],
      "idString": "string",
      "value": [["string", ValueEntry], ...]
    }
  ]
}
```

### ShowValues

Displays all values for a specific key.

**INPUT:**
```json
{
  "showValues": {
    "spaceName": "string",
    "keyName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "showValues": [
    {
      "id": SpaceTimeId,
      "center": Point,
      "vertex": [Point, Point, Point, Point, Point, Point, Point, Point],
      "idString": "string",
      "value": [["string", ValueEntry], ...]
    }
  ]
}
```

## User Operations

### CreateUser

Creates a new user account.

**INPUT:**
```json
{
  "createUser": {
    "userName": "string",
    "password": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### DropUser

Deletes a user account.

**INPUT:**
```json
{
  "dropUser": {
    "userName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### InfoUser

Retrieves information about a specific user.

**INPUT:**
```json
{
  "infoUser": {
    "userName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "infoUser": {
    "userName": "string"
  }
}
```

### ShowUsers

Lists all user accounts.

**INPUT:**
```json
"showUsers"
```

**OUTPUT:**
```json
{
  "showUsers": {
    "users": ["string", ...]
  }
}
```

## Data Types

### Range

Defines a spatial-temporal range for operations:

```json
{
  "function": {
    "spot": {
      "point1": Point,
      "zoom": number
    }
  }
}
```

or

```json
{
  "function": {
    "line": {
      "point1": Point,
      "point2": Point,
      "zoom": number
    }
  }
}
```

or

```json
{
  "function": {
    "triangle": {
      "point1": Point,
      "point2": Point,
      "point3": Point,
      "zoom": number
    }
  }
}
```

or

```json
{
  "prefix": {
    "and": [Range, ...]
  }
}
```

or

```json
{
  "prefix": {
    "or": [Range, ...]
  }
}
```

or

```json
{
  "idSet": [
    {
      "z": number,
      "f": DimensionRange,
      "x": DimensionRange,
      "y": DimensionRange,
      "i": number,
      "t": DimensionRange
    }
  ]
}
```

### Point

Represents a geographical point:

```json
{
  "lat": number,
  "lng": number
}
```

### ValueEntry

Represents a value with its type:

- `{ "int": number }`
- `{ "boolean": boolean }`
- `{ "text": "string" }`
- `{ "float": number }`

### SpaceTimeId

A complex identifier for spatio-temporal data (implementation-specific).

### DimensionRange

Represents a range in a dimension:

- `{ "single": number }`
- `{ "range": [number, number] }`

## Authentication

All commands except login require a valid session token. Sessions are obtained through the `/login` endpoint:

**LOGIN REQUEST:**
```json
{
  "username": "string",
  "password": "string"
}
```

**LOGIN RESPONSE:**
```json
{
  "sessionId": "string",
  "expiresInSecs": number
}
```

## Error Responses

When an error occurs, the system returns an error string instead of the expected output structure.

## Notes

- Commands are processed as arrays, allowing multiple operations in a single request
- Session tokens expire after 1 hour (3600 seconds) of inactivity
- All spatial coordinates use WGS84 (latitude/longitude)
- The system supports 4-dimensional spatio-temporal data following IPA's spatial ID guidelines