# pdfbro (Python)

Rust-native PDF conversion, embeddable. See spec at
`docs/superpowers/specs/2026-05-01-bindings-design.md`.

## Install

    pip install pdfbro

## Quick start

    from pdfbro import PdfBro
    with PdfBro() as f:
        pdf = f.html_to_pdf("<h1>hi</h1>")
        open("out.pdf", "wb").write(pdf)

## Async

    import asyncio
    from pdfbro import AsyncPdfBro

    async def main():
        f = await AsyncPdfBro.create()
        try:
            pdf = await f.html_to_pdf("<h1>hi</h1>")
        finally:
            await f.close()
        return pdf

    asyncio.run(main())
