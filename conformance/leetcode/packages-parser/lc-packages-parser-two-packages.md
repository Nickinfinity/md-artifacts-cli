---
artifactType: LeetCode
leetcodeType: stack
title: Case
difficulty: easy
status: unsolved
tags: [leetcode]
---

```yaml leetcode
packages:
  - name: api
    dir: server
    install: ["pip", "install", "-r", "requirements.txt"]
    start: ["uvicorn", "main:app", "--port", "${PORT}"]
    ready: "Uvicorn running"
  - name: web
    dir: web
    install: ["npm", "ci"]
    start: ["npm", "run", "dev", "--", "--port", "${PORT}"]
    dependsOn: [api]
    exposeAs:
      VITE_API_URL: "http://127.0.0.1:${PORT}/v1"
```
