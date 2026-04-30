@chromium
@chromium-concurrent
Feature: Chromium concurrent conversions

  Scenario: Concurrent HTML to PDF conversions with max concurrency 3
    Given I have a Folio container with the following environment variable(s):
      | CHROMIUM_MAX_CONCURRENCY | 3 |
    When I make concurrent "POST" requests to "/forms/chromium/convert/html" with the following form data:
      | files | testdata/page-1-html/index.html | file |
    Then all concurrent response status codes should be 200
    Then all concurrent responses should have 1 PDF(s)

  Scenario: Concurrent conversions exceeding restart-after limit
    Given I have a Folio container with the following environment variable(s):
      | CHROMIUM_MAX_CONCURRENCY | 3 |
      | CHROMIUM_RESTART_AFTER   | 5 |
    When I make concurrent "POST" requests to "/forms/chromium/convert/html" with the following form data:
      | files | testdata/page-1-html/index.html | file |
    Then all concurrent response status codes should be 200
    Then all concurrent responses should have 1 PDF(s)
