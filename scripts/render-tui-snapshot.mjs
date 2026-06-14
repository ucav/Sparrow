import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { basename, dirname } from 'node:path';

const [inputPath, outputPath] = process.argv.slice(2);

if (!inputPath || !outputPath) {
  console.error('usage: node scripts/render-tui-snapshot.mjs <input.txt> <output.svg>');
  process.exit(1);
}

const text = readFileSync(inputPath, 'utf8').replace(/\r\n/g, '\n');
const lines = text.split('\n');
const charWidth = 9;
const lineHeight = 18;
const paddingX = 24;
const paddingY = 22;
const maxCols = Math.max(...lines.map((line) => [...line].length), 1);
const width = maxCols * charWidth + paddingX * 2;
const height = lines.length * lineHeight + paddingY * 2;

function escapeXml(value) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

const title = basename(inputPath);
const rows = lines
  .map((line, index) => {
    const y = paddingY + 14 + index * lineHeight;
    return `<text x="${paddingX}" y="${y}">${escapeXml(line)}</text>`;
  })
  .join('\n');

const svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <rect width="100%" height="100%" fill="#0e0b08"/>
  <text x="${paddingX}" y="14" fill="#5c5346" font-family="ui-monospace, SFMono-Regular, Consolas, monospace" font-size="11">${escapeXml(title)}</text>
  <g fill="#ece2cf" font-family="ui-monospace, SFMono-Regular, Consolas, monospace" font-size="14" xml:space="preserve">
${rows}
  </g>
</svg>
`;

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, svg, 'utf8');
