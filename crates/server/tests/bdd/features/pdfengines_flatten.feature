# Feature: PDF Flatten
# Ported from Gotenberg's pdfengines_flatten.feature

Feature: /forms/pdfengines/flatten

  Scenario: POST /forms/pdfengines/flatten
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files | page_1.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
