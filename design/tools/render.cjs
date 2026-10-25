/* Render the local, network-free design prototype using the installed browser. */
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const bundled = path.join(process.env.USERPROFILE || '', '.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
let playwright;
try { playwright = require('playwright'); } catch { playwright = require(process.env.PLAYWRIGHT_MODULE_PATH || bundled); }
const root = path.resolve(__dirname, '../..');
const prototype = pathToFileURL(path.join(root, '.planning/sketches/001-x-clipboard/index.html')).href;
const renderRoot = path.join(root, 'design/renders');
const browserPath = process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const sample = process.argv.includes('--sample');

async function main() {
  const browser = await playwright.chromium.launch({ executablePath: browserPath, headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 850 }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', err => errors.push(err.message));
  await page.goto(prototype + '?screen=manager&capture=1');
  await page.evaluate(() => window.design.reset());
  await page.locator('.manager').waitFor();
  const screens = await page.evaluate(() => window.design.screens);
  const selected = sample ? screens.filter(([id]) => ['manager', 'quick', 'preview-image', 'storage', 'welcome', 'shortcuts'].includes(id)) : screens;
  const entries = [];
  const layoutIssues = [];
  const capture = async (id, label, theme, platform) => {
    const isQuick = ['quick','empty','no-results','missing-file','preview-error','permission','storage-error','loading','save-pending','paused','emoji','symbols'].includes(id) || id.startsWith('preview-');
    await page.setViewportSize({ width: isQuick ? 1040 : 1280, height: isQuick ? 680 : 850 });
    await page.goto(`${prototype}?screen=${id}&theme=${theme}&platform=${platform}&capture=1`);
    await page.evaluate(() => window.design.reset());
    await page.evaluate(() => document.fonts.ready);
    await page.locator('#stage').waitFor();
    // Export long settings pages completely; the interactive 760px window stays scrollable.
    await page.evaluate(() => {
      const body = document.querySelector('.settings-body');
      const app = document.querySelector('.manager');
      if (body && app && body.scrollHeight > body.clientHeight) app.style.height = (app.clientHeight + body.scrollHeight - body.clientHeight + 4) + 'px';
    });
    const layout = await page.evaluate(() => {
      const stage = document.querySelector('#stage');
      return { docWidth: document.documentElement.scrollWidth, viewport: innerWidth, stageWidth: stage.scrollWidth,
        clipped: [...document.querySelectorAll('.settings-body,.onboard-main,.page-heading,.quick-foot')]
          .filter(el => el.scrollHeight > el.clientHeight + 2 || el.scrollWidth > el.clientWidth + 2)
          .map(el => ({ class: el.className, height: el.clientHeight, scroll: el.scrollHeight, width: el.clientWidth, scrollWidth: el.scrollWidth })) };
    });
    if (layout.docWidth > layout.viewport || layout.clipped.length) layoutIssues.push({ id, theme, platform, ...layout });
    const folder = `${platform}-${theme}`;
    fs.mkdirSync(path.join(renderRoot, folder), { recursive: true });
    const filename = `${String(screens.findIndex(([s]) => s === id) + 1).padStart(2, '0')}-${id}.png`;
    await page.screenshot({ path: path.join(renderRoot, folder, filename), fullPage: true, animations: 'disabled' });
    entries.push({ id, label, theme, platform, file: `renders/${folder}/${filename}`, prototype: `../.planning/sketches/001-x-clipboard/index.html?screen=${id}&theme=${theme}&platform=${platform}` });
  };
  for (const theme of ['light','dark']) {
    for (const [id,label] of selected) await capture(id,label,theme,'windows');
    console.log(`Windows ${theme}: ${selected.length} views rendered`);
  }
  if (!sample) for (const platform of ['macos','linux']) {
    for (const theme of ['light','dark']) for (const id of ['manager','quick','general']) await capture(id, screens.find(([s]) => s === id)[1], theme, platform);
    console.log(`${platform}: 6 platform variants rendered`);
  }
  const manifest = { generated: '2026-10-24', stage: 'design-review', count: entries.length, entries, errors, layoutIssues };
  fs.mkdirSync(path.join(root, 'design'), { recursive: true });
  fs.writeFileSync(path.join(root, 'design', sample ? 'sample-manifest.json' : 'render-manifest.json'), JSON.stringify(manifest, null, 2));
  if (!sample) fs.writeFileSync(path.join(root, 'design/render-manifest.js'), 'window.RENDER_MANIFEST = ' + JSON.stringify(manifest) + ';\n');
  console.log(JSON.stringify({ count: entries.length, errors, layoutIssues }, null, 2));
  await browser.close();
  if (errors.length || layoutIssues.length) process.exitCode = 1;
}
main().catch(err => { console.error(err); process.exitCode = 1; });
