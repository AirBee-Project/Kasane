import { useState } from 'react'
import './App.css'
import LoginForm from './components/LoginForm'
import CommandPanel from './components/CommandPanel'

function App() {
  const [session, setSession] = useState(null)
  const [user, setUser] = useState(null)

  const handleLogin = (sessionData, userData) => {
    setSession(sessionData)
    setUser(userData)
  }

  const handleLogout = () => {
    setSession(null)
    setUser(null)
  }

  return (
    <div className="App">
      <header className="app-header">
        <h1>Kasane API Web Tool</h1>
        {user && (
          <div className="user-info">
            <span>Logged in as: {user.username}</span>
            <span className="server-url">Server: {user.serverUrl}</span>
            <button onClick={handleLogout} className="logout-btn">Logout</button>
          </div>
        )}
      </header>
      
      <main className="app-main">
        {!session ? (
          <LoginForm onLogin={handleLogin} />
        ) : (
          <CommandPanel session={session} serverUrl={user.serverUrl} />
        )}
      </main>
    </div>
  )
}

export default App
