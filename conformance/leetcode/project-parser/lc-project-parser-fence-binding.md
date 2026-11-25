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
    - name: a
      kind: build
      argv: ["make"]
    - name: b
      kind: build
      argv: ["make", "b"]
```

## Tests
```json
[{"input": {}, "expected": 1}]
```

```json check=zzz
[{"input": {}, "expected": 2}]
```

```json check=b
[{"input": {}, "expected": 3}]
```
