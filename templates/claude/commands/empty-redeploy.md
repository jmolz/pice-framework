---
description: Trigger Vercel redeploy with an empty commit when GitHub/Vercel connection issues occur
---

# Empty Commit Redeploy

Trigger a Vercel redeploy without code changes when GitHub/Vercel connection is acting up.

```bash
git commit --allow-empty -m "chore: trigger redeploy"
git push origin main
```

That's it. This creates an empty commit that triggers Vercel's webhook.
