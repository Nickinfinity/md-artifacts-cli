---
artifactType: LeetCode
leetcodeType: function
title: Valid Anagram
difficulty: easy
status: attempted
tags: [leetcode, strings]
---

Return whether two strings are anagrams of each other.

```yaml leetcode
function: isAnagram
params:
  - name: s
    type: string
  - name: t
    type: string
returns: bool
```

## Tests
```json
[
  { "input": { "s": "anagram", "t": "nagaram" }, "expected": true }
]
```

# Setup

## Python
```python
def isAnagram(s, t):
    pass
```

# Solutions

## Python
```python
def isAnagram(s, t):
    return sorted(s) == sorted(t)
```

# Attempts

## Python
<!-- attempt: { "at": "2026-11-01T09:00:00Z", "duration": "4m10s", "passed": false } -->
```python
def isAnagram(s, t):
    return s == t
```
