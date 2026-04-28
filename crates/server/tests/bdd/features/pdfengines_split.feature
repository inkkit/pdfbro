# Feature: PDF Split
# Ported from Gotenberg's pdfengines_split.feature

Feature: /forms/pdfengines/split

  Scenario: POST /forms/pdfengines/split (Intervals)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | pages_3.pdf | file  |
      | splitMode | intervals   | field |
      | splitSpan | 2           | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  Scenario: POST /forms/pdfengines/split (Pages)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | pages_3.pdf | file  |
      | splitMode | pages       | field |
      | splitSpan | 2-          | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"

  Scenario: POST /forms/pdfengines/split (Pages & Unify)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files      | pages_3.pdf | file  |
      | splitMode  | pages       | field |
      | splitSpan  | 2-          | field |
      | splitUnify | true        | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
