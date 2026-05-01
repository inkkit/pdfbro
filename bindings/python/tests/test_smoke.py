import folio

def test_module_exports():
    assert hasattr(folio, "Folio")
    assert hasattr(folio, "AsyncFolio")
    assert issubclass(folio.ChromeNotFoundError, folio.FolioError)
    assert issubclass(folio.ChromiumError, folio.FolioError)
    assert issubclass(folio.OfficeError, folio.FolioError)
    assert issubclass(folio.ValidationError, folio.FolioError)

def test_validation_error_class_exists():
    assert folio.ValidationError is not None
    assert issubclass(folio.ValidationError, folio.FolioError)

def test_folio_class_methods():
    # Don't instantiate (would launch Chrome). Just check the class exists.
    assert hasattr(folio.Folio, "html_to_pdf")
    assert hasattr(folio.Folio, "url_to_pdf")
    assert hasattr(folio.Folio, "markdown_to_pdf")
    assert hasattr(folio.Folio, "office_to_pdf")
    assert hasattr(folio.Folio, "close")
    assert hasattr(folio.Folio, "__enter__")
    assert hasattr(folio.Folio, "__exit__")
