# Feature: PDF Metadata
# Ported from Gotenberg's pdfengines_metadata.feature

Feature: /forms/pdfengines/metadata

  Scenario: POST /forms/pdfengines/metadata/read
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | files | page_1.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"

  Scenario: POST /forms/pdfengines/metadata/write
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files    | page_1.pdf | file  |
      | metadata | {"title":"Test"} | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
