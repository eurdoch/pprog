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
  const [isProcessing, setIsProcessing] = useState(false);
  const messagesEndRef = useRef<null | HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const handleEnterMessage = async (_e: any) => {
    if (inputMessage.trim() === '') return;

    const userMessage: Message = {
      role: "user",
      content: [
        { type: "text", "text": inputMessage.trim() }
      ]
    };
    setInputMessage('');

    setIsProcessing(true);
    handleSendMessage(userMessage);
    setIsProcessing(false);
  }

  const handleSendMessage = async (message: Message) => {
    setMessages(prevMessages => [...prevMessages, message]);

    try {
      const response = await fetch(`${window.SERVER_URL}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ message: message })
      });

      if (!response.ok) {
        throw new Error('Network response was not ok');
      }

      const data = await response.json();

      for (let contentItem of data.message.content) {
        switch(contentItem.type) {
          case "text":
            setMessages(prevMessages => [
              ...prevMessages,
              {
                role: data.message.role,
                content: [contentItem],
              },
            ]);
            break;
          // Received tool, immediately send back to handle too use on backend
          case "tool_use":
            await handleSendMessage({
              role: "assistant",
              content: [contentItem],
            });
            break;
          // Received tool result, immediately send back to send to model
          case "tool_result":
            await handleSendMessage({
              role: "user",
              content: [contentItem]
            });
            break;
          default:
            break;
        }
      }

    } catch (error: any) {
      console.error('Error:', error);
      setIsProcessing(false);
    }
  };

  const handleClearChat = async () => {
    try {
      const response = await fetch('http://localhost:8080/clear', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
      });
      console.log(response);

      if (!response.ok) {
        throw new Error('Network response was not ok');
      }

      // Clear local messages state
      setMessages([]);

    } catch (error) {
      console.error('Error clearing chat:', error);
    }
  };

  return (
    <div className="chat-container">
      <div className="chat-messages" style={{  }}>
        {messages.map((message, index) => {
            switch (message.content[0].type) {
              case "text":
                return <div
                  key={index}
                  className={`message ${message.role === "user" ? "user-msg" : "bot-msg"}`}
                >
                  {message.content[0].text}
                </div>
              case "tool_use":
                return <div
                  key={index}
                  className="message tool-msg"
                >
                  {"Using tool: " + message.content[0].name}
                </div>
              default:
                return null;
            }
        })}
        <div ref={messagesEndRef} />
      </div>
      <div className="chat-input">
        <input 
          type="text" 
          value={inputMessage}
          onChange={(e) => setInputMessage(e.target.value)}
          onKeyPress={(e) => e.key === 'Enter' && !isProcessing && handleEnterMessage(e)}
          placeholder="Type your message..."
        />
        <button 
          onClick={handleEnterMessage} 
          disabled={isProcessing}
          className={`send-button ${isProcessing ? 'processing' : ''}`}
        >
          Send
        </button>
        <button 
          onClick={handleClearChat}
          disabled={isProcessing}
        >
          Clear Chat
        </button>
      </div>
    </div>
  );
}

export default App;
