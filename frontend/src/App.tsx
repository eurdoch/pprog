import React, { useState, useRef, useEffect } from 'react'
import hljs from 'highlight.js'
import 'highlight.js/styles/github.css'
import './styles/base.css'
import './styles/chat-container.css'
import './styles/input.css'
import './styles/fab.css'
import './styles/modal.css'
import './styles/code-block.css'

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
  role: "user" | "assistant | tool",
  content: (Text | ToolUse | ToolResult)[],
}

interface FileChange {
  filename: string;
  changes: {
    type: 'added' | 'removed';
    content: string;
    lineNumber?: number;
  }[];
}

function renderTextWithCodeBlocks(text: string) {
  const codeBlockRegex = /```(\w+)?\n([\s\S]*?)```/g;
  const parts: (string | { language: string; code: string })[] = [];
  let lastIndex = 0;

  text.replace(codeBlockRegex, (match, language, code, index) => {
    // Add text before code block
    if (index > lastIndex) {
      parts.push(text.slice(lastIndex, index));
    }

    // Add code block
    parts.push({ 
      language: language || 'plaintext', 
      code: code.trim() 
    });

    lastIndex = index + match.length;
    return match;
  });

  // Add remaining text after last code block
  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }

  return parts.map((part, index) => {
    if (typeof part === 'string') {
      return <React.Fragment key={index}>{part}</React.Fragment>;
    } else {
      const highlightedCode = hljs.highlight(part.code, { 
        language: part.language 
      }).value;
      return (
        <pre key={index} className="code-block">
          <code 
            className={`language-${part.language}`} 
            dangerouslySetInnerHTML={{ __html: highlightedCode }} 
          />
        </pre>
      );
    }
  });
}

function parseDiff(diffContent: string): FileChange[] {
  const files: FileChange[] = [];
  let currentFile: FileChange | null = null;
  
  const lines = diffContent.split('\n');
  let lineNumber = 0;

  for (const line of lines) {
    if (line.startsWith('diff --git')) {
      if (currentFile) {
        files.push(currentFile);
      }
      const filename = line.split(' b/')[1];
      currentFile = {
        filename,
        changes: []
      };
    }
    else if (line.startsWith('+') && !line.startsWith('+++')) {
      currentFile?.changes.push({
        type: 'added',
        content: line.substring(1),
        lineNumber: ++lineNumber
      });
    }
    else if (line.startsWith('-') && !line.startsWith('---')) {
      currentFile?.changes.push({
        type: 'removed',
        content: line.substring(1),
        lineNumber: ++lineNumber
      });
    }
  }

  if (currentFile) {
    files.push(currentFile);
  }

  return files;
}

const App: React.FC = () => {
  const [messages, setMessages] = useState<Message[]>([]);
  const [inputMessage, setInputMessage] = useState('');
  const [isProcessing, setIsProcessing] = useState(false);
  const [showFab, setShowFab] = useState(false);
  const [showModal, setShowModal] = useState(false);
  const [diffFiles, setDiffFiles] = useState<FileChange[]>([]);
  const [recursiveCallCount, setRecursiveCallCount] = useState(0);
  const [initialLoadComplete, setInitialLoadComplete] = useState(false);
  const messagesEndRef = useRef<null | HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }

  // Auto-resize textarea
  const adjustTextareaHeight = () => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`; // Max height of 200px
    }
  };

  useEffect(() => {
    adjustTextareaHeight();
  }, [inputMessage]);

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  useEffect(() => {
    if (showFab) {
      const fetchAndParseDiff = async () => {
        try {
          const response = await fetch(`${window.SERVER_URL}/diff`);
          if (!response.ok) {
            throw new Error('Failed to fetch diff');
          }
          const data = await response.json();
          if (data.diff) {
            const parsedFiles = parseDiff(data.diff);
            setDiffFiles(parsedFiles);
          }
        } catch (error) {
          console.error('Error fetching diff:', error);
        }
      };

      fetchAndParseDiff();
    }
  }, [showFab]);

  useEffect(() => {
    const fetchMessages = async () => {
      try {
        const response = await fetch(`${window.SERVER_URL}/messages`);
        if (!response.ok) {
          throw new Error('Failed to fetch messages');
        }
        const data = await response.json();
        setMessages(data);
        setInitialLoadComplete(true);
      } catch (error) {
        console.error('Error fetching messages:', error);
        setInitialLoadComplete(true);
      }
    };

    if (textareaRef.current) {
      textareaRef.current.focus();
    }

    fetchMessages();
  }, []);

  useEffect(() => {
    console.log(messages);
  }, [messages]);

  const handleDiffCheck = () => {
    setShowModal(true);
  };

  const handleModalClose = () => {
    setShowModal(false);
  };

  const handleKeyPress = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (!isProcessing) {
        handleEnterMessage(e);
      }
    }
  };

  const handleEnterMessage = async (_e: any) => {
    try {
      if (inputMessage.trim() === '') return;
      setShowFab(false);
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
    } catch (error: any) {
      console.error(error);
      alert(error.error);
      setMessages(prev => prev.slice(0, -1));
      setIsProcessing(false);
    } finally {
      setIsProcessing(false);
    }
  }

  const handleSendMessage = async (message: Message) => {
    try {
      // Increment recursive call count
      setRecursiveCallCount(prev => prev + 1);

      const response = await fetch(`${window.SERVER_URL}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ message: message })
      });

      if (!response.ok) {
        const data = await response.json();
        setIsProcessing(false);
        throw new Error(data.error);
      }

      const data = await response.json();

      setMessages(prev => [
        ...prev,
        data.message
      ]);
      for (let contentItem of data.message.content) {
        switch(contentItem.type) {
          case "text":
            break;
          case "tool_use":
            const response = await fetch(`${window.SERVER_URL}/tools`, {
              method: 'POST',
              headers: {
                'Content-Type': 'application/json',
              },
              body: JSON.stringify({ ...contentItem })
            });

            if (!response.ok) {
              const data = await response.json();
              setIsProcessing(false);
              throw new Error(data.error);
            }

            const data = await response.json();
            
            await handleSendMessage({
              role: "user",
              content: [{
                type: "tool_result",
                tool_use_id: data.tool_use_id,
                content: data.content
              }],
            });
            break;
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
      console.error(error);
      alert(error);
      setIsProcessing(false);
    } finally {
      // Decrement recursive call count
      setRecursiveCallCount(prev => prev - 1);
    }
  };

  // Show FAB only when all recursive calls are complete, not processing, and initial load is complete
  useEffect(() => {
    if (initialLoadComplete && recursiveCallCount === 0 && !isProcessing && messages.length > 0) {
      setShowFab(true);
    } else {
      setShowFab(false);
    }
  }, [recursiveCallCount, isProcessing, initialLoadComplete, messages]);

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
        setShowFab(false);
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
      <div className="chat-messages">
        {messages.map((message, index) => {
            return message.content.map((contentItem, contentIndex) => {
              switch (contentItem.type) {
                case "text":
                  if (contentItem.text !== "") {
                    return <div
                      key={`${index}-${contentIndex}`}
                      className={`message ${message.role === "user" ? "user-msg" : "bot-msg"}`}
                    >
                      {renderTextWithCodeBlocks(contentItem.text)}
                    </div>
                  } else {
                    return null;
                  }
                case "tool_use":
                  return <div
                    key={`${index}-${contentIndex}`}
                    className="message tool-msg"
                  >
                    {"Using tool: " + contentItem.name}
                  </div>
                default:
                  return null;
              }
            })
        })}
        <div ref={messagesEndRef} />
      </div>
      {showFab && (
        <button 
          className="fab"
          onClick={handleDiffCheck}
          title="Check Diff"
        >
          🔍
        </button>
      )}
      {showModal && (
        <div className="modal-overlay" onClick={handleModalClose}>
          <div className="modal-content" onClick={e => e.stopPropagation()}>
            <button className="modal-close" onClick={handleModalClose}>×</button>
            <h2>Current diff</h2>
            <div className="diff-content">
              {diffFiles && diffFiles.length > 0 ? (
                diffFiles.map((file, fileIndex) => (
                  <div key={fileIndex} className="file-changes">
                    <h3 className="file-name">{file.filename}</h3>
                    <div className="changes-list">
                      {file.changes.map((change, changeIndex) => (
                        <div 
                          key={changeIndex} 
                          className={`change-line ${change.type}`}
                        >
                          <span className="line-number">{change.lineNumber}</span>
                          <span className="line-content">{change.content}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                ))
              ) : (
                <div>No changes found</div>
              )}
            </div>
          </div>
        </div>
      )}
      <div className="chat-input">
        <textarea 
          ref={textareaRef}
          value={inputMessage}
          onChange={(e) => setInputMessage(e.target.value)}
          onKeyDown={handleKeyPress}
          placeholder="Type your message..."
          rows={1}
        />
        <button 
          onClick={handleEnterMessage} 
          disabled={isProcessing || inputMessage.trim() === ''}
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
