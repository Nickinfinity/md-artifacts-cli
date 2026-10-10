---
artifactType: Template
title: rs-render-nested
language: js
---

```js
<VK-each:users>
<VK-users.name>: <VK-each:users.tags:",">[<VK-users.tags>]<VK-end:users.tags>
<VK-end:users>
end
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
