# Feature: Chromium URL to PDF Conversion
# Ported from Gotenberg's chromium_convert_url.feature

Feature: /forms/chromium/convert/url

  Scenario: POST /forms/chromium/convert/url
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/url" with the following form data and header(s):
      | url                       | https://google.com | field  |
      | Gotenberg-Output-Filename | result              | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/chromium/convert/url (bad URL)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/url" with the following form data and header(s):
      | url | not-a-valid-url | field |
    Then the response status code should be 400
