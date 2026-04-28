# Feature: LibreOffice Document Conversion
# Ported from Gotenberg's libreoffice_convert.feature

Feature: /forms/libreoffice/convert

  Scenario: POST /forms/libreoffice/convert (DOCX to PDF)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | page_1.docx | file   |
      | Gotenberg-Output-Filename | result      | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  # TODO: Enable when LibreOffice route supports password field
  # Scenario: POST /forms/libreoffice/convert (password protected)
  #   Given I have a default Folio container
  #   When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
  #     | files       | protected_page_1.docx | file  |
  #     | password    | secret123             | field |
  #     | Gotenberg-Output-Filename | result | header |
  #   Then the response status code should be 200
