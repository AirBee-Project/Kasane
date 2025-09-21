import { useState } from 'react'
import './LoginForm.css'

const LoginForm = ({ onLogin }) => {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [serverUrl, setServerUrl] = useState('http://127.0.0.1:8080')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const handleSubmit = async (e) => {
    e.preventDefault()
    setLoading(true)
    setError('')

    try {
      let response;
      
      // Try direct connection first, fall back to proxy if CORS fails
      try {
        const loginUrl = serverUrl.endsWith('/') ? `${serverUrl}login` : `${serverUrl}/login`
        response = await fetch(loginUrl, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            username,
            password,
          }),
        })
      } catch (corsError) {
        // If direct connection fails (likely CORS), try proxy
        if (serverUrl === 'http://127.0.0.1:8080' || serverUrl === 'http://localhost:8080') {
          response = await fetch('/api/login', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
            },
            body: JSON.stringify({
              username,
              password,
            }),
          })
        } else {
          throw corsError
        }
      }

      if (response.ok) {
        const data = await response.json()
        onLogin(data, { username, serverUrl })
      } else {
        const errorText = await response.text()
        setError(errorText || 'Login failed')
      }
    } catch (err) {
      setError('Network error: ' + err.message)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="login-container">
      <form className="login-form" onSubmit={handleSubmit}>
        <h2>Login to Kasane API</h2>
        
        {error && <div className="error-message">{error}</div>}
        
        <div className="form-group">
          <label htmlFor="serverUrl">Server URL:</label>
          <input
            type="url"
            id="serverUrl"
            value={serverUrl}
            onChange={(e) => setServerUrl(e.target.value)}
            placeholder="http://127.0.0.1:8080"
            required
            disabled={loading}
          />
        </div>
        
        <div className="form-group">
          <label htmlFor="username">Username:</label>
          <input
            type="text"
            id="username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder="Enter username"
            required
            disabled={loading}
            autoComplete="username"
          />
        </div>
        
        <div className="form-group">
          <label htmlFor="password">Password:</label>
          <input
            type="password"
            id="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Enter password"
            required
            disabled={loading}
            autoComplete="current-password"
          />
        </div>
        
        <button type="submit" disabled={loading} className="login-button">
          {loading ? 'Logging in...' : 'Login'}
        </button>
        
        <div className="login-hint">
          <p>Default credentials: admin / nekocute</p>
        </div>
      </form>
    </div>
  )
}

export default LoginForm