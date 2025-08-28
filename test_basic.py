#!/usr/bin/env python3
import json
import subprocess
import sys
import threading
import time

def test_basic_mcp():
    print("Starting basic MCP test...")
    
    # Start the MCP server
    process = subprocess.Popen(
        ["./target/release/language-server-mcp", "."],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )
    
    try:
        # Send initialize request
        init_request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": None,
                "clientInfo": {"name": "test-client", "version": "1.0"},
                "capabilities": {},
                "workspaceFolders": None,
                "rootUri": None
            }
        }
        
        print("Sending initialize request...")
        process.stdin.write(json.dumps(init_request) + '\n')
        process.stdin.flush()
        
        # Read response
        response_line = process.stdout.readline()
        if response_line:
            print(f"Got response: {response_line.strip()}")
            response = json.loads(response_line)
            
            if response.get("id") == 1 and "result" in response:
                print("Initialize successful!")
                
                # Send initialized notification
                initialized = {
                    "jsonrpc": "2.0",
                    "method": "initialized",
                    "params": {}
                }
                print("Sending initialized notification...")
                process.stdin.write(json.dumps(initialized) + '\n')
                process.stdin.flush()
                
                print("MCP server is ready!")
                return True
        
        print("Failed to get valid response")
        return False
        
    finally:
        process.terminate()
        process.wait()

if __name__ == "__main__":
    success = test_basic_mcp()
    sys.exit(0 if success else 1)