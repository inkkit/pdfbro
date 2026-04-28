# Feature: Chromium Markdown to PDF Conversion
# Ported from Gotenberg's chromium_convert_markdown.feature

Feature: /forms/chromium/convert/markdown

  Scenario: POST /forms/chromium/convert/markdown (default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | index.md | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/chromium/convert/markdown with options
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files      | index.md | file  |
      | paperWidth | 8.27     | field |
      | paperHeight | 11.69   | field |
    Then the response status code should be 200
