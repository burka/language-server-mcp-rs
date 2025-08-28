#!/usr/bin/env python3
import json
import subprocess
import sys
import time

def test_mcp_goto_definition():
    print("Testing MCP server goto_definition with timeout...")
    
    # Use the rmcp crate approach like the example
    try:
        process = subprocess.Popen(
            ["cargo", "run", "--manifest-path", "examples/Cargo.toml", "--bin", "client"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd="/home/flob/work/language-server-mcp-rs"
        )
        
        # Set a 10 second timeout
        try:
            stdout, stderr = process.communicate(timeout=10)
            print("MCP Client completed within 10 seconds!")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return True
            
        except subprocess.TimeoutExpired:
            print("TIMEOUT: MCP client took longer than 10 seconds - this indicates hanging!")
            process.kill()
            stdout, stderr = process.communicate()
            print("Partial STDOUT:", stdout)
            print("Partial STDERR:", stderr)
            return False
            
    except Exception as e:
        print(f"Error starting MCP client: {e}")
        return False

if __name__ == "__main__":
    success = test_mcp_goto_definition()
    print(f"\nTest result: {'PASS - No hanging detected' if success else 'FAIL - Hanging detected'}")
    sys.exit(0 if success else 1)