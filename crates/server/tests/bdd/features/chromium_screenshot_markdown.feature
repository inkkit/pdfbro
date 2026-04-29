# Feature: Chromium Markdown Screenshot
# Ported from Gotenberg's chromium_screenshot_markdown.feature

Feature: /forms/chromium/screenshot/markdown

  Scenario: POST /forms/chromium/screenshot/markdown (PNG default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files | index.md | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/png"

  Scenario: POST /forms/chromium/screenshot/markdown (JPEG)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files  | index.md | file  |
      | format | jpeg     | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/jpeg"

  Scenario: POST /forms/chromium/screenshot/markdown (WebP)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files  | index.md | file  |
      | format | webp     | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/webp"

  Scenario: POST /forms/chromium/screenshot/markdown (missing file)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | Gotenberg-Output-Filename | result | header |
    Then the response status code should be 400
