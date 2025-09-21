import './JsonViewer.css'

const JsonViewer = ({ title, json, placeholder }) => {
  const copyToClipboard = () => {
    if (json) {
      navigator.clipboard.writeText(json)
    }
  }

  return (
    <div className="json-viewer">
      <div className="json-viewer-header">
        <h3>{title}</h3>
        {json && (
          <button onClick={copyToClipboard} className="copy-btn">
            Copy
          </button>
        )}
      </div>
      
      <div className="json-content">
        {json ? (
          <pre className="json-text">{json}</pre>
        ) : (
          <div className="json-placeholder">{placeholder}</div>
        )}
      </div>
    </div>
  )
}

export default JsonViewer