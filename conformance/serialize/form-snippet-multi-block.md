---
artifactType: Snippet
title: API URLs
description: Development and production API base URLs.
tags: [api, urls]
---

## Development
Local development server URL.

```javascript
const baseUrl = 'http://localhost:<VK-PORT>';
```

vars:
```vks
VK-PORT: '3000'
```

## Production
Production API base URL.

```javascript
const baseUrl = 'https://api.<VK-DOMAIN>';
```

vars:
```vks
VK-DOMAIN: example.com
```
