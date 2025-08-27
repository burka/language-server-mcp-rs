#!/usr/bin/env python3
import json
import os
import subprocess
import threading
import time
import sys

class MCPClient:
    def __init__(self, server_path, workspace_path):
        self.process = subprocess.Popen(
            [server_path, workspace_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0,
            env={"RUST_LOG": "debug", **dict(os.environ)}
        )
        self.request_id = 0
        self.initialized = False
        
    def send_message(self, message):
        msg_str = json.dumps(message)
        print(f"Sending: {msg_str}")
        self.process.stdin.write(msg_str + '\n')
        self.process.stdin.flush()
        
    def read_response(self, timeout_seconds=None):
        def target():
            try:
                line = self.process.stdout.readline()
                return line.strip() if line else None
            except Exception as e:
                return f"Error: {e}"
                
        if timeout_seconds:
            thread = threading.Thread(target=target)
            thread.daemon = True
            thread.start()
            thread.join(timeout_seconds)
            
            if thread.is_alive():
                print(f"Response timed out after {timeout_seconds} seconds!")
                self.process.terminate()
                return None
            else:
                return target()
        else:
            return target()
    
    def initialize(self):
        # Wait a moment for server startup
        time.sleep(0.5)
        
        self.request_id += 1
        init_msg = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "initialize",
            "params": {
                "processId": None,
                "clientInfo": {"name": "test-client", "version": "1.0"},
                "capabilities": {},
                "workspaceFolders": None,
                "rootUri": None
            }
        }
        
        self.send_message(init_msg)
        response = self.read_response(10)
        
        if response:
            print(f"Initialize response: {response}")
            
            # Send initialized notification
            initialized_msg = {
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            }
            self.send_message(initialized_msg)
            self.initialized = True
            return True
        else:
            # Check stderr for errors
            stderr_output = ""
            while True:
                try:
                    self.process.stderr.settimeout(0.1)
                    line = self.process.stderr.readline()
                    if not line:
                        break
                    stderr_output += line
                except:
                    break
            if stderr_output:
                print(f"Server stderr: {stderr_output}")
            return False
    
    def call_tool(self, tool_name, arguments, timeout_seconds=10):
        if not self.initialized:
            print("Client not initialized!")
            return None
            
        self.request_id += 1
        tool_msg = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        }
        
        print(f"Calling tool: {tool_name} with timeout: {timeout_seconds}s")
        self.send_message(tool_msg)
        
        start_time = time.time()
        response = self.read_response(timeout_seconds)
        elapsed = time.time() - start_time
        
        if response:
            print(f"Got response in {elapsed:.2f}s: {response[:200]}...")
            return response
        else:
            print(f"Tool call timed out after {timeout_seconds}s (actual: {elapsed:.2f}s)")
            return None

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 test_timeout.py <timeout_seconds>")
        print("Example: python3 test_timeout.py 5")
        sys.exit(1)
        
    timeout_secs = int(sys.argv[1])
    
    server_path = "./target/release/language-server-mcp"
    workspace_path = "/home/flob/work/language-server-mcp-rs"
    
    print(f"Starting MCP client with {timeout_secs}s timeout...")
    client = MCPClient(server_path, workspace_path)
    
    # Initialize
    if not client.initialize():
        print("Failed to initialize!")
        return
        
    print("Client initialized successfully!")
    
    # Test goto_definition with our timeout
    test_file = "/home/flob/work/language-server-mcp-rs/src/test_trait.rs"
    
    print(f"\nTesting goto_definition on {test_file} with {timeout_secs}s timeout...")
    result = client.call_tool("goto_definition", {
        "file_path": test_file,
        "line": 58,
        "column": 15
    }, timeout_secs)
    
    if result:
        print("SUCCESS: Tool completed within timeout")
    else:
        print("TIMEOUT: Tool did not complete within timeout")
    
    client.process.terminate()

if __name__ == "__main__":
    main()