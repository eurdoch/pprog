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

  // New effect to fetch messages on component mount
  useEffect(() => {
    const fetchMessages = async () => {
      try {
        const response = await fetch(`${window.SERVER_URL}/messages`);
        if (!response.ok) {
          throw new Error('Failed to fetch messages');
        }
        const data = await response.json();
        setMessages(data);
      } catch (error) {
        console.error('Error fetching messages:', error);
      }
    };

    fetchMessages();
  }, []); // Empty dependency array means this runs once on mount

  const handleEnterMessage = async (_e: any) => {
    try {
      if (inputMessage.trim() === '') return;
      setIsProcessing(true);

      const userMessage: Message = {
        role: "user",
        content: [
          { type: "text", "text": inputMessage.trim() }
        ]
      };
      setMessages(prevMessages => [...prevMessages, userMessage]);
      setInputMessage('');

      await handleSendMessage(userMessage);
    } catch (error) {
      console.error(error);
      setIsProcessing(false);
    } finally {
      setIsProcessing(false);
    }
  }


  const handleSendMessage = async (message: Message) => {
    try {
      const response = await fetch(`${window.SERVER_URL}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ message: message })
      });

      if (!response.ok) {
        const data = await response.json();
        alert(`${data.error.message}`);
        throw new Error("");
      }

      const data = await response.json();

      for (let contentItem of data.message.content) {
        setMessages(prevMessages => [
          ...prevMessages,
          {
            role: data.message.role,
            content: [contentItem],
          },
        ]);
        switch(contentItem.type) {
          case "text":
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
      // If the error wasn't already handled by the response.ok check
      if (!error.message.startsWith('Error:')) {
        alert(`Error: ${error.message}`);
      }
      setIsProcessing(false);
    }
  };

  const handleClearChat = async (_e: any) => {
    try {
      const response = await fetch(`${window.SERVER_URL}/clear`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        throw new Error('Network response was not ok');
      }

      let data = await response.json();
      if (data.cleared) {
        setMessages([]);
        alert("Chat cleared successfully.");
      } else {
        alert("Something went wrong, chat not cleared.");
      }
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
