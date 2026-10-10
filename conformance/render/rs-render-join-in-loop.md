---
artifactType: Template
title: rs-render-join-in-loop
language: js
---

```js
<VK-each:users:"; ">(<VK-users.name>: <VK-join:users.tags>)<VK-end:users>
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
