# AetherIDE — Universal AI Model Gateway Guide

## 1. Supported Providers & Models

| Provider | Supported Models | Key Capabilities | Best For |
| :--- | :--- | :--- | :--- |
| **Anthropic** | Claude 3.5 Sonnet, Claude 3.5 Haiku, Claude 3 Opus | 200K Context, Tool Calling, Vision, Artifacts | Code refactoring, deep architecture, system design |
| **OpenAI** | GPT-4o, o1, o3-mini | Vision, Reasoning, Structured Outputs | General coding, math, logical deduction |
| **Google Gemini** | Gemini 2.0 Flash, Gemini 1.5 Pro | 1M - 2M Context, Multimodal Audio/Video | Large codebase analysis, ultra-fast streaming |
| **xAI** | Grok-2 | High-speed code completion, search | Real-time web citations, coding |
| **DeepSeek** | DeepSeek V3, DeepSeek R1 | Reasoning, High cost-efficiency | Complex algorithmic challenges, offline coding |
| **Ollama (Local)** | Llama 3, DeepSeek Coder, Mistral, Qwen | 100% Offline, Privacy, Zero Token Cost | Offline flights, air-gapped security environments |

---

## 2. Intelligent Model Router
Aether automatically directs prompts to the most suitable model based on task classification:
- **Fast completions / inline edits:** Claude 3.5 Haiku / Gemini 2.0 Flash
- **Complex architecture & multithreaded debugging:** Claude 3.5 Sonnet / DeepSeek R1
- **UI / Screenshot inspection:** GPT-4o / Gemini 1.5 Pro
- **Air-gapped / Local-only mode:** Local Ollama models

---

## 3. Model Arena
Compare multiple model outputs side by side on identical unit test fixtures or design specifications:
- Latency (Time-to-First-Token, Total Duration)
- Token Consumption (Prompt tokens, Completion tokens, Estimated cost)
- Verified pass rate on test suites
