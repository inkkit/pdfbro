import os, pathlib, pytest
import folio

E2E = os.environ.get("FOLIO_E2E") == "1"
pytestmark = pytest.mark.skipif(not E2E, reason="FOLIO_E2E not set")

FIXTURE = pathlib.Path(__file__).resolve().parents[2] / "fixtures" / "hello.html"

def test_html_to_pdf_sync():
    with folio.Folio(engines=["chromium"]) as f:
        pdf = f.html_to_pdf(FIXTURE.read_text())
    assert pdf[:4] == b"%PDF"

def test_url_to_pdf_sync():
    with folio.Folio(engines=["chromium"]) as f:
        pdf = f.url_to_pdf("about:blank")
    assert pdf[:4] == b"%PDF"

def test_markdown_to_pdf_sync():
    with folio.Folio(engines=["chromium"]) as f:
        pdf = f.markdown_to_pdf("# hello\n\nfolio e2e")
    assert pdf[:4] == b"%PDF"

import asyncio

def test_html_to_pdf_async():
    async def run():
        f = await folio.AsyncFolio.create(engines=["chromium"])
        try:
            pdf = await f.html_to_pdf(FIXTURE.read_text())
        finally:
            await f.close()
        return pdf
    pdf = asyncio.run(run())
    assert pdf[:4] == b"%PDF"
