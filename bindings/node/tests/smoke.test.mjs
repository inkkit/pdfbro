import { describe, it, expect } from 'vitest';
import {
  Folio,
  FolioError,
  ChromeNotFoundError,
  ChromeFetchError,
  ChromiumError,
  OfficeError,
  EngineDisabledError,
  TimeoutError,
  ValidationError,
} from '../index.js';

describe('module exports', () => {
  it('exposes Folio class with create static method', () => {
    expect(typeof Folio.create).toBe('function');
  });

  it('exposes Folio instance methods on prototype', () => {
    for (const m of ['htmlToPdf', 'urlToPdf', 'markdownToPdf', 'officeToPdf', 'close']) {
      expect(typeof Folio.prototype[m]).toBe('function');
    }
  });

  it('error subclasses extend FolioError', () => {
    expect(new ChromeNotFoundError('x')).toBeInstanceOf(FolioError);
    expect(new ChromeFetchError('x')).toBeInstanceOf(FolioError);
    expect(new ChromiumError('x')).toBeInstanceOf(FolioError);
    expect(new OfficeError('x')).toBeInstanceOf(FolioError);
    expect(new EngineDisabledError('x')).toBeInstanceOf(FolioError);
    expect(new TimeoutError('x')).toBeInstanceOf(FolioError);
    expect(new ValidationError('x')).toBeInstanceOf(FolioError);
  });

  it('FolioError extends Error', () => {
    expect(new FolioError('x')).toBeInstanceOf(Error);
  });
});
