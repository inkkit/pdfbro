import { describe, it, expect } from 'vitest';
import {
  PdfBro,
  PdfBroError,
  ChromeNotFoundError,
  ChromeFetchError,
  ChromiumError,
  OfficeError,
  EngineDisabledError,
  TimeoutError,
  ValidationError,
} from '../index.js';

describe('module exports', () => {
  it('exposes PdfBro class with create static method', () => {
    expect(typeof PdfBro.create).toBe('function');
  });

  it('exposes PdfBro instance methods on prototype', () => {
    for (const m of ['htmlToPdf', 'urlToPdf', 'markdownToPdf', 'officeToPdf', 'close']) {
      expect(typeof PdfBro.prototype[m]).toBe('function');
    }
  });

  it('error subclasses extend PdfBroError', () => {
    expect(new ChromeNotFoundError('x')).toBeInstanceOf(PdfBroError);
    expect(new ChromeFetchError('x')).toBeInstanceOf(PdfBroError);
    expect(new ChromiumError('x')).toBeInstanceOf(PdfBroError);
    expect(new OfficeError('x')).toBeInstanceOf(PdfBroError);
    expect(new EngineDisabledError('x')).toBeInstanceOf(PdfBroError);
    expect(new TimeoutError('x')).toBeInstanceOf(PdfBroError);
    expect(new ValidationError('x')).toBeInstanceOf(PdfBroError);
  });

  it('PdfBroError extends Error', () => {
    expect(new PdfBroError('x')).toBeInstanceOf(Error);
  });
});
