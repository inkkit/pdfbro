'use strict';

const { Folio: NativeFolio } = require('./_native.js');

class FolioError extends Error { constructor(m){ super(m); this.name='FolioError'; } }
class ChromeNotFoundError extends FolioError { constructor(m){ super(m); this.name='ChromeNotFoundError'; } }
class ChromeFetchError extends FolioError { constructor(m){ super(m); this.name='ChromeFetchError'; } }
class ChromiumError extends FolioError { constructor(m){ super(m); this.name='ChromiumError'; } }
class OfficeError extends FolioError { constructor(m){ super(m); this.name='OfficeError'; } }
class EngineDisabledError extends FolioError { constructor(m){ super(m); this.name='EngineDisabledError'; } }
class TimeoutError extends FolioError { constructor(m){ super(m); this.name='TimeoutError'; } }
class ValidationError extends FolioError { constructor(m){ super(m); this.name='ValidationError'; } }

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

class Folio {
  constructor(inner) { this._inner = inner; }
  static async create(opts) {
    try {
      const inner = await NativeFolio.create(opts);
      return new Folio(inner);
    } catch (e) { throw decorate(e); }
  }
}
for (const m of ['htmlToPdf', 'urlToPdf', 'markdownToPdf', 'officeToPdf', 'close']) {
  Folio.prototype[m] = wrapMethod(function(...args) { return this._inner[m](...args); });
}

module.exports = {
  Folio,
  FolioError, ChromeNotFoundError, ChromeFetchError, ChromiumError,
  OfficeError, EngineDisabledError, TimeoutError, ValidationError,
};
