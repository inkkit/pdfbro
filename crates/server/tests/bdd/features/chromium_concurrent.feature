# Feature: Chromium Concurrent Requests
# Ported from Gotenberg's chromium_concurrent.feature

Feature: Chromium Concurrent

  Scenario: Multiple concurrent HTML conversions
    Given I have a default Folio container
    When I make concurrent "POST" requests to "/forms/chromium/convert/html" with the following form data:
      | files | index.html | file |
    Then all responses should have status code 200
