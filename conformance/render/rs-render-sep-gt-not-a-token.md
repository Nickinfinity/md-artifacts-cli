---
artifactType: Template
title: rs-render-sep-gt-not-a-token
language: js
---

```js
<VK-each:x:"a>b">
```

vars:
```vks
VK-env:
  - dev
  - prod
VK-users:
  - id: 1
    name: Alice Smith
    tags:
      - core
      - ops
  - id: 2
    name: Bob Jones
    tags: []
VK-db:
  host: localhost
  port: "5432"
VK-none: []
```
