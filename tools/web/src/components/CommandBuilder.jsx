import { useState } from 'react'
import './CommandBuilder.css'

const COMMAND_TYPES = {
  // Custom JSON Input
  'customJson': { label: 'Custom JSON', fields: [{ name: 'json', type: 'textarea', required: true, placeholder: 'Enter custom JSON command...' }] },
  
  // Database Operations
  'createSpace': { label: 'Create Space', fields: [{ name: 'space_name', type: 'text', required: true }] },
  'dropSpace': { label: 'Drop Space', fields: [{ name: 'space_name', type: 'text', required: true }] },
  'infoSpace': { label: 'Info Space', fields: [{ name: 'space_name', type: 'text', required: true }] },
  'showSpaces': { label: 'Show Spaces', fields: [] },
  'version': { label: 'Version', fields: [] },
  
  // Key Operations
  'createKey': { 
    label: 'Create Key', 
    fields: [
      { name: 'space_name', type: 'text', required: true },
      { name: 'key_name', type: 'text', required: true },
      { name: 'key_type', type: 'select', required: true, options: ['INT', 'BOOLEAN', 'TEXT', 'FLOAT'] },
      { name: 'key_mode', type: 'select', required: true, options: ['UniqueKey', 'MultiKey'] }
    ]
  },
  'dropKey': { 
    label: 'Drop Key', 
    fields: [
      { name: 'space_name', type: 'text', required: true },
      { name: 'key_name', type: 'text', required: true }
    ]
  },
  'showKeys': { label: 'Show Keys', fields: [{ name: 'space_name', type: 'text', required: true }] },
  'infoKey': { 
    label: 'Info Key', 
    fields: [
      { name: 'space_name', type: 'text', required: true },
      { name: 'key_name', type: 'text', required: true }
    ]
  },
  
  // User Operations
  'createUser': { 
    label: 'Create User', 
    fields: [
      { name: 'user_name', type: 'text', required: true },
      { name: 'password', type: 'password', required: true }
    ]
  },
  'dropUser': { label: 'Drop User', fields: [{ name: 'user_name', type: 'text', required: true }] },
  'infoUser': { label: 'Info User', fields: [{ name: 'user_name', type: 'text', required: true }] },
  'showUsers': { label: 'Show Users', fields: [] },
}

const CommandBuilder = ({ onAddCommand }) => {
  const [selectedCommand, setSelectedCommand] = useState('')
  const [formData, setFormData] = useState({})

  const handleCommandChange = (commandType) => {
    setSelectedCommand(commandType)
    setFormData({})
  }

  const handleFieldChange = (fieldName, value) => {
    setFormData({
      ...formData,
      [fieldName]: value
    })
  }

  const handleSubmit = (e) => {
    e.preventDefault()
    
    if (!selectedCommand) return

    const commandConfig = COMMAND_TYPES[selectedCommand]
    
    // Validate required fields
    for (const field of commandConfig.fields) {
      if (field.required && !formData[field.name]) {
        alert(`${field.name} is required`)
        return
      }
    }

    let command
    if (selectedCommand === 'customJson') {
      // Handle custom JSON input
      try {
        command = JSON.parse(formData.json)
      } catch (error) {
        alert('Invalid JSON format: ' + error.message)
        return
      }
    } else if (commandConfig.fields.length === 0) {
      // Simple commands like "version", "showSpaces", "showUsers"
      command = selectedCommand
    } else {
      // Commands with parameters
      command = {
        [selectedCommand]: formData
      }
    }

    onAddCommand(command)
    setFormData({})
  }

  const renderField = (field) => {
    const value = formData[field.name] || ''
    
    if (field.type === 'select') {
      return (
        <select
          value={value}
          onChange={(e) => handleFieldChange(field.name, e.target.value)}
          required={field.required}
        >
          <option value="">Select {field.name}</option>
          {field.options.map(option => (
            <option key={option} value={option}>{option}</option>
          ))}
        </select>
      )
    }
    
    if (field.type === 'textarea') {
      return (
        <textarea
          value={value}
          onChange={(e) => handleFieldChange(field.name, e.target.value)}
          placeholder={field.placeholder || `Enter ${field.name}`}
          required={field.required}
          rows={10}
        />
      )
    }
    
    return (
      <input
        type={field.type}
        value={value}
        onChange={(e) => handleFieldChange(field.name, e.target.value)}
        placeholder={field.placeholder || `Enter ${field.name}`}
        required={field.required}
      />
    )
  }

  return (
    <div className="command-builder">
      <div className="command-selector">
        <label htmlFor="command-type">Command Type:</label>
        <select
          id="command-type"
          value={selectedCommand}
          onChange={(e) => handleCommandChange(e.target.value)}
        >
          <option value="">Select a command</option>
          <optgroup label="Custom Input">
            <option value="customJson">Custom JSON</option>
          </optgroup>
          <optgroup label="Database Operations">
            <option value="createSpace">Create Space</option>
            <option value="dropSpace">Drop Space</option>
            <option value="infoSpace">Info Space</option>
            <option value="showSpaces">Show Spaces</option>
            <option value="version">Version</option>
          </optgroup>
          <optgroup label="Key Operations">
            <option value="createKey">Create Key</option>
            <option value="dropKey">Drop Key</option>
            <option value="showKeys">Show Keys</option>
            <option value="infoKey">Info Key</option>
          </optgroup>
          <optgroup label="User Operations">
            <option value="createUser">Create User</option>
            <option value="dropUser">Drop User</option>
            <option value="infoUser">Info User</option>
            <option value="showUsers">Show Users</option>
          </optgroup>
        </select>
      </div>

      {selectedCommand && (
        <form onSubmit={handleSubmit} className="command-form">
          <h3>{COMMAND_TYPES[selectedCommand].label}</h3>
          
          {COMMAND_TYPES[selectedCommand].fields.map(field => (
            <div key={field.name} className="form-group">
              <label htmlFor={field.name}>
                {field.name}:
                {field.required && <span className="required">*</span>}
              </label>
              {renderField(field)}
            </div>
          ))}
          
          <button type="submit" className="add-command-btn">
            Add Command
          </button>
        </form>
      )}
    </div>
  )
}

export default CommandBuilder