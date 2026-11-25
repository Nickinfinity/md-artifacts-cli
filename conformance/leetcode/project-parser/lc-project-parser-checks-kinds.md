---
artifactType: LeetCode
leetcodeType: package
title: Case
difficulty: easy
status: unsolved
tags: [leetcode]
---

```yaml leetcode
test:
  checks:
    - name: r
      kind: render
    - name: cl
      kind: class
    - name: u
      kind: teleport
    - name: h
      kind: http
    - name: b
      kind: build
      argv: "npm run build"
    - name: d
      kind: dom-assert
    - name: c
      kind: call
      file: a.ts
    - name: ok
      kind: build
      argv: ["npm", "run", "build"]
      dir: web
```
