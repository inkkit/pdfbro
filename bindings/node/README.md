# @folio/folio

Rust-native PDF conversion, embeddable in Node. See spec at
`docs/superpowers/specs/2026-05-01-bindings-design.md`.

    npm install @folio/folio

    import { Folio } from '@folio/folio';
    const f = await Folio.create();
    try {
      const pdf = await f.htmlToPdf('<h1>hi</h1>');
    } finally {
      await f.close();
    }
