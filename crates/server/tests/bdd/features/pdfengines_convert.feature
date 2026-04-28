# Feature: PDF/A Conversion
# Ported from Gotenberg's pdfengines_convert.feature

Feature: /forms/pdfengines/convert

  Scenario: POST /forms/pdfengines/convert (PDF/A-1b)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | page_1.pdf | file |
      | pdfa  | PDF/A-1b   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/convert (PDF/A-2b)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | page_1.pdf | file |
      | pdfa  | PDF/A-2b   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/pdfengines/convert (PDF/A-3b)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | page_1.pdf | file |
      | pdfa  | PDF/A-3b   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/pdfengines/convert (Bad Request - no pdfa)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | page_1.pdf | file |
    Then the response status code should be 400
