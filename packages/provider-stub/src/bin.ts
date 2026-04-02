#!/usr/bin/env node
import { StubProvider } from './index.js';

const provider = new StubProvider('0.1.0');
provider.start().catch((err) => {
  console.error('stub provider failed:', err);
  process.exit(1);
});
