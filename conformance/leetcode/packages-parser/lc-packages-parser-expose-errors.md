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
    dir: a
    install: ["a"]
    start: ["b"]
    exposeAs:
      lower: "http://127.0.0.1:${PORT}"
      LD_PRELOAD: "http://127.0.0.1:${PORT}"
      API_PORT: "${PORT}"
      OK_URL: "http://127.0.0.1:${PORT}"
```
