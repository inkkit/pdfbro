'use strict';

const { PdfBro: NativePdfBro } = require('./_native.js');

class PdfBroError extends Error { constructor(m){ super(m); this.name='PdfBroError'; } }
class ChromeNotFoundError extends PdfBroError { constructor(m){ super(m); this.name='ChromeNotFoundError'; } }
class ChromeFetchError extends PdfBroError { constructor(m){ super(m); this.name='ChromeFetchError'; } }
class ChromiumError extends PdfBroError { constructor(m){ super(m); this.name='ChromiumError'; } }
class OfficeError extends PdfBroError { constructor(m){ super(m); this.name='OfficeError'; } }
class EngineDisabledError extends PdfBroError { constructor(m){ super(m); this.name='EngineDisabledError'; } }
class TimeoutError extends PdfBroError { constructor(m){ super(m); this.name='TimeoutError'; } }
class ValidationError extends PdfBroError { constructor(m){ super(m); this.name='ValidationError'; } }

const tagMap = {
  ChromeNotFound: ChromeNotFoundError,
  ChromeFetch: ChromeFetchError,
  Chromium: ChromiumError,
  Office: OfficeError,
  EngineDisabled: EngineDisabledError,
  Timeout: TimeoutError,
  Validation: ValidationError,
};

function decorate(err) {
  if (!(err instanceof Error)) return err;
  const m = err.message || '';
  const match = m.match(/^\[(\w+)\]\s*(.*)$/);
  if (!match) return err;
  const Cls = tagMap[match[1]];
  if (!Cls) return err;
  const decorated = new Cls(match[2]);
  decorated.cause = err;
  return decorated;
}

function wrapMethod(fn) {
  return async function(...args) {
    try { return await fn.apply(this, args); }
    catch (e) { throw decorate(e); }
  };
}

class PdfBro {
  constructor(inner) { this._inner = inner; }
  static async create(opts) {
    try {
      const inner = await NativePdfBro.create(opts);
      return new PdfBro(inner);
    } catch (e) { throw decorate(e); }
  }
}
for (const m of ['htmlToPdf', 'urlToPdf', 'markdownToPdf', 'officeToPdf', 'close']) {
  PdfBro.prototype[m] = wrapMethod(function(...args) { return this._inner[m](...args); });
}

module.exports = {
  PdfBro,
  PdfBroError, ChromeNotFoundError, ChromeFetchError, ChromiumError,
  OfficeError, EngineDisabledError, TimeoutError, ValidationError,
};
