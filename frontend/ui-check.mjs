import { chromium } from 'playwright';
const url = 'http://localhost:5173';
const browser = await chromium.launch();
const page = await browser.newPage();
const errors = [];
const warnings = [];
page.on('console', msg => {
  if (msg.type() === 'error') errors.push(msg.text());
  if (msg.type() === 'warning') warnings.push(msg.text());
});
page.on('pageerror', err => errors.push(err.message));
await page.goto(url, { waitUntil: 'networkidle' });
await page.waitForTimeout(2000);
await page.screenshot({ path: '/tmp/ui-check.png', fullPage: false });
console.log('ERRORS:', JSON.stringify(errors));
console.log('WARNINGS:', JSON.stringify(warnings));
await browser.close();
