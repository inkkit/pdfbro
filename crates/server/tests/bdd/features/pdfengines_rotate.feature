# Feature: PDF Rotation
# Ported from Gotenberg's pdfengines_rotate.feature

Feature: /forms/pdfengines/rotate

  Scenario: POST /forms/pdfengines/rotate (90 degrees)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files | page_1.pdf | file |
      | rotate  | 90       | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/pdfengines/rotate (180 degrees)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files | page_1.pdf | file  |
      | rotate  | 180      | field |
    Then the response status code should be 200
