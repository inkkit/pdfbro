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

  Scenario: POST /forms/libreoffice/convert with landscape orientation
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | page_1.docx | file   |
      | landscape                 | true        | field  |
      | Gotenberg-Output-Filename | result      | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/libreoffice/convert with page ranges
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | pages_3.docx | file   |
      | pageRanges                | 1            | field  |
      | Gotenberg-Output-Filename | result       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "result" PDF should have 1 page(s)

  Scenario: POST /forms/libreoffice/convert with native watermark
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | page_1.docx | file   |
      | nativeWatermarkText       | CONFIDENTIAL | field |
      | nativeWatermarkColor      | 16711680    | field  |
      | nativeWatermarkFontHeight | 24          | field  |
      | Gotenberg-Output-Filename | result      | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/libreoffice/convert with viewer preferences
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | page_1.docx | file   |
      | initialView               | 1           | field  |
      | pageLayout                | 2           | field  |
      | hideViewerToolbar         | true        | field  |
      | Gotenberg-Output-Filename | result      | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/libreoffice/convert with multiple advanced options
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                         | pages_3.docx | file   |
      | landscape                     | true         | field  |
      | exportBookmarks               | true         | field  |
      | exportFormFields              | true         | field  |
      | exportNotes                   | true         | field  |
      | skipEmptyPages                | true         | field  |
      | singlePageSheets              | true         | field  |
      | losslessImageCompression      | true         | field  |
      | reduceImageResolution         | true         | field  |
      | maxImageResolution            | 300          | field  |
      | useTransitionEffects          | true         | field  |
      | openBookmarkLevels            | 2            | field  |
      | Gotenberg-Output-Filename     | result       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
