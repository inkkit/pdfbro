@libreoffice
@libreoffice-convert
Feature: LibreOffice document-load error mapping

  Scenario: Encrypted DOCX returns 422 with DOCUMENT_ENCRYPTED
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/encrypted.docx | file |
    Then the response status code should be 422
    And the response body should contain "DOCUMENT_ENCRYPTED"

  Scenario: Corrupted DOCX returns 422 with DOCUMENT_CORRUPTED
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/truncated.docx | file |
    Then the response status code should be 422
    And the response body should contain "DOCUMENT_CORRUPTED"

  Scenario: Unknown format returns 415 with UNSUPPORTED_FORMAT
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/unknown.xyz | file |
    Then the response status code should be 415
    And the response body should contain "UNSUPPORTED_FORMAT"
