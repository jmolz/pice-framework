#!/usr/bin/env node
import { ClaudeCodeProvider } from './index.js';

const provider = new ClaudeCodeProvider();
provider.start().catch((err) => {
  console.error('Claude Code provider failed:', err);
  process.exit(1);
});
