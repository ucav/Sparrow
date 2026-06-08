#!/usr/bin/env bash
# Install browser dependencies for Sparrow
# Requires: Node.js >= 18
set -e

echo "🔧 Installing Sparrow browser dependencies..."

# Check Node.js
if ! command -v node &>/dev/null; then
    echo "❌ Node.js not found. Install: https://nodejs.org"
    exit 1
fi

echo "  Node.js: $(node --version)"

# Install Playwright
cd /tmp
npm init -y > /dev/null 2>&1
npm install playwright > /dev/null 2>&1
npx playwright install chromium > /dev/null 2>&1

echo "✅ Browser deps installed."
echo ""
echo "Test with: sparrow run 'take a screenshot of https://example.com'"
echo ""
echo "Available browser actions:"
echo "  navigate(url)       — open a page"
echo "  screenshot()        — capture screenshot"
echo "  get_text()          — extract visible text"
echo "  extract(selector)   — extract specific content"
echo "  click(selector)     — click element"
echo "  type(selector,text) — type into input"
echo "  press(key)          — press keyboard key"
echo "  evaluate(js)        — run JavaScript"
