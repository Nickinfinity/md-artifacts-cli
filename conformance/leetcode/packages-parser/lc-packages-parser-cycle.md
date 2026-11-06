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
  - name: a
    dir: a
    install: ["x"]
    start: ["y"]
    dependsOn: [b]
  - name: b
    dir: b
    install: ["x"]
    start: ["y"]
    dependsOn: [a]
```
