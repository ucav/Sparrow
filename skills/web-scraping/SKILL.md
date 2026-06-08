# Skill: Web Scraping

**Trigger:** scrape, web scraping, extract data, crawler

**Description:** Web scraping avec Playwright/curl : extraction de contenu, parsing HTML, pagination, respect des robots.txt.

## Body

### curl + grep (simple)
```bash
curl -s https://example.com | grep -oP '<title>\K[^<]+'
curl -s https://api.example.com/data | jq '.items[].name'
```

### Playwright (JavaScript — intégré Sparrow)
```javascript
const { chromium } = require('playwright');
const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto('https://example.com');
const text = await page.textContent('body');
console.log(text);
await browser.close();
```

### Bonnes pratiques
1. Vérifier `robots.txt` avant de scraper
2. Rate limit : 1 req/s minimum entre les pages
3. User-Agent honnête : `Sparrow/0.5`
4. Cache local pour éviter de re-scraper

### Pièges
- IP bannie → utiliser un délai + user-agent réaliste
- JavaScript rendering → curl ne suffit pas, utiliser Playwright
- Structure HTML qui change → préférer les APIs quand elles existent
