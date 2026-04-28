# Feature: PDF Bookmarks
# Ported from Gotenberg's pdfengines_bookmarks.feature

Feature: /forms/pdfengines/bookmarks

  Scenario: POST /forms/pdfengines/bookmarks/read
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | page_1_with_bookmarks.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"

  Scenario: POST /forms/pdfengines/bookmarks/write
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files     | page_1.pdf | file  |
      | bookmarks | [{"title":"Chapter 1","page":1}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
