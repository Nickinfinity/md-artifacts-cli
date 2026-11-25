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
  - dir: x
    install: ["a"]
    start: ["b"]
  - name: abs
    dir: /srv
    install: ["a"]
    start: ["b"]
  - name: noinstall
    dir: n
    start: ["b"]
  - name: str
    dir: s
    install: "npm ci"
    start: ["b"]
  - name: empty
    dir: e
    install: []
    start: ["b"]
  - name: subst
    dir: u
    install: ["a"]
    start: ["run", "${HOME}"]
```
