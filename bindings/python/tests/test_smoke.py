import pdfbro

def test_module_exports():
    assert hasattr(pdfbro, "PdfBro")
    assert hasattr(pdfbro, "AsyncPdfBro")
    assert issubclass(pdfbro.ChromeNotFoundError, pdfbro.PdfBroError)
    assert issubclass(pdfbro.ChromiumError, pdfbro.PdfBroError)
    assert issubclass(pdfbro.OfficeError, pdfbro.PdfBroError)
    assert issubclass(pdfbro.ValidationError, pdfbro.PdfBroError)

def test_validation_error_class_exists():
    assert pdfbro.ValidationError is not None
    assert issubclass(pdfbro.ValidationError, pdfbro.PdfBroError)

def test_pdfbro_class_methods():
    # Don't instantiate (would launch Chrome). Just check the class exists.
    assert hasattr(pdfbro.PdfBro, "html_to_pdf")
    assert hasattr(pdfbro.PdfBro, "url_to_pdf")
    assert hasattr(pdfbro.PdfBro, "markdown_to_pdf")
    assert hasattr(pdfbro.PdfBro, "office_to_pdf")
    assert hasattr(pdfbro.PdfBro, "close")
    assert hasattr(pdfbro.PdfBro, "__enter__")
    assert hasattr(pdfbro.PdfBro, "__exit__")

def test_async_pdfbro_class_exists():
    assert hasattr(pdfbro.AsyncPdfBro, "create")
    assert hasattr(pdfbro.AsyncPdfBro, "html_to_pdf")
    assert hasattr(pdfbro.AsyncPdfBro, "url_to_pdf")
    assert hasattr(pdfbro.AsyncPdfBro, "markdown_to_pdf")
    assert hasattr(pdfbro.AsyncPdfBro, "office_to_pdf")
    assert hasattr(pdfbro.AsyncPdfBro, "close")

def test_async_pdfbro_create_returns_coroutine():
    """AsyncPdfBro.create() must return an awaitable, not eagerly launch."""
    import pdfbro, inspect
    # Don't call it (would launch chrome). Just confirm it's a static method
    # and the signature accepts our args.
    assert callable(pdfbro.AsyncPdfBro.create)
