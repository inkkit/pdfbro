# Feature: PDF Watermark
# Ported from Gotenberg's pdfengines_watermark.feature

Feature: /forms/pdfengines/watermark

  Scenario: POST /forms/pdfengines/watermark (text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/watermark" with the following form data and header(s):
      | files     | page_1.pdf | file  |
      | watermark | CONFIDENTIAL | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/watermark with opacity
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/watermark" with the following form data and header(s):
      | files     | page_1.pdf | file  |
      | watermark | DRAFT      | field |
      | opacity   | 0.3        | field |
    Then the response status code should be 200
