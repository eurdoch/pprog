import { useState, useRef, useEffect } from 'react'
import './App.css'

interface Text {
  type: "text",
  text: string,
}

interface ToolUse {
  type: "tool_use",
  id: string,
  name: string,
  input: object,
}

interface ToolResult {
  type: "tool_result",
  tool_use_id: string,
  content: string,
}

interface Message {
  role: "user" | "assistant",
  content: (Text | ToolUse | ToolResult)[],
}

function App() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [inputMessage, setInputMessage] = useState('');
  const messagesEndRef = useRef<null | HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const handleSendMessage = async () => {
    if (inputMessage.trim() === '') return;

    // Add user message
    const userMessage: Message = {
      role: "user",
      content: [
        { type: "text", "text": inputMessage.trim() }
      ]
    };

    setMessages(prevMessages => [...prevMessages, userMessage]);
    setInputMessage('');

    try {
      // Send message to backend
      const response = await fetch('http://localhost:8080/chat', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ message: userMessage })
      });

      if (!response.ok) {
        throw new Error('Network response was not ok');
      }

      const data = await response.json();

      console.log(data);
      for (let contentItem of data.message.content) {
        switch(contentItem.type) {
            case "text":
              const newMessage: Message = {
                role: data.message.role,
                content: [contentItem],
              };
              setMessages(prevMessages => [
                ...prevMessages,
                newMessage,
              ]);
              break;
            case "tool_use":
              break;
            case "tool_result":
              break;
            default:
              break;
        }
      }

    } catch (error) {
      console.error('Error:', error);
    }
  };

  return (
    <div className="chat-container">
      <div className="chat-messages" style={{  }}>
        {messages.map((message, index) => (
          <div 
            key={index} 
            className={`message ${message.role === 'user' ? 'user-message' : 'bot-message'}`}
          >
            {message.content[0].text}
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>
      <div className="chat-input">
        <input 
          type="text" 
          value={inputMessage}
          onChange={(e) => setInputMessage(e.target.value)}
          onKeyPress={(e) => e.key === 'Enter' && handleSendMessage()}
          placeholder="Type your message..."
        />
        <button onClick={handleSendMessage}>Send</button>
      </div>
    </div>
  );
}

export default App;

