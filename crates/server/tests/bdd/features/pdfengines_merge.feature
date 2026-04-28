# Feature: PDF Merge
# Ported from Gotenberg's pdfengines_merge.feature
# Note: Folio uses lopdf for merging (no separate engine selection)

Feature: /forms/pdfengines/merge

  Scenario: POST /forms/pdfengines/merge (default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | page_1.pdf | file   |
      | files                     | page_2.pdf | file   |
      | Gotenberg-Output-Filename | result     | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | result.pdf |
    Then the "result.pdf" PDF should have 2 page(s)

  Scenario: POST /forms/pdfengines/merge (Bad Request - no files)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | Gotenberg-Output-Filename | result | header |
    Then the response status code should be 400
