# AetherIDE — Model Context Protocol (MCP) Integration Guide

## 1. Overview
AetherIDE provides native client integration for the Model Context Protocol (MCP), enabling AI engineering agents to interact with external tools, contextual resources, and local machine development servers.

---

## 2. Configuration (`.aether/mcp_servers.json`)
MCP servers can be configured globally or per-workspace:
```json
{
  "mcpServers": {
    "git-tools": {
      "command": "git-mcp-server",
      "args": ["--workspace", "."],
      "env": {}
    },
    "database-explorer": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/aether_dev"]
    },
    "filesystem-advanced": {
      "command": "aether-cli",
      "args": ["mcp", "fs"]
    }
  }
}
```

---

## 3. Dynamic Tool Calling & Permissions
- When an agent requests to invoke an MCP tool, Aether's `permission-engine` intercepts the tool invocation.
- Read tools (`read_resource`, `list_resources`) run automatically under Tier P0.
- State-mutating tools require authorization based on the current autonomy mode (Observe, Assist, Autopilot).
