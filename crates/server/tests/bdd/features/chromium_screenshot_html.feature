# Feature: Chromium HTML Screenshot
# Ported from Gotenberg's chromium_screenshot_html.feature

Feature: /forms/chromium/screenshot/html

  Scenario: POST /forms/chromium/screenshot/html (PNG default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/html" with the following form data and header(s):
      | files | index.html | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/png"

  Scenario: POST /forms/chromium/screenshot/html (JPEG)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/html" with the following form data and header(s):
      | files  | index.html | file  |
      | format | jpeg       | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/jpeg"
