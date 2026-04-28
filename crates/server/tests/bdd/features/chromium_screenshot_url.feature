# Feature: Chromium URL Screenshot
# Ported from Gotenberg's chromium_screenshot_url.feature

Feature: /forms/chromium/screenshot/url

  Scenario: POST /forms/chromium/screenshot/url (PNG)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url    | https://example.com | field |
      | format | png                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/png"

  Scenario: POST /forms/chromium/screenshot/url (full page)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url      | https://example.com | field |
      | fullPage | true                | field |
    Then the response status code should be 200
