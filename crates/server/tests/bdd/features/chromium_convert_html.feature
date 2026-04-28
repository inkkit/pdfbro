# Feature: Chromium HTML to PDF Conversion
# Ported from Gotenberg's chromium_convert_html.feature
# Simplified for Folio core features

Feature: /forms/chromium/convert/html

  Scenario: POST /forms/chromium/convert/html (default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | index.html | file   |
      | Gotenberg-Output-Filename | result     | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/chromium/convert/html (missing file)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | Gotenberg-Output-Filename | result | header |
    Then the response status code should be 400
