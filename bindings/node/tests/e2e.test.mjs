import { describe, it, expect } from 'vitest';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { PdfBro } from '../index.js';

const E2E = process.env.PDFBRO_E2E === '1';
const here = dirname(fileURLToPath(import.meta.url));
const fixture = resolve(here, '../../fixtures/hello.html');

describe.skipIf(!E2E)('e2e', () => {
  it('htmlToPdf', async () => {
    const f = await PdfBro.create({ engines: ['chromium'] });
    try {
      const html = await readFile(fixture, 'utf8');
      const pdf = await f.htmlToPdf(html);
      expect(pdf.subarray(0, 4).toString()).toBe('%PDF');
    } finally { await f.close(); }
  }, 120_000);

  it('urlToPdf', async () => {
    const f = await PdfBro.create({ engines: ['chromium'] });
    try {
      const pdf = await f.urlToPdf('about:blank');
      expect(pdf.subarray(0, 4).toString()).toBe('%PDF');
    } finally { await f.close(); }
  }, 120_000);

  it('markdownToPdf', async () => {
    const f = await PdfBro.create({ engines: ['chromium'] });
    try {
      const pdf = await f.markdownToPdf('# hello');
      expect(pdf.subarray(0, 4).toString()).toBe('%PDF');
    } finally { await f.close(); }
  }, 120_000);
});
