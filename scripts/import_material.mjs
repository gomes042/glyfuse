// Port associations and dominant SVG colors from the MIT-licensed Material Icon Theme.
// Run: node scripts/import_material.mjs .source-material
// SVGs are read for palette extraction only; no SVG assets ship with Glyfuse.
// Nerd Font glyph mappings below are intentionally approximations, not SVG equivalents.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';

const source = resolve(process.argv[2] ?? '.source-material');
const icons = join(source, 'icons');
const input = join(source, 'src/core/icons');
if (!existsSync(join(source, 'LICENSE')) || !existsSync(join(input, 'folderIcons.ts'))) {
  throw Error(`Material Icon Theme checkout missing at ${source}`);
}
const commit = execFileSync('git', ['-C', source, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();

// Scanner respects quotes and nesting so nested `clone`, `patterns`, etc. do not
// terminate the containing theme definition prematurely.
function endOfObject(text, start) {
  let depth = 0, quoted = null, escape = false;
  for (let i = start; i < text.length; ++i) {
    const c = text[i];
    if (quoted) {
      if (escape) { escape = false; continue; }
      if (c === '\\') { escape = true; continue; }
      if (c === quoted) quoted = null;
      continue;
    }
    if (c === '"' || c === "'" || c === '`') { quoted = c; continue; }
    if (c === '{') depth++;
    if (c === '}') {
      depth--;
      if (depth === 0) return i + 1;
    }
  }
  throw Error('Unterminated upstream icon object');
}

function definitions(filename, property) {
  const text = readFileSync(join(input, filename), 'utf8');
  const result = [];
  const pattern = /\{\s*name:\s*['"]([^'"\n]+)['"]/g;
  for (const match of text.matchAll(pattern)) {
    const content = text.slice(match.index, endOfObject(text, match.index));
    const array = new RegExp(`\\b${property}:\\s*\\[([\\s\\S]*?)\\]`).exec(content);
    if (!array) continue;
    const names = [...array[1].matchAll(/['"]([^'"\n]+)['"]/g)].map(x => x[1]);
    if (names.length) result.push({ id: match[1], names });
  }
  return result;
}

const specialGlyphs = {
  rust: '\ue7a8', python: '\ue73c', javascript: '\ue74e', typescript: '\ue628',
  react: '\ue7ba', html: '\ue736', css: '\ue749', sass: '\ue74b',
  json: '\ue60b', yaml: '\uf481', markdown: '\ue73e', git: '\ue5fb',
  docker: '\ue7b0', go: '\ue626', java: '\ue738', lua: '\ue620',
  c: '\ue61e', cpp: '\ue61d', csharp: '\uf031b', shell: '\ue795',
  powershell: '\ue795', ruby: '\ue739', php: '\ue73d',
  kotlin: '\ue634', swift: '\ue755', vue: '\uf0c45',
  angular: '\ue753', svelte: '\ue697', nodejs: '\ue719',
  xml: '\uf05c0', sql: '\ue706', database: '\ue706',
  image: '\uf03e', video: '\uf03d', audio: '\uf001',
  pdf: '\uf1c1', zip: '\uf1c6', lock: '\uf023',
  test: '\uf0c3', config: '\uf013', settings: '\uf013',
};

function glyph(id, folder) {
  const token = id.replace(/^folder-/, '').replace(/[-_].*$/, '');
  if (specialGlyphs[id]) return specialGlyphs[id];
  if (specialGlyphs[token]) return specialGlyphs[token];
  return folder ? '\ue5ff' : '\uf016';
}
function dominantColor(id, folder) {
  const file = join(icons, `${id}.svg`);
  if (!existsSync(file)) return folder ? '#90A4AE' : '#B0BEC5';
  const svg = readFileSync(file, 'utf8');
  const folderColor = svg.match(/id="folder"\s+fill="(#[0-9a-fA-F]{6})"/);
  const fill = svg.match(/\bfill="(#[0-9a-fA-F]{6})"/);
  return (folderColor?.[1] || fill?.[1] || (folder ? '#90A4AE' : '#B0BEC5')).toLowerCase();
}
function rule(kind, name, id) {
  const folder = kind === 'folder';
  return { kind, pattern: name.toLowerCase(), icon: { glyph: glyph(id, folder), color: dominantColor(id, folder) }, source_icon: id };
}

const rules = [];
for (const { id, names } of definitions('folderIcons.ts', 'folderNames')) {
  for (const name of names) rules.push(rule('folder', name, id));
}
for (const { id, names } of definitions('fileIcons.ts', 'fileExtensions')) {
  for (const name of names) rules.push(rule('extension', name, id));
}
for (const { id, names } of definitions('fileIcons.ts', 'fileNames')) {
  for (const name of names) rules.push(rule('file', name, id));
}
const theme = {
  format: 1,
  name: 'Material Icon Theme (terminal approximation)',
  fallbacks: {
    file: { glyph: '\uf016', color: dominantColor('file', false) },
    folder: { glyph: '\ue5ff', color: dominantColor('folder', true) },
    folder_expanded: { glyph: '\ue5fe', color: dominantColor('folder', true) },
  },
  rules,
};
writeFileSync(join(import.meta.dirname, '..', 'themes', 'material.json'), JSON.stringify(theme, null, 2) + '\n');
console.log(`Material upstream ${commit}: ${rules.length} associations (${rules.filter(r => r.kind === 'folder').length} folders, ${rules.filter(r => r.kind === 'file').length} filenames, ${rules.filter(r => r.kind === 'extension').length} extensions).`);
