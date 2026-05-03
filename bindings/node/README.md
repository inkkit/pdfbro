# @vel/pdfbro

Rust-native PDF conversion, embeddable in Node. See spec at
`docs/superpowers/specs/2026-05-01-bindings-design.md`.

    npm install @vel/pdfbro

    import { PdfBro } from '@vel/pdfbro';
    const f = await PdfBro.create();
    try {
      const pdf = await f.htmlToPdf('<h1>hi</h1>');
    } finally {
      await f.close();
    }
