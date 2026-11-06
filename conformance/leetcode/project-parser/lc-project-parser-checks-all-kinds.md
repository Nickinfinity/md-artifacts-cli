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
    - name: page
      kind: dom-assert
      file: index.html
    - name: page2
      kind: dom-assert
      file: index.html
      package: web
    - name: style
      kind: css-assert
      file: src/app.css
    - name: api
      kind: http
      package: api
    - name: b
      kind: build
      argv: ["make"]
```

## Tests
```json check=page
[{"input": {}, "expected": 1}]
```

```json check=api
[{"input": {}, "expected": 2}]
```
