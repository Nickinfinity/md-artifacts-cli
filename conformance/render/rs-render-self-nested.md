---
artifactType: Template
title: rs-render-self-nested
language: js
---

```js
<VK-each:env>(<VK-each:env>x<VK-end:env>)<VK-end:env>
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
