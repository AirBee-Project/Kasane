import { useState } from 'react'
import './CommandPanel.css'
import CommandBuilder from './CommandBuilder'
import JsonViewer from './JsonViewer'

const CommandPanel = ({ session, serverUrl }) => {
  const [commands, setCommands] = useState([])
  const [requestJson, setRequestJson] = useState('')
  const [responseJson, setResponseJson] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const addCommand = (command) => {
    setCommands([...commands, command])
  }

  const removeCommand = (index) => {
    setCommands(commands.filter((_, i) => i !== index))
  }

  const executeCommands = async () => {
    if (commands.length === 0) {
      setError('Please add at least one command')
      return
    }

    setLoading(true)
    setError('')

    const payload = {
      session: session.session_id,
      command: commands
    }

    const requestJsonStr = JSON.stringify(payload, null, 2)
    setRequestJson(requestJsonStr)

    try {
      let response;
      
      // Try direct connection first, fall back to proxy if CORS fails
      try {
        const executeUrl = serverUrl.endsWith('/') ? `${serverUrl}execute` : `${serverUrl}/execute`
        response = await fetch(executeUrl, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: requestJsonStr,
        })
      } catch (corsError) {
        // If direct connection fails (likely CORS), try proxy
        if (serverUrl === 'http://127.0.0.1:8080' || serverUrl === 'http://localhost:8080') {
          response = await fetch('/api/execute', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
            },
            body: requestJsonStr,
          })
        } else {
          throw corsError
        }
      }

      const responseText = await response.text()
      
      if (response.ok) {
        try {
          const data = JSON.parse(responseText)
          setResponseJson(JSON.stringify(data, null, 2))
        } catch {
          setResponseJson(responseText)
        }
      } else {
        setError(`HTTP ${response.status}: ${responseText}`)
        setResponseJson(responseText)
      }
    } catch (err) {
      setError('Network error: ' + err.message)
      setResponseJson('')
    } finally {
      setLoading(false)
    }
  }

  const clearAll = () => {
    setCommands([])
    setRequestJson('')
    setResponseJson('')
    setError('')
  }

  return (
    <div className="command-panel">
      <div className="panel-section">
        <h2>Command Builder</h2>
        <CommandBuilder onAddCommand={addCommand} />
        
        <div className="commands-list">
          <h3>Commands Queue ({commands.length})</h3>
          {commands.length === 0 ? (
            <p className="no-commands">No commands added yet</p>
          ) : (
            <div className="commands">
              {commands.map((command, index) => (
                <div key={index} className="command-item">
                  <pre>{JSON.stringify(command, null, 2)}</pre>
                  <button 
                    onClick={() => removeCommand(index)}
                    className="remove-command-btn"
                  >
                    Remove
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="action-buttons">
          <button 
            onClick={executeCommands} 
            disabled={loading || commands.length === 0}
            className="execute-btn"
          >
            {loading ? 'Executing...' : 'Execute Commands'}
          </button>
          <button 
            onClick={clearAll}
            disabled={loading}
            className="clear-btn"
          >
            Clear All
          </button>
        </div>

        {error && <div className="error-message">{error}</div>}
      </div>

      <div className="json-section">
        <div className="json-viewer-container">
          <JsonViewer 
            title="Request JSON"
            json={requestJson}
            placeholder="Request JSON will appear here when you execute commands"
          />
        </div>
        
        <div className="json-viewer-container">
          <JsonViewer 
            title="Response JSON"
            json={responseJson}
            placeholder="Response JSON will appear here after execution"
          />
        </div>
      </div>
    </div>
  )
}

export default CommandPanel