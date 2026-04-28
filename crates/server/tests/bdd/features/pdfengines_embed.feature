# Feature: PDF File Embedding (PDF/A-3)
# Ported from Gotenberg's pdfengines_embed.feature

Feature: /forms/pdfengines/convert (embed)

  Scenario: POST /forms/pdfengines/convert with embedded file
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | page_1.pdf | file |
      | embedFiles | embed_1.xml | file |
      | pdfa | PDF/A-3b | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST with multiple embedded files
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | page_1.pdf | file |
      | embedFiles | embed_1.xml | file |
      | embedFiles | embed_2.xml | file |
      | pdfa | PDF/A-3b | field |
    Then the response status code should be 200
